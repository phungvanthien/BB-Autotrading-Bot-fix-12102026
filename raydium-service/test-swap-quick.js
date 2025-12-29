// Quick test - chỉ kiểm tra xem endpoint có phản hồi không
const RAYDIUM_SERVICE_URL = 'http://localhost:3001';

async function quickTest() {
    try {
        console.log('🧪 Quick swap test...\n');
        
        const swapRequest = {
            inputMint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
            outputMint: 'So11111111111111111111111111111111111111112',
            amountIn: '0.1', // Rất nhỏ để test nhanh
            slippage: 1.0
        };
        
        console.log('   Sending request...');
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 60000); // 1 minute timeout
        
        const swapRes = await fetch(`${RAYDIUM_SERVICE_URL}/swap`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(swapRequest),
            signal: controller.signal
        });
        
        clearTimeout(timeoutId);
        const swapResult = await swapRes.json();
        
        console.log('   Response status:', swapRes.status);
        console.log('   Response:', JSON.stringify(swapResult, null, 2));
        
    } catch (error) {
        if (error.name === 'AbortError') {
            console.error('   ⏱️  Request timed out (1 minute)');
            console.error('   This is expected - Raydium is retrying. Jupiter fallback should trigger after all retries fail.');
        } else {
            console.error('   ❌ Error:', error.message);
        }
    }
}

quickTest();
