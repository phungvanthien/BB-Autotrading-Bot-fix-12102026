import express from 'express';
import cors from 'cors';
import { Connection, Keypair, VersionedTransaction, PublicKey } from '@solana/web3.js';
import dotenv from 'dotenv';
import bs58 from 'bs58';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

dotenv.config();

const app = express();
const PORT = process.env.PORT || 3002;

app.use(cors());
app.use(express.json());

// Increase timeout for long-running operations (5 minutes)
app.use((req, res, next) => {
    req.setTimeout(300000);
    res.setTimeout(300000);
    next();
});

// Global connection and wallet
let connection = null;
let owner = null;

// Jupiter API endpoints
const JUPITER_QUOTE_API = 'https://lite-api.jup.ag/swap/v1/quote';
const JUPITER_SWAP_API = 'https://lite-api.jup.ag/swap/v1/swap';

// Initialize connection and wallet
async function initialize() {
    try {
        console.log('🔌 Initializing Jupiter Swap Service...');
        
        // Load RPC URL
        const rpcUrl = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
        console.log(`   RPC: ${rpcUrl.replace(/\/v2\/[^/]+/, '/v2/***')}`);
        
        // Initialize connection
        connection = new Connection(rpcUrl, {
            commitment: 'confirmed',
            confirmTransactionInitialTimeout: 60000,
        });
        
        // Load wallet
        const walletPath = process.env.WALLET_PRIVATE_KEY || '~/.config/solana/id.json';
        const expandedPath = walletPath.startsWith('~') 
            ? walletPath.replace('~', process.env.HOME || process.env.USERPROFILE || '')
            : walletPath;
        
        let keypair;
        if (fs.existsSync(expandedPath)) {
            const keyData = JSON.parse(fs.readFileSync(expandedPath, 'utf8'));
            keypair = Keypair.fromSecretKey(new Uint8Array(keyData));
        } else if (walletPath.startsWith('[') || !walletPath.includes('/')) {
            // Try parsing as base58 or JSON array
            try {
                const decoded = bs58.decode(walletPath);
                keypair = Keypair.fromSecretKey(decoded);
            } catch {
                const keyData = JSON.parse(walletPath);
                keypair = Keypair.fromSecretKey(new Uint8Array(keyData));
            }
        } else {
            throw new Error(`Wallet file not found: ${expandedPath}`);
        }
        
        owner = keypair;
        console.log(`   Wallet: ${owner.publicKey.toString()}`);
        
        // Test connection
        const balance = await connection.getBalance(owner.publicKey);
        console.log(`   Balance: ${balance / 1e9} SOL`);
        
        console.log('✅ Jupiter Swap Service initialized successfully');
        return true;
    } catch (error) {
        console.error('❌ Initialization error:', error);
        throw error;
    }
}

// Get token decimals (common tokens cache)
const TOKEN_DECIMALS = {
    'So11111111111111111111111111111111111111112': 9, // SOL
    'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v': 6, // USDC
    'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB': 6, // USDT
};

async function getTokenDecimals(mintAddress) {
    if (TOKEN_DECIMALS[mintAddress]) {
        return TOKEN_DECIMALS[mintAddress];
    }
    
    try {
        const mintInfo = await connection.getParsedAccountInfo(new PublicKey(mintAddress));
        if (mintInfo.value && mintInfo.value.data && typeof mintInfo.value.data === 'object') {
            return mintInfo.value.data.parsed.info.decimals;
        }
    } catch (error) {
        console.warn(`   Could not fetch decimals for ${mintAddress}, using default 9`);
    }
    
    return 9; // Default
}

// Root endpoint
app.get('/', (req, res) => {
    res.json({
        service: 'Jupiter Swap Service',
        version: '1.0.0',
        status: 'running',
        endpoints: {
            health: '/health',
            quote: '/quote (POST)',
            swap: '/swap (POST)',
        },
        wallet: owner ? owner.publicKey.toString() : 'not initialized',
        rpc: connection ? 'connected' : 'not connected',
        documentation: 'See README.md for API usage',
    });
});

// Health check endpoint
app.get('/health', (req, res) => {
    res.json({
        status: 'ok',
        service: 'Jupiter Swap Service',
        wallet: owner ? owner.publicKey.toString() : 'not initialized',
        rpc: connection ? 'connected' : 'not connected',
    });
});

// Helper function to retry fetch with exponential backoff
async function fetchWithRetry(url, options, maxRetries = 3) {
    for (let i = 0; i < maxRetries; i++) {
        try {
            const response = await fetch(url, options);
            return response;
        } catch (error) {
            if (i === maxRetries - 1) throw error;
            const delay = Math.pow(2, i) * 1000; // Exponential backoff: 1s, 2s, 4s
            console.log(`   Retry ${i + 1}/${maxRetries} after ${delay}ms...`);
            await new Promise(resolve => setTimeout(resolve, delay));
        }
    }
}

// Quote endpoint
app.post('/quote', async (req, res) => {
    try {
        const { inputMint, outputMint, amountIn, slippageBps = 50 } = req.body;
        
        if (!inputMint || !outputMint || !amountIn) {
            return res.status(400).json({ error: 'Missing required parameters: inputMint, outputMint, amountIn' });
        }
        
        const inputDecimals = await getTokenDecimals(inputMint);
        const amountInBaseUnits = Math.floor(parseFloat(amountIn) * Math.pow(10, inputDecimals));
        
        const quoteUrl = `${JUPITER_QUOTE_API}?inputMint=${inputMint}&outputMint=${outputMint}&amount=${amountInBaseUnits}&slippageBps=${slippageBps}`;
        
        console.log(`   Fetching quote from: ${quoteUrl}`);
        
        const quoteResponse = await fetchWithRetry(quoteUrl, {
            method: 'GET',
            headers: { 'Accept': 'application/json' },
            signal: AbortSignal.timeout(30000), // 30 seconds timeout
        });
        
        if (!quoteResponse.ok) {
            const errorText = await quoteResponse.text().catch(() => 'Unknown error');
            throw new Error(`Jupiter quote API error (${quoteResponse.status}): ${errorText}`);
        }
        
        const quoteData = await quoteResponse.json();
        
        if (quoteData.error) {
            return res.status(400).json({ error: quoteData.error });
        }
        
        const outputDecimals = await getTokenDecimals(outputMint);
        const outputAmount = Number(quoteData.outAmount) / Math.pow(10, outputDecimals);
        
        res.json({
            inputAmount: amountIn,
            outputAmount: outputAmount.toString(),
            priceImpact: quoteData.priceImpactPct ? (quoteData.priceImpactPct * 100).toFixed(2) + '%' : 'N/A',
            routePlan: quoteData.routePlan,
        });
    } catch (error) {
        console.error('Quote error:', error);
        const errorMsg = error.message || 'Unknown error';
        // Check if it's a DNS/network error
        if (errorMsg.includes('ENOTFOUND') || errorMsg.includes('fetch failed') || error.cause?.code === 'ENOTFOUND') {
            res.status(503).json({ 
                error: 'Cannot connect to Jupiter API. Please check your internet connection or try again later.',
                details: errorMsg
            });
        } else {
            res.status(500).json({ error: errorMsg });
        }
    }
});

// Swap endpoint
app.post('/swap', async (req, res) => {
    try {
        if (!connection || !owner) {
            return res.status(503).json({ error: 'Service not initialized' });
        }
        
        const { inputMint, outputMint, amountIn, slippage = 1.0 } = req.body;
        
        if (!inputMint || !outputMint || !amountIn) {
            return res.status(400).json({ error: 'Missing required parameters: inputMint, outputMint, amountIn' });
        }
        
        console.log(`🔄 Executing swap via Jupiter: ${amountIn} ${inputMint} -> ${outputMint}`);
        
        // Get token decimals
        const inputDecimals = await getTokenDecimals(inputMint);
        const outputDecimals = await getTokenDecimals(outputMint);
        const amountInBaseUnits = Math.floor(parseFloat(amountIn) * Math.pow(10, inputDecimals));
        const slippageBps = Math.floor(slippage * 100);
        
        // Step 1: Get quote from Jupiter
        console.log(`   Step 1: Getting quote from Jupiter...`);
        const quoteUrl = `${JUPITER_QUOTE_API}?inputMint=${inputMint}&outputMint=${outputMint}&amount=${amountInBaseUnits}&slippageBps=${slippageBps}`;
        
        const quoteResponse = await fetchWithRetry(quoteUrl, {
            method: 'GET',
            headers: { 'Accept': 'application/json' },
            signal: AbortSignal.timeout(30000), // 30 seconds timeout
        });
        
        if (!quoteResponse.ok) {
            const errorData = await quoteResponse.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(`Jupiter quote error: ${JSON.stringify(errorData.error || errorData)}`);
        }
        
        const quoteData = await quoteResponse.json();
        
        if (quoteData.error) {
            throw new Error(`Jupiter quote error: ${JSON.stringify(quoteData.error)}`);
        }
        
        const outputAmount = Number(quoteData.outAmount) / Math.pow(10, outputDecimals);
        console.log(`   Quote received: ${outputAmount} ${outputMint}`);
        
        // Step 2: Get swap transaction from Jupiter
        console.log(`   Step 2: Getting swap transaction from Jupiter...`);
        const swapResponse = await fetchWithRetry(JUPITER_SWAP_API, {
            method: 'POST',
            headers: { 
                'Content-Type': 'application/json',
                'Accept': 'application/json',
            },
            body: JSON.stringify({
                quoteResponse: quoteData,
                userPublicKey: owner.publicKey.toString(),
                wrapAndUnwrapSol: true,
                dynamicComputeUnitLimit: true,
                prioritizationFeeLamports: 'auto',
            }),
            signal: AbortSignal.timeout(30000), // 30 seconds timeout
        });
        
        if (!swapResponse.ok) {
            const errorData = await swapResponse.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(`Jupiter swap error: ${JSON.stringify(errorData.error || errorData)}`);
        }
        
        const swapData = await swapResponse.json();
        
        if (swapData.error) {
            throw new Error(`Jupiter swap error: ${JSON.stringify(swapData.error)}`);
        }
        
        // Step 3: Deserialize, sign, and send transaction
        console.log(`   Step 3: Signing and sending transaction...`);
        const swapTransactionBuf = Buffer.from(swapData.swapTransaction, 'base64');
        const transaction = VersionedTransaction.deserialize(swapTransactionBuf);
        
        // Sign the transaction
        transaction.sign([owner]);
        
        // Send the transaction
        const txid = await connection.sendRawTransaction(transaction.serialize(), {
            skipPreflight: false,
            maxRetries: 5,
            preflightCommitment: 'confirmed',
        });
        
        console.log(`   Transaction sent: ${txid}`);
        
        // Wait for confirmation using polling (to avoid WebSocket subscription issues)
        console.log(`   Step 4: Waiting for confirmation...`);
        const maxRetries = 12; // 12 times max wait time
        const retryDelay = 5000; // 5 seconds between checks => 1 minute max wait time
        let confirmed = false;
        let confirmationError = null;
        
        for (let i = 0; i < maxRetries; i++) {
            const status = await connection.getSignatureStatus(txid);
            
            if (status.value) {
                if (status.value.err) {
                    confirmationError = status.value.err;
                    break;
                }
                if (status.value.confirmationStatus === 'confirmed' || status.value.confirmationStatus === 'finalized') {
                    confirmed = true;
                    break;
                }
            }
            
            // Wait before next check
            await new Promise(resolve => setTimeout(resolve, retryDelay));
        }
        
        if (confirmationError) {
            throw new Error(`Transaction failed: ${JSON.stringify(confirmationError)}`);
        }
        
        if (!confirmed) {
            throw new Error(`Transaction confirmation timeout after ${maxRetries} seconds`);
        }
        
        console.log(`   ✅ Swap successful: ${txid}`);
        
        res.json({
            success: true,
            transactionSignature: txid,
            inputAmount: amountIn,
            outputAmount: outputAmount.toString(),
            provider: 'Jupiter',
        });
    } catch (error) {
        console.error('Swap error:', error);
        const errorMsg = error.message || 'Unknown error';
        // Check if it's a DNS/network error
        if (errorMsg.includes('ENOTFOUND') || errorMsg.includes('fetch failed') || error.cause?.code === 'ENOTFOUND') {
            res.status(503).json({
                success: false,
                error: 'Cannot connect to Jupiter API. Please check your internet connection or try again later.',
                details: errorMsg
            });
        } else {
            res.status(500).json({
                success: false,
                error: errorMsg,
            });
        }
    }
});

// Start server
async function start() {
    try {
        await initialize();
        app.listen(PORT, () => {
            console.log(`🚀 Jupiter Swap Service running on http://localhost:${PORT}`);
            console.log(`   Health check: http://localhost:${PORT}/health`);
        });
    } catch (error) {
        console.error('Failed to start server:', error);
        process.exit(1);
    }
}

start();

