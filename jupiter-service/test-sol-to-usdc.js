// Test swap 0.1 SOL -> USDC
const JUPITER_SERVICE_URL = 'http://localhost:3002';

async function testSwap() {
    try {
        console.log('🧪 Testing swap: 0.1 SOL -> USDC\n');
        
        // Test health check first
        console.log('1. Checking health...');
        const healthRes = await fetch(`${JUPITER_SERVICE_URL}/health`);
        const health = await healthRes.json();
        console.log('   Health:', health);
        
        if (health.status !== 'ok') {
            console.error('❌ Service not healthy!');
            return;
        }
        
        // Test quote first
        console.log('\n2. Getting quote...');
        const quoteRequest = {
            inputMint: 'So11111111111111111111111111111111111111112', // SOL
            outputMint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v', // USDC
            amountIn: '0.1', // 0.1 SOL
            slippageBps: 100 // 1%
        };
        
        const quoteRes = await fetch(`${JUPITER_SERVICE_URL}/quote`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(quoteRequest)
        });
        
        const quoteResult = await quoteRes.json();
        if (quoteRes.ok) {
            console.log('   ✅ Quote successful!');
            console.log('   Input: 0.1 SOL');
            console.log('   Output:', quoteResult.outputAmount, 'USDC');
            console.log('   Price Impact:', quoteResult.priceImpact);
        } else {
            console.error('   ❌ Quote failed!');
            console.error('   Error:', quoteResult.error);
            return;
        }
        
        // Test swap
        console.log('\n3. Executing swap...');
        const swapRequest = {
            inputMint: 'So11111111111111111111111111111111111111112', // SOL
            outputMint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v', // USDC
            amountIn: '0.1', // 0.1 SOL
            slippage: 1.0
        };
        
        console.log('   Request:', JSON.stringify(swapRequest, null, 2));
        console.log('   ⏳ This may take 30-60 seconds...\n');
        
        const swapRes = await fetch(`${JUPITER_SERVICE_URL}/swap`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(swapRequest),
            signal: AbortSignal.timeout(120000) // 2 minutes
        });
        
        const swapResult = await swapRes.json();
        
        if (swapRes.ok && swapResult.success) {
            console.log('   ✅ Swap successful!');
            console.log('   Provider:', swapResult.provider);
            console.log('   Transaction:', swapResult.transactionSignature);
            console.log('   Input:', swapResult.inputAmount, 'SOL');
            console.log('   Output:', swapResult.outputAmount, 'USDC');
            console.log('\n   🔗 View on Solscan:');
            console.log(`   https://solscan.io/tx/${swapResult.transactionSignature}`);
        } else {
            console.error('   ❌ Swap failed!');
            console.error('   Error:', swapResult.error || swapResult);
        }
        
    } catch (error) {
        console.error('❌ Test error:', error.message);
        if (error.name === 'AbortError') {
            console.error('   Request timed out');
        }
    }
}

testSwap();


