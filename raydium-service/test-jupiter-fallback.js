// Test script để kiểm tra Jupiter fallback
const RAYDIUM_SERVICE_URL = 'http://localhost:3001';

async function testJupiterFallback() {
    try {
        console.log('🧪 Testing Jupiter fallback mechanism...\n');
        
        // Test với một swap nhỏ
        const swapRequest = {
            inputMint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v', // USDC
            outputMint: 'So11111111111111111111111111111111111111112', // SOL
            amountIn: '1.0', // 1 USDC (nhỏ hơn để test nhanh)
            slippage: 1.0
        };
        
        console.log('   Request:', JSON.stringify(swapRequest, null, 2));
        console.log('   ⏳ This may take a while as Raydium will retry multiple times...\n');
        
        const startTime = Date.now();
        const swapRes = await fetch(`${RAYDIUM_SERVICE_URL}/swap`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(swapRequest),
            signal: AbortSignal.timeout(300000) // 5 minutes
        });
        
        const swapResult = await swapRes.json();
        const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
        
        console.log(`\n⏱️  Total time: ${elapsed}s\n`);
        
        if (swapRes.ok && swapResult.success) {
            console.log('   ✅ Swap successful!');
            console.log('   Provider:', swapResult.provider || 'Raydium');
            console.log('   Transaction:', swapResult.transactionSignature);
            console.log('   Input:', swapResult.inputAmount);
            console.log('   Output:', swapResult.outputAmount);
            
            if (swapResult.provider === 'Jupiter (fallback)') {
                console.log('\n   🎯 Jupiter fallback was successfully triggered!');
            }
        } else {
            console.error('   ❌ Swap failed!');
            console.error('   Error:', swapResult.error || swapResult);
        }
        
    } catch (error) {
        console.error('❌ Test error:', error.message);
        if (error.name === 'AbortError') {
            console.error('   Request timed out after 5 minutes');
        }
    }
}

testJupiterFallback();


