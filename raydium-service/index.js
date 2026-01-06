import express from 'express';
import cors from 'cors';
import { Connection, Keypair } from '@solana/web3.js';
import { Raydium } from '@raydium-io/raydium-sdk-v2';
import dotenv from 'dotenv';
import bs58 from 'bs58';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

dotenv.config();

const app = express();
const PORT = 3001;

app.use(cors());
app.use(express.json());

// Increase timeout for long-running operations (5 minutes)
app.use((req, res, next) => {
    req.setTimeout(300000); // 5 minutes
    res.setTimeout(300000);
    next();
});

// Global raydium instance
let raydium = null;
let connection = null;
let owner = null;

// Retry helper function with exponential backoff and timeout
async function retryWithBackoff(fn, options = {}) {
    const {
        maxRetries = 3,
        initialDelay = 1000,
        maxDelay = 10000,
        backoffFactor = 2,
        timeout = 30000, // 30 seconds default timeout
        retryableErrors = ['fetch vault info error', 'ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'rate limit', '429']
    } = options;
    
    let lastError;
    let delay = initialDelay;
    
    for (let attempt = 0; attempt <= maxRetries; attempt++) {
        try {
            // Add timeout wrapper
            const timeoutPromise = new Promise((_, reject) => {
                setTimeout(() => reject(new Error(`Operation timed out after ${timeout}ms`)), timeout);
            });
            
            return await Promise.race([fn(), timeoutPromise]);
        } catch (error) {
            lastError = error;
            const errorMessage = error.message?.toLowerCase() || error.toString().toLowerCase();
            const isRetryable = retryableErrors.some(retryable => 
                errorMessage.includes(retryable.toLowerCase())
            );
            
            // Also retry on network-related errors
            const isNetworkError = error.code === 'ECONNRESET' || 
                                 error.code === 'ETIMEDOUT' ||
                                 error.message?.includes('network') ||
                                 error.message?.includes('fetch') ||
                                 error.message?.includes('vault');
            
            if (attempt === maxRetries || (!isRetryable && !isNetworkError)) {
                console.error(`   ❌ Final attempt failed: ${error.message}`);
                throw error;
            }
            
            console.log(`   ⚠️  Attempt ${attempt + 1}/${maxRetries + 1} failed: ${error.message}`);
            console.log(`   ⏳ Retrying in ${delay}ms...`);
            
            await new Promise(resolve => setTimeout(resolve, delay));
            delay = Math.min(delay * backoffFactor, maxDelay);
        }
    }
    
    throw lastError;
}

// Initialize Raydium SDK
async function initRaydium() {
    try {
        // Try multiple RPC endpoints, prefer one with API key
        let RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
        
        // If RPC_API_KEY is provided, construct RPC URL based on provider
        if (process.env.RPC_API_KEY && !RPC_URL.includes('api-key') && !RPC_URL.includes('helius') && !RPC_URL.includes('alchemy') && !RPC_URL.includes('quiknode')) {
            // Check if it's Alchemy format or use Helius by default
            if (process.env.RPC_PROVIDER === 'alchemy') {
                RPC_URL = `https://solana-mainnet.g.alchemy.com/v2/${process.env.RPC_API_KEY}`;
                console.log('   Using Alchemy RPC with API key');
            } else {
                // Default to Helius
                RPC_URL = `https://mainnet.helius-rpc.com/?api-key=${process.env.RPC_API_KEY}`;
                console.log('   Using Helius RPC with API key');
            }
        } else if (RPC_URL.includes('api-key') || RPC_URL.includes('helius') || RPC_URL.includes('quiknode') || RPC_URL.includes('alchemy')) {
            console.log('   Using custom RPC endpoint with API key');
        } else {
            console.log('   ⚠️  Using public RPC - may encounter rate limits');
            console.log('   💡 Tip: Add RPC_API_KEY to .env for better performance');
            console.log('   💡 See README-RPC-FIX.md for instructions');
        }
        
        connection = new Connection(RPC_URL, {
            commitment: 'confirmed',
            confirmTransactionInitialTimeout: 60000,
            httpHeaders: {
                'Content-Type': 'application/json',
            },
        });
        
        // Load keypair from env
        const PRIVATE_KEY = process.env.WALLET_PRIVATE_KEY;
        if (!PRIVATE_KEY) {
            throw new Error('WALLET_PRIVATE_KEY not found in .env');
        }
        
        let keypairBytes;
        if (PRIVATE_KEY.startsWith('[')) {
            // JSON array format
            keypairBytes = JSON.parse(PRIVATE_KEY);
        } else if (PRIVATE_KEY.startsWith('/') || PRIVATE_KEY.startsWith('.') || PRIVATE_KEY.startsWith('~')) {
            // File path - read file
            const keypairPath = PRIVATE_KEY.replace('~', process.env.HOME || process.env.USERPROFILE || '');
            const keypairData = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
            keypairBytes = keypairData;
        } else {
            // Base58 string
            keypairBytes = Array.from(bs58.decode(PRIVATE_KEY));
        }
        
        owner = Keypair.fromSecretKey(Uint8Array.from(keypairBytes));
        
        console.log('🔌 Initializing Raydium SDK...');
        console.log('   Wallet:', owner.publicKey.toString());
        console.log('   RPC:', RPC_URL);
        
        raydium = await Raydium.load({
            connection,
            owner,
            disableLoadToken: false,
        });
        
        // Debug: Log available methods
        console.log('📋 Raydium SDK loaded. Available methods:', Object.keys(raydium));
        if (raydium.trade) {
            console.log('   Trade methods:', Object.keys(raydium.trade));
        }
        if (raydium.tradeV2) {
            console.log('   TradeV2 methods:', Object.keys(raydium.tradeV2));
            console.log('   TradeV2.swap type:', typeof raydium.tradeV2.swap);
            if (typeof raydium.tradeV2.swap === 'function') {
                console.log('   TradeV2.swap is a function');
            }
        }
        if (raydium.cpmm) {
            console.log('   CPMM methods:', Object.keys(raydium.cpmm));
        }
        
        console.log('✅ Raydium SDK initialized successfully');
        return true;
    } catch (error) {
        console.error('❌ Failed to initialize Raydium SDK:', error);
        return false;
    }
}

// Health check endpoint
app.get('/health', (req, res) => {
    res.json({ 
        status: 'ok', 
        raydium_initialized: raydium !== null,
        wallet: owner ? owner.publicKey.toString() : null
    });
});

// Get quote endpoint
app.post('/quote', async (req, res) => {
    try {
        if (!raydium) {
            return res.status(503).json({ error: 'Raydium SDK not initialized' });
        }
        
        const { inputMint, outputMint, amountIn, slippage = 1.0 } = req.body;
        
        if (!inputMint || !outputMint || !amountIn) {
            return res.status(400).json({ error: 'Missing required parameters: inputMint, outputMint, amountIn' });
        }
        
        console.log(`📊 Getting quote: ${amountIn} ${inputMint} -> ${outputMint}`);
        
        // Use Raydium SDK to get quote
        const { PublicKey } = await import('@solana/web3.js');
        const { Percent } = await import('@raydium-io/raydium-sdk-v2');
        
        const inputMintPubkey = new PublicKey(inputMint);
        const outputMintPubkey = new PublicKey(outputMint);
        const slippagePercent = new Percent(Math.floor(slippage * 100), 10000);
        
        // Get token decimals - try both PublicKey and string
        let inputToken = raydium.token.tokenMap.get(inputMint);
        let outputToken = raydium.token.tokenMap.get(outputMint);
        if (!inputToken) {
            inputToken = raydium.token.tokenMap.get(inputMintPubkey.toString());
        }
        if (!outputToken) {
            outputToken = raydium.token.tokenMap.get(outputMintPubkey.toString());
        }
        
        if (!inputToken || !outputToken) {
            throw new Error(`Token info not found for ${inputMint} or ${outputMint}`);
        }
        
        const inputDecimals = inputToken.decimals;
        const amountInBaseUnits = BigInt(Math.floor(parseFloat(amountIn) * Math.pow(10, inputDecimals)));
        
        // Use tradeV2 for quote (simpler API, no need to find pool first)
        if (!raydium.tradeV2 || !raydium.tradeV2.swap) {
            throw new Error('Raydium SDK does not have tradeV2.swap method. Available: ' + Object.keys(raydium.tradeV2 || {}).join(', '));
        }
        
        // Get quote using tradeV2 (don't execute, just calculate)
        // Convert BigInt to string for SDK compatibility
        const amountInStr = amountInBaseUnits.toString();
        const swapResult = await raydium.tradeV2.swap({
            inputMint: inputMintPubkey,
            outputMint: outputMintPubkey,
            amountIn: amountInStr, // Use string instead of BigInt
            slippage: slippagePercent,
            fixedSide: 'in',
        });
        
        // Calculate output amount in human-readable format
        const outputDecimals = outputToken.decimals;
        const outputAmount = swapResult.amountOut ? Number(swapResult.amountOut) / Math.pow(10, outputDecimals) : 0;
        
        res.json({
            success: true,
            inputAmount: amountIn,
            outputAmount: outputAmount.toString(),
            priceImpact: '0', // tradeV2 doesn't always provide price impact in quote
            route: 'Raydium TradeV2',
        });
    } catch (error) {
        console.error('Quote error:', error);
        res.status(500).json({ error: error.message });
    }
});

// Swap endpoint
app.post('/swap', async (req, res) => {
    try {
        if (!raydium) {
            return res.status(503).json({ error: 'Raydium SDK not initialized' });
        }
        
        const { inputMint, outputMint, amountIn, slippage = 1.0 } = req.body;
        
        if (!inputMint || !outputMint || !amountIn) {
            return res.status(400).json({ error: 'Missing required parameters: inputMint, outputMint, amountIn' });
        }
        
        console.log(`🔄 Executing swap: ${amountIn} ${inputMint} -> ${outputMint}`);
        
        const { PublicKey } = await import('@solana/web3.js');
        const { Percent, TokenAmount } = await import('@raydium-io/raydium-sdk-v2');
        
        const inputMintPubkey = new PublicKey(inputMint);
        const outputMintPubkey = new PublicKey(outputMint);
        const slippagePercent = new Percent(Math.floor(slippage * 100), 10000);
        
        // Get token decimals first (needed for all swap methods)
        let inputToken = raydium.token.tokenMap.get(inputMint);
        let outputToken = raydium.token.tokenMap.get(outputMint);
        if (!inputToken) {
            inputToken = raydium.token.tokenMap.get(inputMintPubkey.toString());
        }
        if (!outputToken) {
            outputToken = raydium.token.tokenMap.get(outputMintPubkey.toString());
        }
        
        if (!inputToken || !outputToken) {
            throw new Error(`Token info not found for ${inputMint} or ${outputMint}`);
        }
        
        const inputDecimals = inputToken.decimals;
        const outputDecimals = outputToken.decimals;
        const amountInBaseUnits = BigInt(Math.floor(parseFloat(amountIn) * Math.pow(10, inputDecimals)));
        
        // Try tradeV2 first (most common, doesn't need pool)
        let swapResult;
        if (raydium.tradeV2 && raydium.tradeV2.swap) {
            console.log(`   Using tradeV2.swap`);
            console.log(`   Input: ${amountInBaseUnits.toString()} (${amountIn} UI)`);
            console.log(`   Slippage: ${slippage}%`);
            
            try {
                // Import required types
                const { TokenAmount } = await import('@raydium-io/raydium-sdk-v2');
                
                // Step 1: Get pools for the route using fetchPoolByMints (with retry)
                console.log(`   Step 1: Fetching pools for token pair...`);
                let ammPools = [];
                
                try {
                    // Try fetchPoolByMints first - this should return pools for our specific token pair
                    const poolResult = await retryWithBackoff(async () => {
                        return await raydium.api.fetchPoolByMints({
                            mint1: inputMintPubkey.toString(),
                            mint2: outputMintPubkey.toString(),
                        });
                    }, {
                        maxRetries: 3,
                        initialDelay: 1000,
                        retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'rate limit', 'fetch']
                    });
                    
                    console.log(`   fetchPoolByMints result type: ${typeof poolResult}, isArray: ${Array.isArray(poolResult)}`);
                    
                    // Handle different response formats
                    if (Array.isArray(poolResult)) {
                        ammPools = poolResult;
                    } else if (poolResult && Array.isArray(poolResult.data)) {
                        ammPools = poolResult.data;
                    } else if (poolResult && Array.isArray(poolResult.pools)) {
                        ammPools = poolResult.pools;
                    } else if (poolResult && typeof poolResult === 'object') {
                        // Try to extract pools from object
                        ammPools = Object.values(poolResult).filter(Array.isArray).flat() || [];
                        if (ammPools.length === 0 && poolResult.pool) {
                            // Single pool object
                            ammPools = [poolResult.pool];
                        }
                    }
                    
                    console.log(`   Fetched ${ammPools.length} pool(s) from fetchPoolByMints`);
                    if (ammPools.length > 0) {
                        // Debug: log first pool structure
                        const firstPool = ammPools[0];
                        console.log(`   First pool keys: ${Object.keys(firstPool).join(', ')}`);
                        console.log(`   First pool mintA type: ${typeof firstPool.mintA}, value: ${firstPool.mintA}`);
                        if (firstPool.mintA && typeof firstPool.mintA === 'object') {
                            console.log(`   First pool mintA keys: ${Object.keys(firstPool.mintA).join(', ')}`);
                            console.log(`   First pool mintA toString: ${firstPool.mintA.toString ? firstPool.mintA.toString() : 'N/A'}`);
                            console.log(`   First pool mintA address: ${firstPool.mintA.address || firstPool.mintA.toBase58 || 'N/A'}`);
                        }
                    }
                } catch (e) {
                    console.log(`   fetchPoolByMints failed: ${e.message}, trying getPoolList...`);
                    try {
                        // Fallback to getPoolList
                        const poolListResult = await raydium.api.getPoolList({});
                        if (Array.isArray(poolListResult)) {
                            ammPools = poolListResult;
                        } else if (poolListResult && Array.isArray(poolListResult.data)) {
                            ammPools = poolListResult.data;
                        }
                        console.log(`   Fetched ${ammPools.length} pool(s) from getPoolList`);
                    } catch (e2) {
                        console.log(`   getPoolList also failed: ${e2.message}`);
                    }
                }
                
                // Convert mint addresses to PublicKey objects if needed
                // The SDK expects pools with mintA/mintB as PublicKey objects with equals() method
                const normalizedPools = ammPools.map(pool => {
                    if (!pool) return null;
                    try {
                        // Create a new object to avoid mutating the original
                        const normalized = Object.assign({}, pool);
                        
                        // Helper to safely convert to PublicKey
                        const toPublicKey = (value) => {
                            if (!value) return value;
                            // If it's already a PublicKey with equals method, return as-is
                            if (value.equals && typeof value.equals === 'function') {
                                return value;
                            }
                            // If it's a string, convert to PublicKey
                            if (typeof value === 'string') {
                                return new PublicKey(value);
                            }
                            // If it's an object with an address property (token info object)
                            if (typeof value === 'object' && value.address) {
                                return new PublicKey(value.address);
                            }
                            // If it has toString, try converting
                            if (value.toString && typeof value.toString === 'function') {
                                const str = value.toString();
                                // Only convert if it looks like a base58 string (not [object Object])
                                if (str.length > 20 && str.length < 50 && !str.includes('object')) {
                                    return new PublicKey(str);
                                }
                            }
                            // Return as-is if we can't convert
                            return value;
                        };
                        
                        // Handle mintA/mintB format (most common)
                        if (normalized.mintA !== undefined) {
                            normalized.mintA = toPublicKey(normalized.mintA);
                        }
                        if (normalized.mintB !== undefined) {
                            normalized.mintB = toPublicKey(normalized.mintB);
                        }
                        
                        // Handle baseMint/quoteMint format
                        if (normalized.baseMint !== undefined) {
                            normalized.baseMint = toPublicKey(normalized.baseMint);
                        }
                        if (normalized.quoteMint !== undefined) {
                            normalized.quoteMint = toPublicKey(normalized.quoteMint);
                        }
                        
                        // Handle id field (pool ID) - also needs to be PublicKey
                        if (normalized.id !== undefined) {
                            normalized.id = toPublicKey(normalized.id);
                        }
                        
                        // Handle programId if present
                        if (normalized.programId !== undefined) {
                            normalized.programId = toPublicKey(normalized.programId);
                        }
                        
                        return normalized;
                    } catch (e) {
                        // If conversion fails, try to use pool as-is (might already be correct format)
                        console.log(`   Warning: Could not normalize pool, using as-is: ${e.message}`);
                        return pool;
                    }
                }).filter(p => p !== null);
                
                console.log(`   Normalized ${normalizedPools.length} pools (from ${ammPools.length} original)`);
                if (normalizedPools.length > 0) {
                    const first = normalizedPools[0];
                    console.log(`   Sample pool - mintA type: ${typeof first.mintA}, has equals: ${first.mintA?.equals ? 'yes' : 'no'}`);
                }
                
                // Filter and sort pools by quality (TVL, volume, etc.)
                // Prioritize pools with higher TVL/liquidity
                const sortedPools = normalizedPools.sort((a, b) => {
                    const tvlA = a.tvl || a.liquidity || 0;
                    const tvlB = b.tvl || b.liquidity || 0;
                    return tvlB - tvlA; // Sort descending
                });
                
                // Try with progressively fewer pools if vault fetching fails
                // Also try different pool combinations to avoid problematic pools
                const poolBatchSizes = [10, 5, 3, 1];
                let routes = null;
                let routesData = null;
                let lastError = null;
                let failedPoolIndices = new Set(); // Track which pool indices failed
                
                for (let batchAttempt = 0; batchAttempt < poolBatchSizes.length; batchAttempt++) {
                    const batchSize = poolBatchSizes[batchAttempt];
                    
                    // Skip pools that previously failed, try different combinations
                    const availablePools = sortedPools.filter((_, idx) => !failedPoolIndices.has(idx));
                    if (availablePools.length === 0) {
                        console.log(`   No more pools to try, all have failed`);
                        break;
                    }
                    
                    const poolsToUse = availablePools.slice(0, batchSize);
                    console.log(`   Trying batch ${batchAttempt + 1}/${poolBatchSizes.length} with ${poolsToUse.length} pools (${availablePools.length} available)...`);
                    
                    try {
                        // Step 2: Get routes - pass normalized pools (with retry)
                        console.log(`   Step 2: Getting routes with ${poolsToUse.length} pools...`);
                        routes = await retryWithBackoff(async () => {
                            return await raydium.tradeV2.getAllRoute({
                                inputMint: inputMintPubkey,
                                outputMint: outputMintPubkey,
                                clmmPools: [],
                                ammPools: poolsToUse,
                                cpmmPools: [],
                            });
                        }, {
                            maxRetries: 2,
                            initialDelay: 1000,
                            retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network']
                        });
                        
                        if (!routes || !routes.directPath || routes.directPath.length === 0) {
                            console.log(`   No routes found with ${poolsToUse.length} pools, trying next batch...`);
                            // Mark these pools as potentially problematic
                            poolsToUse.forEach((_, idx) => {
                                const originalIdx = sortedPools.indexOf(poolsToUse[idx]);
                                if (originalIdx >= 0) failedPoolIndices.add(originalIdx);
                            });
                            continue;
                        }
                        
                        console.log(`   Found route with ${routes.directPath.length} pool(s)`);
                        
                        // Step 3: Fetch swap routes data (with retry - this is where vault errors occur)
                        console.log(`   Step 3: Fetching swap routes data with ${poolsToUse.length} pools...`);
                        console.log(`   ⚠️  This step may take time due to RPC rate limits...`);
                        routesData = await retryWithBackoff(async () => {
                            // Add delay before fetch to avoid rate limiting
                            await new Promise(resolve => setTimeout(resolve, 1000));
                            return await raydium.tradeV2.fetchSwapRoutesData({
                                routes,
                                inputMint: inputMintPubkey,
                                outputMint: outputMintPubkey,
                            });
                        }, {
                            maxRetries: 5, // More retries for vault fetching
                            initialDelay: 5000, // Longer initial delay for RPC (5 seconds)
                            maxDelay: 20000, // Max 20 seconds between retries
                            timeout: 90000, // 90 seconds timeout for vault fetching
                            retryableErrors: ['fetch vault info error', 'ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'rate limit', 'vault', '429', '500', '502', '503', '504', 'fetch']
                        });
                        
                        // Success! Break out of loop
                        console.log(`   ✅ Successfully fetched routes data with ${poolsToUse.length} pools`);
                        break;
                        
                    } catch (error) {
                        lastError = error;
                        const errorMsg = error.message?.toLowerCase() || '';
                        const isVaultError = errorMsg.includes('vault') || errorMsg.includes('fetch vault info');
                        
                        // Extract vault address from error if possible
                        const vaultMatch = error.message?.match(/11111[\w]+/);
                        if (vaultMatch) {
                            console.log(`   ⚠️  Problematic vault detected: ${vaultMatch[0]}`);
                        }
                        
                        if (isVaultError) {
                            // Mark current pools as failed and try different ones
                            poolsToUse.forEach((pool) => {
                                const originalIdx = sortedPools.indexOf(pool);
                                if (originalIdx >= 0) {
                                    failedPoolIndices.add(originalIdx);
                                    console.log(`   Marking pool ${originalIdx} as failed`);
                                }
                            });
                            
                            // Check if there are more pools to try
                            const remainingPools = sortedPools.filter((_, idx) => !failedPoolIndices.has(idx));
                            
                            if (remainingPools.length > 0 && batchAttempt < poolBatchSizes.length - 1) {
                                console.log(`   ⚠️  Vault error with ${poolsToUse.length} pools, ${remainingPools.length} pools remaining, trying next batch...`);
                                // Add delay before trying next batch
                                await new Promise(resolve => setTimeout(resolve, 3000));
                                continue;
                            } else if (remainingPools.length > 0) {
                                // Last batch size, but try with remaining pools
                                console.log(`   ⚠️  Last batch size failed, but ${remainingPools.length} pools remaining, trying them...`);
                                // Try with remaining pools one by one
                                for (let i = 0; i < Math.min(remainingPools.length, 5); i++) {
                                    const singlePool = [remainingPools[i]];
                                    console.log(`   Trying single pool ${i + 1}/${Math.min(remainingPools.length, 5)}...`);
                                    try {
                                        routes = await raydium.tradeV2.getAllRoute({
                                            inputMint: inputMintPubkey,
                                            outputMint: outputMintPubkey,
                                            clmmPools: [],
                                            ammPools: singlePool,
                                            cpmmPools: [],
                                        });
                                        
                                        if (routes && routes.directPath && routes.directPath.length > 0) {
                                            routesData = await retryWithBackoff(async () => {
                                                return await raydium.tradeV2.fetchSwapRoutesData({
                                                    routes,
                                                    inputMint: inputMintPubkey,
                                                    outputMint: outputMintPubkey,
                                                });
                                            }, {
                                                maxRetries: 2,
                                                initialDelay: 3000,
                                                timeout: 50000,
                                                retryableErrors: ['fetch vault info error', 'ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'rate limit', 'vault', '429', '500', '502', '503', '504']
                                            });
                                            
                                            console.log(`   ✅ Successfully fetched routes data with single pool!`);
                                            break;
                                        }
                                    } catch (singleError) {
                                        const originalIdx = sortedPools.indexOf(singlePool[0]);
                                        if (originalIdx >= 0) failedPoolIndices.add(originalIdx);
                                        console.log(`   Pool ${i + 1} also failed, continuing...`);
                                        if (i < Math.min(remainingPools.length, 5) - 1) {
                                            await new Promise(resolve => setTimeout(resolve, 2000));
                                        }
                                    }
                                }
                                
                                if (routes && routesData) {
                                    break; // Success!
                                }
                            }
                        }
                        
                        // If not vault error or no more pools, check if we should throw or use Jupiter
                        console.log(`   ❌ Error with ${poolsToUse.length} pools: ${error.message}`);
                        if (batchAttempt === poolBatchSizes.length - 1) {
                            // Last batch failed, but if it's a vault error, we'll try Jupiter in the outer catch
                            if (isVaultError) {
                                throw error; // Will be caught by outer catch and trigger Jupiter fallback
                            }
                            throw error;
                        }
                    }
                }
                
                if (!routes || !routesData) {
                    // If all attempts failed with vault errors, throw to trigger Jupiter fallback
                    if (lastError && (lastError.message?.toLowerCase().includes('vault') || lastError.message?.toLowerCase().includes('fetch vault info'))) {
                        throw lastError;
                    }
                    throw lastError || new Error(`Failed to fetch routes data after trying all pool combinations. ${failedPoolIndices.size} pools marked as problematic.`);
                }
                
                // Step 4: Get current chain time and epoch info (with retry)
                console.log(`   Step 4: Getting chain info...`);
                const slot = await retryWithBackoff(async () => {
                    return await connection.getSlot();
                }, {
                    maxRetries: 3,
                    initialDelay: 500,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network']
                });
                
                const epochInfo = await retryWithBackoff(async () => {
                    return await connection.getEpochInfo();
                }, {
                    maxRetries: 3,
                    initialDelay: 500,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network']
                });
                
                const blockTime = await retryWithBackoff(async () => {
                    const time = await connection.getBlockTime(slot);
                    return time || Math.floor(Date.now() / 1000);
                }, {
                    maxRetries: 3,
                    initialDelay: 500,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network']
                });
                
                // Step 5: Compute amount out (with retry)
                console.log(`   Step 5: Computing swap info...`);
                const inputTokenAmount = new TokenAmount(inputToken, amountInBaseUnits.toString());
                const swapInfoList = await retryWithBackoff(async () => {
                    return await raydium.tradeV2.getAllRouteComputeAmountOut({
                        inputTokenAmount,
                        outputToken,
                        directPath: routes.directPath,
                        routePathDict: routesData.routePathDict,
                        simulateCache: routesData.ammSimulateCache,
                        tickCache: routesData.computePoolTickData,
                        mintInfos: routesData.mintInfos,
                        slippage: slippage / 100, // Convert percentage to decimal (1% = 0.01)
                        chainTime: blockTime,
                        epochInfo: epochInfo,
                    });
                }, {
                    maxRetries: 3,
                    initialDelay: 1000,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'compute']
                });
                
                if (!swapInfoList || swapInfoList.length === 0) {
                    throw new Error('Failed to compute swap amount out');
                }
                
                const swapInfo = swapInfoList[0]; // Use first/best route
                console.log(`   Computed output: ${swapInfo.amountOut.toExact()}`);
                
                // Step 6: Get pool keys (with retry)
                console.log(`   Step 6: Getting pool keys...`);
                const poolKeys = await retryWithBackoff(async () => {
                    return await raydium.tradeV2.computePoolToPoolKeys({
                        pools: swapInfo.poolInfoList,
                        clmmRpcData: routesData.clmmPoolsRpcInfo,
                        ammRpcData: routesData.ammPoolsRpcInfo,
                    });
                }, {
                    maxRetries: 3,
                    initialDelay: 1000,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network']
                });
                
                // Step 7: Execute swap (with retry)
                console.log(`   Step 7: Executing swap...`);
                swapResult = await retryWithBackoff(async () => {
                    return await raydium.tradeV2.swap({
                        swapInfo: {
                            ...swapInfo,
                            outputMint: outputMintPubkey,
                        },
                        swapPoolKeys: poolKeys,
                        ownerInfo: {
                            wallet: owner.publicKey,
                            associatedOnly: false,
                            checkCreateATAOwner: true,
                        },
                        txVersion: 'V0',
                    });
                }, {
                    maxRetries: 3,
                    initialDelay: 1000,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'transaction']
                });
                
                // Debug: Log swapResult structure
                console.log(`   Swap result type: ${typeof swapResult}`);
                console.log(`   Swap result is null/undefined: ${swapResult == null}`);
                if (swapResult) {
                    console.log(`   Swap result keys: ${Object.keys(swapResult).join(', ')}`);
                    console.log(`   Has execute(): ${typeof swapResult.execute === 'function'}`);
                    console.log(`   Has transaction: ${!!swapResult.transaction}`);
                    console.log(`   Has amountOut: ${!!swapResult.amountOut}`);
                    console.log(`   Has sendTx: ${typeof swapResult.sendTx === 'function'}`);
                    console.log(`   Has buildTx: ${typeof swapResult.buildTx === 'function'}`);
                    // Log full structure (limited depth)
                    try {
                        const resultStr = JSON.stringify(swapResult, (key, value) => {
                            if (typeof value === 'function') return '[Function]';
                            if (value instanceof Error) return value.message;
                            return value;
                        }, 2);
                        console.log(`   Full swapResult structure:\n${resultStr.substring(0, 1000)}`);
                    } catch (e) {
                        console.log(`   Could not stringify swapResult: ${e.message}`);
                    }
                }
            } catch (swapError) {
                console.error(`   ❌ tradeV2.swap error:`, swapError);
                throw swapError;
            }
        } else if (raydium.clmm && raydium.clmm.swap) {
            // CLMM needs pool, so find it first
            console.log(`   Using clmm.swap (need to find pool)`);
            
            let pools = [];
            
            // Method 1: Try fetchPoolByMints
            try {
                const poolResult = await raydium.api.fetchPoolByMints({
                    mint1: inputMintPubkey.toString(),
                    mint2: outputMintPubkey.toString(),
                });
                if (Array.isArray(poolResult)) {
                    pools = poolResult;
                } else if (poolResult && typeof poolResult === 'object') {
                    pools = poolResult.pools || poolResult.data || [poolResult];
                }
            } catch (e) {
                console.log('   fetchPoolByMints error:', e.message);
            }
            
            // Method 2: Try getPoolList if no pools found
            if (pools.length === 0 && raydium.api.getPoolList) {
                try {
                    const allPools = await raydium.api.getPoolList();
                    pools = allPools.filter(p => 
                        (p.baseMint && (p.baseMint.equals(inputMintPubkey) || p.baseMint.equals(outputMintPubkey))) &&
                        (p.quoteMint && (p.quoteMint.equals(inputMintPubkey) || p.quoteMint.equals(outputMintPubkey)))
                    );
                } catch (e) {
                    console.log('   getPoolList error:', e.message);
                }
            }
            
            if (pools.length === 0) {
                throw new Error(`No pool found for ${inputMint} -> ${outputMint}. CLMM requires a pool.`);
            }
            
            const pool = pools[0];
            console.log(`   Found pool: ${pool.id || pool.address || 'N/A'}`);
            const baseIn = inputMintPubkey.equals(pool.baseMint || pool.mintA);
            swapResult = await raydium.clmm.swap({
                poolInfo: pool,
                baseIn: baseIn,
                inputAmount: amountInBaseUnits,
                slippage: slippagePercent,
            });
        } else {
            throw new Error('Neither tradeV2.swap nor clmm.swap is available. Available: ' + 
                JSON.stringify({
                    tradeV2: Object.keys(raydium.tradeV2 || {}),
                    clmm: Object.keys(raydium.clmm || {}),
                    cpmm: Object.keys(raydium.cpmm || {})
                }));
        }
        
        // Execute swap - tradeV2 returns { execute } function, clmm returns { transaction, signers }
        let txId;
        console.log('   Attempting to execute swap...');
        console.log(`   swapResult structure:`, JSON.stringify({
            hasExecute: typeof swapResult?.execute === 'function',
            hasTransaction: !!swapResult?.transaction,
            hasSigners: !!swapResult?.signers,
            keys: Object.keys(swapResult || {})
        }, null, 2));
        
        if (typeof swapResult.execute === 'function') {
            // tradeV2 style - execute() method (with retry)
            console.log('   Executing via tradeV2.execute()');
            try {
                txId = await retryWithBackoff(async () => {
                    return await swapResult.execute();
                }, {
                    maxRetries: 3,
                    initialDelay: 2000,
                    maxDelay: 10000,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'blockhash', 'transaction', '429']
                });
                console.log(`   ✅ Execute completed, txId: ${txId}`);
            } catch (executeError) {
                console.error(`   ❌ Execute error after retries:`, executeError);
                throw new Error(`Failed to execute swap: ${executeError.message}`);
            }
        } else if (typeof swapResult.sendTx === 'function') {
            // Alternative: sendTx() method (with retry)
            console.log('   Executing via tradeV2.sendTx()');
            try {
                txId = await retryWithBackoff(async () => {
                    return await swapResult.sendTx();
                }, {
                    maxRetries: 3,
                    initialDelay: 2000,
                    maxDelay: 10000,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'blockhash', 'transaction', '429']
                });
                console.log(`   ✅ SendTx completed, txId: ${txId}`);
            } catch (sendTxError) {
                console.error(`   ❌ SendTx error after retries:`, sendTxError);
                throw new Error(`Failed to sendTx swap: ${sendTxError.message}`);
            }
        } else if (swapResult.transaction) {
            // clmm/cpmm style - need to send transaction manually (with retry)
            console.log('   Executing via sendAndConfirmTransaction');
            const { sendAndConfirmTransaction } = await import('@solana/web3.js');
            try {
                txId = await retryWithBackoff(async () => {
                    return await sendAndConfirmTransaction(
                        connection,
                        swapResult.transaction,
                        swapResult.signers || [owner],
                        { 
                            commitment: 'confirmed',
                            skipPreflight: false,
                            maxRetries: 3
                        }
                    );
                }, {
                    maxRetries: 3,
                    initialDelay: 2000,
                    maxDelay: 10000,
                    retryableErrors: ['ECONNRESET', 'ETIMEDOUT', 'timeout', 'network', 'blockhash', 'transaction', '429']
                });
                console.log(`   ✅ Transaction sent, txId: ${txId}`);
            } catch (txError) {
                console.error(`   ❌ Transaction error after retries:`, txError);
                throw new Error(`Failed to send transaction: ${txError.message}`);
            }
        } else {
            // Log full swapResult for debugging
            console.error('   ❌ Swap result structure:');
            console.error('   ', JSON.stringify(swapResult, null, 2));
            throw new Error('Swap result does not have execute() method or transaction property. Result keys: ' + Object.keys(swapResult || {}).join(', '));
        }
        
        // Calculate output amount in human-readable format
        const outputAmount = swapResult.amountOut ? Number(swapResult.amountOut) / Math.pow(10, outputDecimals) : 0;
        
        console.log(`✅ Swap successful: ${txId}`);
        console.log(`   Output: ${outputAmount} ${outputMint}`);
        
        res.json({
            success: true,
            transactionSignature: txId.toString(),
            inputAmount: amountIn,
            outputAmount: outputAmount.toString(),
        });
    } catch (error) {
        console.error('Swap error:', error);
        
        // Try Jupiter API as fallback for various Raydium errors
        const errorMsg = error.message?.toLowerCase() || '';
        const shouldUseJupiter = 
            errorMsg.includes('vault') || 
            errorMsg.includes('fetch vault info') ||
            errorMsg.includes('failed to compute swap amount out') ||
            errorMsg.includes('failed to fetch routes') ||
            errorMsg.includes('no pool found') ||
            errorMsg.includes('timeout') ||
            errorMsg.includes('network') ||
            errorMsg.includes('rate limit') ||
            errorMsg.includes('429');
        
        if (shouldUseJupiter) {
            console.log(`   ⚠️  Raydium error detected (${error.message}), trying Jupiter API as fallback...`);
            try {
                return await swapViaJupiter(req, res, inputMintPubkey, outputMintPubkey, amountIn, slippage);
            } catch (jupiterError) {
                console.error('   Jupiter fallback also failed:', jupiterError.message);
                res.status(500).json({ 
                    success: false,
                    error: `Raydium failed: ${error.message}. Jupiter fallback also failed: ${jupiterError.message}` 
                });
                return;
            }
        }
        
        res.status(500).json({ success: false, error: error.message });
    }
});

// Jupiter API fallback swap function
async function swapViaJupiter(req, res, inputMintPubkey, outputMintPubkey, amountIn, slippage) {
    console.log('   🔄 Using Jupiter API fallback...');
    
    const { PublicKey } = await import('@solana/web3.js');
    const { Transaction, VersionedTransaction } = await import('@solana/web3.js');
    
    // Get token decimals
    let inputToken = raydium.token.tokenMap.get(inputMintPubkey.toString());
    if (!inputToken) {
        inputToken = raydium.token.tokenMap.get(inputMintPubkey.toString());
    }
    
    const inputDecimals = inputToken?.decimals || 6;
    const amountInBaseUnits = Math.floor(parseFloat(amountIn) * Math.pow(10, inputDecimals));
    const slippageBps = Math.floor(slippage * 100);
    
    // Step 1: Get quote from Jupiter
    const quoteUrl = `https://lite-api.jup.ag/swap/v1/quote?inputMint=${inputMintPubkey.toString()}&outputMint=${outputMintPubkey.toString()}&amount=${amountInBaseUnits}&slippageBps=${slippageBps}`;
    
    console.log(`   Getting Jupiter quote: ${quoteUrl}`);
    const quoteResponse = await fetch(quoteUrl);
    const quoteData = await quoteResponse.json();
    
    if (quoteData.error) {
        throw new Error(`Jupiter quote error: ${JSON.stringify(quoteData.error)}`);
    }
    
    console.log(`   Jupiter quote received: ${quoteData.outAmount} output tokens`);
    
    // Step 2: Get swap transaction
    const swapResponse = await fetch('https://lite-api.jup.ag/swap/v1/swap', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            quoteResponse: quoteData,
            userPublicKey: owner.publicKey.toString(),
            wrapAndUnwrapSol: true,
            dynamicComputeUnitLimit: true,
            prioritizationFeeLamports: 'auto',
        }),
    });
    
    const swapData = await swapResponse.json();
    
    if (swapData.error) {
        throw new Error(`Jupiter swap error: ${JSON.stringify(swapData.error)}`);
    }
    
    // Step 3: Deserialize and sign transaction
    const swapTransactionBuf = Buffer.from(swapData.swapTransaction, 'base64');
    const transaction = VersionedTransaction.deserialize(swapTransactionBuf);
    
    // Sign transaction
    transaction.sign([owner]);
    
    // Step 4: Send transaction
    const txid = await connection.sendRawTransaction(transaction.serialize(), {
        skipPreflight: false,
        maxRetries: 3,
    });
    
    // Wait for confirmation
    await connection.confirmTransaction(txid, 'confirmed');
    
    console.log(`   ✅ Jupiter swap successful: ${txid}`);
    
    // Calculate output amount
    const outputDecimals = raydium.token.tokenMap.get(outputMintPubkey.toString())?.decimals || 9;
    const outputAmount = Number(quoteData.outAmount) / Math.pow(10, outputDecimals);
    
    res.json({
        success: true,
        transactionSignature: txid,
        inputAmount: amountIn,
        outputAmount: outputAmount.toString(),
        provider: 'Jupiter (fallback)',
    });
}

// Start server
async function startServer() {
    const initialized = await initRaydium();
    if (!initialized) {
        console.error('Failed to initialize Raydium SDK. Server will not start.');
        process.exit(1);
    }
    
    app.listen(PORT, () => {
        console.log(`🚀 Raydium Swap Service running on http://localhost:${PORT}`);
        console.log(`   Health check: http://localhost:${PORT}/health`);
    });
}

startServer().catch(console.error);

