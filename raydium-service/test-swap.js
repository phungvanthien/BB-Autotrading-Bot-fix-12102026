// Test script để kiểm tra swap trên Raydium
// Sử dụng fetch built-in (Node.js 18+)

const RAYDIUM_SERVICE_URL = 'http://localhost:3001';

// Test swap USDC -> SOL
async function testSwap() {
    try {
        console.log('🧪 Testing Raydium swap...\n');
        
        // Test health check first
        console.log('1. Checking health...');
        const healthRes = await fetch(`${RAYDIUM_SERVICE_URL}/health`);
        const health = await healthRes.json();
        console.log('   Health:', health);
        
        if (!health.raydium_initialized) {
            console.error('❌ Raydium SDK not initialized!');
            return;
        }
        
        // Test swap
        console.log('\n2. Testing swap...');
        const swapRequest = {
            inputMint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v', // USDC
            outputMint: 'So11111111111111111111111111111111111111112', // SOL
            amountIn: '10.0', // 10 USDC
            slippage: 1.0
        };
        
        console.log('   Request:', JSON.stringify(swapRequest, null, 2));
        
        const swapRes = await fetch(`${RAYDIUM_SERVICE_URL}/swap`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(swapRequest),
            signal: AbortSignal.timeout(300000) // 5 minutes timeout
        });
        
        const swapResult = await swapRes.json();
        
        if (swapRes.ok && swapResult.success) {
            console.log('   ✅ Swap successful!');
            console.log('   Transaction:', swapResult.transactionSignature);
            console.log('   Output:', swapResult.outputAmount);
        } else {
            console.error('   ❌ Swap failed!');
            console.error('   Error:', swapResult.error || swapResult);
        }
        
    } catch (error) {
        console.error('❌ Test error:', error);
    }
}

testSwap();

