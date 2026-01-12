#!/usr/bin/env node

/**
 * Script test để mua 1 SOL từ USDC
 * Script này độc lập, không ảnh hưởng đến bot chính
 * Chỉ cần Jupiter service đang chạy
 */

const JUPITER_SERVICE_URL = 'http://localhost:3002';
const USDC_MINT = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
const SOL_MINT = 'So11111111111111111111111111111111111111112';

// Get current SOL price from Binance
async function getCurrentPrice() {
    try {
        const response = await fetch('https://api.binance.com/api/v3/ticker/price?symbol=SOLUSDT');
        const data = await response.json();
        return parseFloat(data.price);
    } catch (error) {
        console.error('❌ Error fetching price:', error.message);
        return null;
    }
}

// Main function
async function main() {
    try {
        console.log('🧪 TEST: Mua 1 SOL từ USDC');
        console.log('='.repeat(50));
        
        // Check if Jupiter service is running
        console.log('\n🔍 Kiểm tra Jupiter service...');
        try {
            const healthResponse = await fetch(`${JUPITER_SERVICE_URL}/health`);
            if (!healthResponse.ok) {
                throw new Error('Jupiter service không phản hồi');
            }
            const health = await healthResponse.json();
            console.log(`   ✅ Jupiter service: ${health.status}`);
            console.log(`   Wallet: ${health.wallet}`);
        } catch (error) {
            console.log(`   ❌ Jupiter service không chạy: ${error.message}`);
            console.log(`   → Vui lòng khởi động Jupiter service: cd jupiter-service && npm start`);
            process.exit(1);
        }
        
        // Get current price
        console.log('\n📊 Đang lấy giá hiện tại...');
        const currentPrice = await getCurrentPrice();
        if (!currentPrice) {
            throw new Error('Không thể lấy giá từ Binance');
        }
        console.log(`   Giá SOL/USDC: $${currentPrice.toFixed(4)}`);
        
        // Calculate USDC needed (thêm 5% buffer để đảm bảo đủ)
        const solAmount = 1.0;
        const usdcNeeded = solAmount * currentPrice * 1.05; // Thêm 5% buffer
        console.log(`\n💵 Cần ~${usdcNeeded.toFixed(2)} USDC để mua ${solAmount} SOL (có buffer 5%)`);
        
        // Check balance first
        console.log('\n💰 Kiểm tra số dư...');
        try {
            // Get balance from API if available, or estimate from wallet
            const balanceResponse = await fetch(`${JUPITER_SERVICE_URL}/health`);
            const health = await balanceResponse.json();
            console.log(`   Wallet: ${health.wallet}`);
            console.log(`   ⚠️  Vui lòng kiểm tra số dư USDC trong ví trước khi swap!`);
            console.log(`   Cần ít nhất: ${(solAmount * currentPrice).toFixed(2)} USDC`);
        } catch (error) {
            console.log(`   ⚠️  Không thể kiểm tra số dư tự động`);
        }
        
        // Get quote first
        console.log('\n📈 Đang lấy quote từ Jupiter...');
        const quoteResponse = await fetch(`${JUPITER_SERVICE_URL}/quote`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                inputMint: USDC_MINT,
                outputMint: SOL_MINT,
                amountIn: (solAmount * currentPrice).toFixed(6), // Chính xác số USDC cần
                slippageBps: 100, // 1% slippage
            }),
        });
        
        if (!quoteResponse.ok) {
            const errorData = await quoteResponse.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(`Quote failed: ${JSON.stringify(errorData)}`);
        }
        
        const quote = await quoteResponse.json();
        console.log(`   Input: ${quote.inputAmount} USDC`);
        console.log(`   Output: ${quote.outputAmount} SOL`);
        console.log(`   Price Impact: ${quote.priceImpact || 'N/A'}`);
        
        // Confirm before swap
        console.log('\n⚠️  XÁC NHẬN GIAO DỊCH:');
        console.log(`   Mua: ~${quote.outputAmount} SOL`);
        console.log(`   Giá: $${currentPrice.toFixed(4)}`);
        console.log(`   Số tiền: ${quote.inputAmount} USDC`);
        console.log(`   Slippage: 1%`);
        console.log('\n   Nhấn Ctrl+C để hủy, hoặc đợi 5 giây để tiếp tục...');
        await new Promise(resolve => setTimeout(resolve, 5000));
        
        // Execute swap
        console.log('\n🔄 Đang thực hiện swap...');
        const swapResponse = await fetch(`${JUPITER_SERVICE_URL}/swap`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                inputMint: USDC_MINT,
                outputMint: SOL_MINT,
                amountIn: quote.inputAmount, // Dùng chính xác số từ quote
                slippage: 1.0,
            }),
        });
        
        if (!swapResponse.ok) {
            const errorData = await swapResponse.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(`Swap failed: ${JSON.stringify(errorData)}`);
        }
        
        const swapResult = await swapResponse.json();
        
        if (swapResult.success) {
            console.log('\n✅ SWAP THÀNH CÔNG!');
            console.log(`   Transaction: ${swapResult.transactionSignature}`);
            console.log(`   Xem trên Solscan: https://solscan.io/tx/${swapResult.transactionSignature}`);
            console.log(`   Output: ${swapResult.outputAmount || 'N/A'} SOL`);
            console.log(`   Provider: ${swapResult.provider || 'Jupiter'}`);
            
            console.log('\n📝 Lưu ý:');
            console.log('   - Transaction này KHÔNG ảnh hưởng đến bot chính');
            console.log('   - Bot sẽ tiếp tục chạy bình thường với thuật toán Bollinger Bands');
            console.log('   - Số dư mới sẽ được cập nhật trong dashboard sau vài phút');
        } else {
            throw new Error(`Swap failed: ${swapResult.error || 'Unknown error'}`);
        }
        
    } catch (error) {
        console.error('\n❌ Lỗi:', error.message);
        if (error.message.includes('Cannot connect')) {
            console.error('\n💡 Gợi ý:');
            console.error('   1. Kiểm tra Jupiter service có đang chạy: curl http://localhost:3002/health');
            console.error('   2. Khởi động Jupiter service: cd jupiter-service && npm start');
        }
        process.exit(1);
    }
}

// Run
main();
