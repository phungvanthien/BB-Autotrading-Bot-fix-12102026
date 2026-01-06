import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import dotenv from 'dotenv';
import bs58 from 'bs58';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

dotenv.config();

// Common token symbols for display (optional, will use mint address if not found)
const TOKEN_SYMBOLS = {
    'So11111111111111111111111111111111111111112': 'SOL',
    'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v': 'USDC',
    'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB': 'USDT',
};

const TOKEN_PROGRAM_ID = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');

// Get SOL price from Binance
async function getSOLPrice() {
    try {
        const url = 'https://api.binance.com/api/v3/ticker/price?symbol=SOLUSDT';
        const response = await fetch(url);
        if (!response.ok) {
            throw new Error(`Binance API error: ${response.status}`);
        }
        const data = await response.json();
        return parseFloat(data.price);
    } catch (error) {
        console.warn(`⚠️  Failed to get SOL price from Binance: ${error.message}`);
        return null;
    }
}

// Load wallet keypair
function loadWallet() {
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
    
    return keypair;
}

// Get all token accounts for a wallet
async function getAllTokenBalances(connection, walletPublicKey) {
    try {
        const tokenAccounts = await connection.getParsedTokenAccountsByOwner(
            walletPublicKey,
            {
                programId: TOKEN_PROGRAM_ID,
            }
        );

        const balances = [];
        
        for (const accountInfo of tokenAccounts.value) {
            const parsedInfo = accountInfo.account.data;
            
            if (parsedInfo.parsed && parsedInfo.parsed.info) {
                const info = parsedInfo.parsed.info;
                const mintAddress = info.mint;
                const tokenAmount = info.tokenAmount;
                
                // Skip accounts with zero balance
                if (parseFloat(tokenAmount.uiAmountString || '0') === 0) {
                    continue;
                }
                
                const symbol = TOKEN_SYMBOLS[mintAddress] || mintAddress.slice(0, 8) + '...';
                
                balances.push({
                    mint: mintAddress,
                    symbol: symbol,
                    balance: parseFloat(tokenAmount.uiAmountString || '0'),
                    decimals: tokenAmount.decimals,
                    rawBalance: tokenAmount.amount,
                    tokenAccount: accountInfo.pubkey.toString(),
                });
            }
        }
        
        return balances;
    } catch (error) {
        console.warn(`⚠️  Failed to get token accounts: ${error.message}`);
        return [];
    }
}

// Main function
async function snapshotBalance() {
    try {
        console.log('📸 Starting wallet balance snapshot...\n');
        
        // Load RPC URL
        const rpcUrl = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
        console.log(`🔌 Connecting to RPC: ${rpcUrl.replace(/\/v2\/[^/]+/, '/v2/***')}`);
        
        // Initialize connection
        const connection = new Connection(rpcUrl, {
            commitment: 'confirmed',
        });
        
        // Load wallet
        console.log('🔑 Loading wallet...');
        const keypair = loadWallet();
        const walletAddress = keypair.publicKey.toString();
        console.log(`   Wallet: ${walletAddress}\n`);
        
        // Get SOL balance
        console.log('💰 Fetching balances...');
        const solBalance = await connection.getBalance(keypair.publicKey);
        const solBalanceFormatted = solBalance / 1e9;
        
        const balances = [];
        
        // Add SOL balance if > 0
        if (solBalanceFormatted > 0) {
            balances.push({
                mint: 'So11111111111111111111111111111111111111112',
                symbol: 'SOL',
                balance: solBalanceFormatted,
                decimals: 9,
                rawBalance: solBalance.toString(),
            });
            console.log(`   SOL: ${solBalanceFormatted.toFixed(9)}`);
        }
        
        // Get all token balances
        console.log('   Fetching all token accounts...');
        const tokenBalances = await getAllTokenBalances(connection, keypair.publicKey);
        
        // Sort by balance (descending)
        tokenBalances.sort((a, b) => b.balance - a.balance);
        
        // Add token balances
        for (const tokenBalance of tokenBalances) {
            balances.push(tokenBalance);
            console.log(`   ${tokenBalance.symbol}: ${tokenBalance.balance.toFixed(tokenBalance.decimals)} (${tokenBalance.mint})`);
        }
        
        if (tokenBalances.length === 0) {
            console.log('   No token accounts found');
        }
        
        // Get SOL and USDC balances for portfolio calculation
        const usdcBalance = balances.find(b => b.symbol === 'USDC')?.balance || 0;
        
        // Get current SOL price and calculate portfolio values
        console.log('\n📈 Fetching SOL price for portfolio calculation...');
        const currentPrice = await getSOLPrice();
        
        let portfolio = null;
        if (currentPrice && currentPrice > 0) {
            // Calculate portfolio values (same as frontend)
            const totalValueSOL = solBalanceFormatted + (usdcBalance / currentPrice);
            const totalValueUSDC = (solBalanceFormatted * currentPrice) + usdcBalance;
            
            portfolio = {
                sol: solBalanceFormatted,
                usdc: usdcBalance,
                totalValueSOL: totalValueSOL,
                totalValueUSDC: totalValueUSDC,
                price: currentPrice,
            };
            
            console.log(`   SOL Price: $${currentPrice.toFixed(4)}`);
            console.log(`   Portfolio Value: ${totalValueSOL.toFixed(6)} SOL / $${totalValueUSDC.toFixed(2)} USDC`);
        } else {
            console.log('   ⚠️  Could not fetch price, portfolio values not calculated');
        }
        
        // Create snapshot object
        const snapshot = {
            timestamp: new Date().toISOString(),
            walletAddress: walletAddress,
            rpcUrl: rpcUrl.replace(/\/v2\/[^/]+/, '/v2/***'),
            balances: balances,
            totalSOL: solBalanceFormatted,
            portfolio: portfolio,
        };
        
        // Save snapshot to file (in parent directory's logging folder)
        const snapshotDir = path.join(__dirname, '..', 'logging');
        if (!fs.existsSync(snapshotDir)) {
            fs.mkdirSync(snapshotDir, { recursive: true });
        }
        
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
        const filename = `balance-snapshot-${timestamp}.json`;
        const filepath = path.join(snapshotDir, filename);
        
        fs.writeFileSync(filepath, JSON.stringify(snapshot, null, 2));
        
        console.log(`\n✅ Snapshot saved to: ${filepath}`);
        console.log(`\n📊 Summary:`);
        console.log(`   SOL Balance: ${solBalanceFormatted.toFixed(9)}`);
        console.log(`   USDC Balance: ${usdcBalance.toFixed(2)}`);
        if (portfolio) {
            console.log(`   Portfolio Value: ${portfolio.totalValueSOL.toFixed(6)} SOL / $${portfolio.totalValueUSDC.toFixed(2)} USDC`);
        }
        console.log(`   Token Accounts: ${tokenBalances.length}`);
        console.log(`   Total Assets: ${balances.length}`);
        
        return snapshot;
    } catch (error) {
        console.error('❌ Error creating snapshot:', error);
        process.exit(1);
    }
}

// Run the snapshot
snapshotBalance();

