mod web;

use anyhow::Result;
use bs58;
use delay_timer::prelude::*;
use dotenv;
use reqwest;
use serde::Deserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::*;
use spl_token::{
    amount_to_ui_amount, 
    native_mint
};
use std::{fs::File, time::Duration, sync::Arc, path::Path};
use std::io::{Write, BufWriter};
use tokio::sync::Mutex;
use chrono::{Utc, DateTime};
use web::{AppState, TokenBalance, TradeRecord, BollingerBandsInfo, BandPoint, PricePoint, ApiConnectionStatus};

#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

// Hàm để load keypair từ nhiều định dạng khác nhau
fn load_keypair() -> Result<Keypair> {
    dotenv::dotenv().ok();
    
    let wallet_key = dotenv::var("WALLET_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("WALLET_PRIVATE_KEY not found in .env file"))?;
    
    // Cách 1: Nếu là đường dẫn file (bắt đầu với / hoặc .)
    if wallet_key.starts_with('/') || wallet_key.starts_with('.') || wallet_key.contains('/') {
        println!("Loading keypair from file: {}", wallet_key);
        return read_keypair_file(&wallet_key)
            .map_err(|e| anyhow::anyhow!("Failed to read keypair file: {}", e));
    }
    
    // Cách 2: Nếu là mảng JSON [1,2,3,...]
    if wallet_key.trim_start().starts_with('[') {
        println!("Loading keypair from JSON array");
        let bytes: Vec<u8> = serde_json::from_str(&wallet_key)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON array: {}", e))?;
        if bytes.len() != 64 {
            return Err(anyhow::anyhow!("Invalid keypair length: expected 64 bytes, got {}", bytes.len()));
        }
        return Keypair::from_bytes(&bytes)
            .map_err(|e| anyhow::anyhow!("Failed to create keypair from bytes: {}", e));
    }
    
    // Cách 3: Nếu là base58 string
    println!("Loading keypair from base58 string");
    let bytes = bs58::decode(&wallet_key)
        .into_vec()
        .map_err(|e| anyhow::anyhow!("Failed to decode base58: {}", e))?;
    
    if bytes.len() != 64 {
        return Err(anyhow::anyhow!("Invalid keypair length: expected 64 bytes, got {}", bytes.len()));
    }
    
    Keypair::from_bytes(&bytes)
        .map_err(|e| anyhow::anyhow!("Failed to create keypair from bytes: {}", e))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    let keypair = load_keypair()?;
    let wallet_address = keypair.pubkey();
    println!("✅ Wallet connected: {}", wallet_address);
    println!("📝 Make sure this wallet has enough SOL and tokens for trading!");
    
    // Tạo shared state cho web server
    let app_state = AppState::new();
    
    // Cập nhật bot status
    {
        let mut status = app_state.bot_status.write().await;
        status.is_running = true;
        status.wallet_address = wallet_address.to_string();
        status.last_update = Utc::now();
        status.api_status = ApiConnectionStatus {
            rpc_endpoint: "checking".to_string(),
            raydium_service: "checking".to_string(),
            last_check: Utc::now(),
            rpc_url: None,
        };
    }
    
    // Kiểm tra kết nối API ngay lập tức
    {
        let app_state_for_check = app_state.clone();
        let wallet_address_for_check = wallet_address;
        tokio::spawn(async move {
            // Kiểm tra ngay lập tức
            check_api_connections(&app_state_for_check, wallet_address_for_check).await;
            // Sau đó kiểm tra định kỳ mỗi 30 giây
            check_api_connections_loop(app_state_for_check, wallet_address_for_check).await;
        });
    }
    
    // Khởi động web server TRƯỚC để có thể truy cập ngay cả khi có lỗi
    let app_state_for_web = app_state.clone();
    std::thread::spawn(move || {
        println!("🌐 Starting web server thread...");
        let sys = actix_web::rt::System::new();
        sys.block_on(async {
            println!("🌐 Starting web server on http://localhost:8080");
            if let Err(e) = web::start_web_server(app_state_for_web).await {
                eprintln!("❌ Web server error: {}", e);
            }
        });
    });
    
    // Đợi web server khởi động
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("✅ Web server should be running at http://localhost:8080");
    
    let keypair_buf = keypair.to_bytes();

    let path = "./config.json";
    let file = File::open(path)?;
    let dca: DcaJobs = serde_json::from_reader(file)?;
    let jobs = dca.jobs;
    let conditional_jobs = dca.conditional_jobs;
    let mut i = 0;

    let delay_timer = DelayTimerBuilder::default().build();

    // Xử lý DCA jobs
    if jobs.is_empty() {
        println!("ℹ️  No DCA jobs found in config.json, skipping DCA job setup");
    } else {
        println!("📋 Found {} DCA job(s), setting up...", jobs.len());
        for job in jobs {
            println!("in: {} out: {} amt: {}", job.input_mint, job.output_mint, job.amount);
            let rpc_client = RpcClient::new_with_commitment(
                "https://ssc-dao.genesysgo.net/".into(),
                CommitmentConfig::confirmed(),
            );

            // Xử lý lỗi RPC để bot vẫn tiếp tục chạy
            if let Err(e) = maybe_init_token_account(
                &rpc_client,
                &keypair,
                Pubkey::try_from(job.output_mint.as_str())?,
            ).await {
                eprintln!("⚠️  Warning: Failed to init token account: {}", e);
                eprintln!("   Bot will continue but trading may fail until RPC is available");
            }
            
            let body = move || {

                let kp_buf =  keypair_buf.clone();
                let kp = Keypair::from_bytes(&kp_buf).unwrap();
                let input_mint = Pubkey::try_from(job.input_mint.as_str()).expect("error parsing input mint");
                let output_mint = Pubkey::try_from(job.output_mint.as_str()).expect("error parsing output mint");
                let ui_amount = job.amount.clone();

                async move {
                    let swap_start = Utc::now();
                    match swap(
                        input_mint,
                        output_mint,
                        ui_amount,
                        1.0,
                        kp,
                    ).await {
                        Ok(swap_result) => {
                            // Log to CSV
                            if let Err(e) = log_transaction_to_csv(swap_start, &swap_result, &input_mint, &output_mint, ui_amount) {
                                eprintln!("⚠️  Failed to log transaction to CSV: {}", e);
                            }
                            println!("Swap successful: {}", swap_result.txid);
                        }
                        Err(e) => {
                            // Log failed swap
                            let failed_result = SwapResult {
                                txid: "failed".to_string(),
                                provider: "unknown".to_string(),
                                amount_in: ui_amount,
                                amount_out: 0.0,
                                expected_out: None,
                                status_code: 0,
                                error_message: Some(e.to_string()),
                                latency_ms: 0,
                            };
                            if let Err(log_err) = log_transaction_to_csv(swap_start, &failed_result, &input_mint, &output_mint, ui_amount) {
                                eprintln!("⚠️  Failed to log transaction to CSV: {}", log_err);
                            }
                            eprintln!("Swap error: {}", e);
                        }
                    }
                }
            };

            println!("Building DCA task {}", i + 1);
            println!("cron str {}", &job.cron);

            let task = TaskBuilder::default()
                .set_task_id(i.try_into().unwrap())
                .set_frequency_repeated_by_cron_str(&job.cron)
                .spawn_async_routine(body)?;

            delay_timer.add_task(task)?;
            i = i + 1;
        }
        println!("✅ DCA jobs setup complete");
    }

    // Xử lý Conditional jobs (Bollinger Bands)
    for conditional_job in conditional_jobs {
        if conditional_job.job_type == "bollinger_bands" {
            println!("Setting up Bollinger Bands job: {} -> {}",
                conditional_job.input_mint, conditional_job.output_mint);
            
            let rpc_client = RpcClient::new_with_commitment(
                "https://ssc-dao.genesysgo.net/".into(),
                CommitmentConfig::confirmed(),
            );

            // Khởi tạo token accounts (xử lý lỗi để bot vẫn tiếp tục)
            if let Err(e) = maybe_init_token_account(
                &rpc_client,
                &keypair,
                Pubkey::try_from(conditional_job.output_mint.as_str())?,
            ).await {
                eprintln!("⚠️  Warning: Failed to init output token account: {}", e);
            }
            
            if let Err(e) = maybe_init_token_account(
                &rpc_client,
                &keypair,
                Pubkey::try_from(conditional_job.input_mint.as_str())?,
            ).await {
                eprintln!("⚠️  Warning: Failed to init input token account: {}", e);
            }

            // Tạo price history cho job này
            let period = conditional_job.period;
            let price_history = Arc::new(Mutex::new(PriceHistory::new(period)));

            let kp_buf = keypair_buf.clone();
            let job_clone = conditional_job.clone();
            let price_history_clone = price_history.clone();
            let app_state_clone = app_state.clone();

            // Tạo task kiểm tra định kỳ (nến 1 phút = 60 giây)
            let body = move || {
                let kp_buf_inner = kp_buf.clone();
                let kp = Keypair::from_bytes(&kp_buf_inner).unwrap();
                let job = job_clone.clone();
                let price_history = price_history_clone.clone();
                let app_state = app_state_clone.clone();
                
                async move {
                    let _ = handle_bollinger_bands_job(
                        job, 
                        kp, 
                        price_history, 
                        app_state
                    ).await;
                }
            };

            // Nến 1 phút = kiểm tra mỗi 60 giây
            // Format: second minute hour day month weekday
            // Chạy mỗi 60 giây = mỗi phút ở giây 0
            let cron_str = "0 * * * * *".to_string();

            println!("Building conditional task {} with 1-minute candle interval (every 60 seconds)", i + 1);

            let task = TaskBuilder::default()
                .set_task_id(i.try_into().unwrap())
                .set_frequency_repeated_by_cron_str(&cron_str)
                .spawn_async_routine(body)?;

            delay_timer.add_task(task)?;
            i = i + 1;
        }
    }
    
    // Cập nhật số dư định kỳ (ngay lập tức và sau đó mỗi 30 giây)
    let app_state_balance = app_state.clone();
    let keypair_balance_buf = keypair_buf.clone();
    
    // Cập nhật ngay lập tức
    {
        let kp = Keypair::from_bytes(&keypair_buf).unwrap();
        if let Err(e) = update_balances(&app_state, &kp).await {
            eprintln!("⚠️  Error updating balances initially: {}", e);
        } else {
            println!("✅ Initial balances updated");
        }
    }
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let kp = Keypair::from_bytes(&keypair_balance_buf).unwrap();
            if let Err(e) = update_balances(&app_state_balance, &kp).await {
                eprintln!("⚠️  Error updating balances: {}", e);
            } else {
                println!("✅ Balances updated at {}", Utc::now().format("%H:%M:%S"));
            }
        }
    });

    // Chạy web server trong thread riêng với actix runtime
    let app_state_for_web = app_state.clone();
    std::thread::spawn(move || {
        println!("🌐 Starting web server thread...");
        let sys = actix_web::rt::System::new();
        sys.block_on(async {
            println!("🌐 Starting web server on http://localhost:8080");
            if let Err(e) = web::start_web_server(app_state_for_web).await {
                eprintln!("❌ Web server error: {}", e);
            }
        });
    });
    
    // Đợi một chút để web server khởi động
    println!("⏳ Waiting for web server to start...");
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Kiểm tra xem web server có chạy không
    match std::net::TcpStream::connect("127.0.0.1:8080") {
        Ok(_) => println!("✅ Web server is running! Access at http://localhost:8080"),
        Err(e) => {
            println!("⚠️  Cannot connect to web server: {}", e);
            println!("   The server may still be starting. Please wait a few seconds and try again.");
        }
    }
    
    println!("🤖 Bot is running. Press Ctrl+C to stop gracefully.");
    
    // Set up signal handlers for graceful shutdown
    setup_signal_handlers(app_state, delay_timer).await;
    
    Ok(())
}

// Set up signal handlers for SIGINT and SIGTERM
async fn setup_signal_handlers(app_state: AppState, _delay_timer: DelayTimer) {
    #[cfg(unix)]
    {
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to register SIGTERM handler");
        
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Received SIGINT (Ctrl+C). Shutting down gracefully...");
                shutdown_bot(&app_state).await;
            }
            _ = sigterm.recv() => {
                println!("\n🛑 Received SIGTERM. Shutting down gracefully...");
                shutdown_bot(&app_state).await;
            }
        }
    }
    
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.expect("Failed to wait for Ctrl+C");
        println!("\n🛑 Received SIGINT (Ctrl+C). Shutting down gracefully...");
        shutdown_bot(&app_state).await;
    }
}

// Graceful shutdown function
async fn shutdown_bot(app_state: &AppState) {
    println!("📝 Updating bot status...");
    
    // Update bot status to indicate shutdown
    {
        let mut status = app_state.bot_status.write().await;
        status.is_running = false;
        status.last_update = Utc::now();
    }
    
    println!("✅ Bot status updated. Shutting down...");
    println!("👋 Goodbye!");
}

#[derive(Deserialize)]
struct DcaJobs {
    #[serde(default)]
    jobs: Vec<Job>,
    #[serde(default)]
    conditional_jobs: Vec<ConditionalJob>,
}

#[derive(Deserialize, Clone)]
struct Job {
    input_mint: String,
    output_mint: String,
    amount: f64,
    cron: String,
}

// Swap result with detailed information for logging
struct SwapResult {
    txid: String,
    provider: String,
    amount_in: f64,
    amount_out: f64,
    expected_out: Option<f64>,
    status_code: u16,
    error_message: Option<String>,
    latency_ms: u64,
}

// Token symbol mapping
fn get_token_symbol(mint: &Pubkey) -> String {
    let mint_str = mint.to_string();
    match mint_str.as_str() {
        "So11111111111111111111111111111111111111112" => "SOL".to_string(),
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => "USDC".to_string(),
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" => "USDT".to_string(),
        _ => mint_str.chars().take(8).collect::<String>() + "...",
    }
}

// Get trading pair string
fn get_trading_pair(input_mint: &Pubkey, output_mint: &Pubkey) -> String {
    format!("{}/{}", get_token_symbol(output_mint), get_token_symbol(input_mint))
}

// Determine side (buy or sell) based on whether we're buying SOL or selling SOL
fn get_side(input_mint: &Pubkey, output_mint: &Pubkey) -> String {
    let sol_mint = Pubkey::try_from("So11111111111111111111111111111111111111112").unwrap();
    if output_mint == &sol_mint {
        "buy".to_string()
    } else if input_mint == &sol_mint {
        "sell".to_string()
    } else {
        "unknown".to_string()
    }
}

// Log transaction to CSV
fn log_transaction_to_csv(
    time: DateTime<Utc>,
    swap_result: &SwapResult,
    input_mint: &Pubkey,
    output_mint: &Pubkey,
    amount_in: f64,
) -> Result<()> {
    // Create logging directory if it doesn't exist
    let log_dir = Path::new("logging");
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir)?;
    }
    
    let csv_path = log_dir.join("transactions.csv");
    let file_exists = csv_path.exists();
    
    // Open file in append mode
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&csv_path)?;
    
    let mut writer = BufWriter::new(file);
    
    // Write header if file is new
    if !file_exists {
        writeln!(writer, "time,txid,pair,side,amount-in,amount-out,price,expected-out,provider,latency-ms,status")?;
    }
    
    // Calculate price (quote-amount / base-amount)
    // For SOL/USDC pair: base = SOL, quote = USDC, price = USDC/SOL
    let sol_mint = Pubkey::try_from("So11111111111111111111111111111111111111112").unwrap();
    let price = if swap_result.amount_out > 0.0 {
        // If output is SOL (base), price = quote/base = amount_in/amount_out
        // If input is SOL (base), price = quote/base = amount_out/amount_in
        if output_mint == &sol_mint {
            // Buying SOL: input is quote (USDC), output is base (SOL)
            amount_in / swap_result.amount_out
        } else if input_mint == &sol_mint {
            // Selling SOL: input is base (SOL), output is quote (USDC)
            swap_result.amount_out / amount_in
        } else {
            // Fallback: assume first token is base
            amount_in / swap_result.amount_out
        }
    } else {
        0.0
    };
    
    // Format expected_out
    let expected_out_str = swap_result.expected_out
        .map(|v| v.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    
    // Format status with error message if any
    let status = if let Some(ref err) = swap_result.error_message {
        format!("{} - {}", swap_result.status_code, err)
    } else {
        swap_result.status_code.to_string()
    };
    
    // Escape CSV fields (handle commas and quotes)
    let escape_csv = |s: &str| {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace("\"", "\"\""))
        } else {
            s.to_string()
        }
    };
    
    // Write CSV row
    writeln!(
        writer,
        "{},{},{},{},{},{},{},{},{},{},{}",
        escape_csv(&time.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
        escape_csv(&swap_result.txid),
        escape_csv(&get_trading_pair(input_mint, output_mint)),
        escape_csv(&get_side(input_mint, output_mint)),
        amount_in,
        swap_result.amount_out,
        price,
        expected_out_str,
        escape_csv(&swap_result.provider),
        swap_result.latency_ms,
        escape_csv(&status),
    )?;
    
    writer.flush()?;
    Ok(())
}

#[derive(Deserialize, Clone)]
#[derive(serde::Serialize)]
struct ConditionalJob {
    #[serde(rename = "type")]
    job_type: String, // "bollinger_bands"
    input_mint: String,
    output_mint: String,
    base_mint: String, // Token để tính giá (ví dụ: USDC)
    quote_mint: String, // Token để so sánh (ví dụ: SOL)
    buy_amount_usd: f64, // Số tiền mua (USD)
    sell_amount: f64, // Số lượng bán (token)
    period: usize, // Số kỳ cho Bollinger Bands (mặc định 20)
    std_dev: f64, // Độ lệch chuẩn (mặc định 2.0)
    check_interval_seconds: u64, // Khoảng thời gian kiểm tra (giây)
}

// Lưu trữ lịch sử giá cho Bollinger Bands (kèm timestamp)
struct PriceHistory {
    prices: Vec<PricePoint>,
    period: usize,     // số kỳ để tính Bollinger
    max_store: usize,  // số điểm lưu tối đa để hiển thị (vd 60 phút)
}

impl PriceHistory {
    fn new(period: usize) -> Self {
        let max_store = 60_usize.max(period);
        Self {
            prices: Vec::with_capacity(max_store),
            period,
            max_store,
        }
    }

    fn add_price(&mut self, price: f64, timestamp: DateTime<Utc>) {
        self.prices.push(PricePoint { price, timestamp });
        if self.prices.len() > self.max_store {
            self.prices.remove(0);
        }
    }

    fn calculate_bollinger_bands(&self, std_dev: f64) -> Option<(f64, f64, f64)> {
        if self.prices.len() < self.period {
            return None;
        }

        // Lấy đoạn cuối theo số kỳ để tính Bollinger
        let start_idx = self.prices.len().saturating_sub(self.period);
        let slice = &self.prices[start_idx..];

        // Tính SMA (Simple Moving Average)
        let sma = slice.iter().map(|p| p.price).sum::<f64>() / slice.len() as f64;

        // Tính độ lệch chuẩn
        let variance = slice
            .iter()
            .map(|p| (p.price - sma).powi(2))
            .sum::<f64>() / slice.len() as f64;
        let std = variance.sqrt();

        // Tính các dải
        let upper_band = sma + (std_dev * std);
        let lower_band = sma - (std_dev * std);

        Some((upper_band, sma, lower_band))
    }
}

async fn maybe_init_token_account(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    mint: Pubkey,
) -> Result<()> {
    let token_address = get_associated_token_address(
        &keypair.pubkey(), 
        &mint
    );

    let token_acc = rpc_client.get_token_account(
        &token_address
    )
    .await;

    match token_acc {
        // check error type
        Err(_) => {
            let create_ata_ix = instruction::create_associated_token_account(
                &keypair.pubkey(),
                &keypair.pubkey(),
                &mint,
                &spl_token::id()
            );
            
            let blockhash = rpc_client.get_latest_blockhash().await?;
            let transaction = Transaction::new_signed_with_payer(
                &[create_ata_ix],
                Some(&keypair.pubkey()),
                &[keypair],
                blockhash,
            );
            println!(
                "Sending transaction: {}",
                transaction.signatures[0]
            );
            let signature = rpc_client
                .send_and_confirm_transaction_with_spinner(&transaction)
                .await?;
            println!(
                "TX signature: {}",
                signature
            );
            let _ = rpc_client.get_token_account(
                &token_address
            )
            .await?;
        }
        _ => {}
    };
        
    Ok(())
}

// Lấy giá hiện tại của cặp token từ Binance
// Trả về giá quote/base, ví dụ: SOL/USDC = 150 nghĩa là 1 SOL = 150 USDC
async fn get_current_price(
    _base_mint: Pubkey,
    _quote_mint: Pubkey,
) -> Result<f64> {
    // Sử dụng Binance để lấy giá SOL/USDT (gần tương đương SOL/USDC)
    get_price_from_binance().await
}

// Lấy giá SOL/USDT từ Binance (dùng làm fallback, gần tương đương SOL/USDC)
async fn get_price_from_binance() -> Result<f64> {
    let url = "https://api.binance.com/api/v3/ticker/price?symbol=SOLUSDT";
    println!("🌐 Đang lấy giá từ Binance: {}", url);
    let resp = reqwest::get(url).await?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("Binance HTTP {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await?;
    let price_str = body.get("price")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Binance response missing price"))?;
    let price: f64 = price_str.parse()
        .map_err(|e| anyhow::anyhow!("Cannot parse Binance price: {}", e))?;
    Ok(price)
}

// Hàm cập nhật số dư token
async fn update_balances(
    app_state: &AppState,
    keypair: &Keypair,
) -> Result<()> {
    // Thử nhiều RPC endpoints
    let rpc_urls = vec![
        "https://api.mainnet-beta.solana.com",
        "https://solana-api.projectserum.com",
        "https://ssc-dao.genesysgo.net",
    ];
    
    let mut rpc_client_opt = None;
    for url in rpc_urls {
        let client = RpcClient::new_with_commitment(
            url.into(),
            CommitmentConfig::confirmed(),
        );
        // Test connection
        if client.get_balance(&keypair.pubkey()).await.is_ok() {
            rpc_client_opt = Some(client);
            break;
        }
    }
    
    let rpc_client = match rpc_client_opt {
        Some(client) => client,
        None => {
            eprintln!("⚠️  All RPC endpoints failed, using default");
            RpcClient::new_with_commitment(
                "https://api.mainnet-beta.solana.com".into(),
                CommitmentConfig::confirmed(),
            )
        }
    };
    
    let mut balances = Vec::new();
    
    // SOL balance
    match rpc_client.get_balance(&keypair.pubkey()).await {
        Ok(balance) => {
            let sol_balance = amount_to_ui_amount(balance, native_mint::DECIMALS);
            balances.push(TokenBalance {
                mint: "So11111111111111111111111111111111111111112".to_string(),
                symbol: "SOL".to_string(),
                balance: sol_balance,
                decimals: 9,
            });
        }
        Err(e) => {
            eprintln!("⚠️  Failed to get SOL balance: {}", e);
        }
    }
    
    // USDC balance
    let usdc_mint = Pubkey::try_from("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    let usdc_address = get_associated_token_address(&keypair.pubkey(), &usdc_mint);
    match rpc_client.get_token_account(&usdc_address).await {
        Ok(Some(acc)) => {
            balances.push(TokenBalance {
                mint: usdc_mint.to_string(),
                symbol: "USDC".to_string(),
                balance: acc.token_amount.ui_amount.unwrap_or(0.0),
                decimals: acc.token_amount.decimals,
            });
        }
        Ok(None) => {
            // Token account doesn't exist, balance is 0
            balances.push(TokenBalance {
                mint: usdc_mint.to_string(),
                symbol: "USDC".to_string(),
                balance: 0.0,
                decimals: 6,
            });
        }
        Err(e) => {
            eprintln!("⚠️  Failed to get USDC balance: {}", e);
        }
    }
    
    *app_state.balances.write().await = balances;
    
    // Cập nhật last_update
    {
        let mut status = app_state.bot_status.write().await;
        status.last_update = Utc::now();
    }
    
    Ok(())
}

// Hàm xử lý conditional job với Bollinger Bands
async fn handle_bollinger_bands_job(
    job: ConditionalJob,
    keypair: Keypair,
    price_history: Arc<Mutex<PriceHistory>>,
    app_state: AppState,
) -> Result<()> {
    let base_mint = Pubkey::try_from(job.base_mint.as_str())?;
    let quote_mint = Pubkey::try_from(job.quote_mint.as_str())?;
    let input_mint = Pubkey::try_from(job.input_mint.as_str())?;
    let output_mint = Pubkey::try_from(job.output_mint.as_str())?;
    
    // Lấy giá hiện tại (ví dụ: SOL/USDC) với retry logic
    let current_price = match get_current_price(base_mint, quote_mint).await {
        Ok(price) => price,
        Err(e) => {
            eprintln!("❌ Không thể lấy giá sau nhiều lần thử: {}", e);
            eprintln!("   Kiểm tra kết nối internet và DNS");
            // Không return ngay, vẫn cập nhật last_update để biết bot đang chạy
            {
                let mut status = app_state.bot_status.write().await;
                status.last_update = Utc::now();
            }
            return Ok(());
        }
    };
    
    println!("Current price {}: {} {}", 
        format!("{}/{}", job.base_mint, job.quote_mint),
        current_price,
        job.quote_mint
    );

    // Cập nhật giá hiện tại vào state ngay lập tức
    {
        *app_state.current_price.write().await = Some(current_price);
    }

    // Thêm giá vào lịch sử và snapshot để tính toán
    let (bands_opt, prices_snapshot, count) = {
        let mut history = price_history.lock().await;
        let now = Utc::now();
        history.add_price(current_price, now);
        let count = history.prices.len();
        println!("📊 Price history: {}/{} prices collected", count, job.period);
        // Snapshot giá kèm timestamp để lưu (cap 60 điểm)
        // Lấy 60 điểm gần nhất, giữ nguyên thứ tự thời gian (cũ -> mới)
        let prices_snapshot: Vec<PricePoint> = if history.prices.len() > 60 {
            history.prices.iter().skip(history.prices.len() - 60).cloned().collect()
        } else {
            history.prices.clone()
        };
        let bands_opt = history.calculate_bollinger_bands(job.std_dev);
        (bands_opt, prices_snapshot, count)
    };

    // Nếu chưa đủ dữ liệu, lưu history (band rỗng) và thoát
    let (upper_band, middle_band, lower_band) = match bands_opt {
        Some(bands) => bands,
        None => {
            println!("⏳ Chưa đủ dữ liệu để tính Bollinger Bands (cần {} giá, hiện có {})", job.period, count);

            // Lưu history để vẽ (bands rỗng)
            // Lưu tối đa 60 điểm để biểu đồ 60 phút
            *app_state.price_history.write().await = prices_snapshot.clone();

            let mut band_history = app_state.band_history.write().await;
            band_history.push(BandPoint { upper: None, middle: None, lower: None });
            if band_history.len() > 60 {
                band_history.remove(0);
            }

            // Cập nhật last_update
            {
                let mut status = app_state.bot_status.write().await;
                status.last_update = Utc::now();
            }
            return Ok(());
        }
    };

    println!("Bollinger Bands - Upper: {:.4}, Middle: {:.4}, Lower: {:.4}, Current: {:.4}",
        upper_band, middle_band, lower_band, current_price);

    // Cập nhật Bollinger Bands info vào state
    {
        let mut bands_info = app_state.bollinger_bands.write().await;
        *bands_info = Some(BollingerBandsInfo {
            upper_band,
            middle_band,
            lower_band,
            current_price,
        });
    }
    
    // Cập nhật price_history và band_history để vẽ biểu đồ
    {
        *app_state.price_history.write().await = prices_snapshot.clone();

        let mut band_history = app_state.band_history.write().await;
        band_history.push(BandPoint { 
            upper: Some(upper_band), 
            middle: Some(middle_band), 
            lower: Some(lower_band) 
        });
        if band_history.len() > 60 {
            band_history.remove(0);
        }
    }

    // Cập nhật last_update
    {
        let mut status = app_state.bot_status.write().await;
        status.last_update = Utc::now();
    }

    // Kiểm tra cooldown để tránh swap liên tục (tối thiểu 60 giây giữa các lần swap)
    const MIN_SWAP_INTERVAL_SECS: u64 = 60;
    {
        let last_swap = app_state.last_swap_time.read().await;
        if let Some(last_time) = *last_swap {
            let elapsed = last_time.elapsed().as_secs();
            if elapsed < MIN_SWAP_INTERVAL_SECS {
                println!("⏳ Cooldown: Chờ {} giây nữa trước khi swap tiếp (đã qua {} giây)", 
                         MIN_SWAP_INTERVAL_SECS - elapsed, elapsed);
                return Ok(());
            }
        }
    }
    
    // Kiểm tra điều kiện và thực hiện swap
    // Lưu ý: 
    // - input_mint = USDC (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v)
    // - output_mint = SOL (So11111111111111111111111111111111111111112)
    // - current_price = giá SOL/USDC (ví dụ: 150 nghĩa là 1 SOL = 150 USDC)
    
    if current_price <= lower_band {
        // Mua 0.1 SOL khi giá đóng nến dưới band dưới
        println!("🚀 TÍN HIỆU MUA: Giá ({:.4}) <= BB Lower ({:.4})", current_price, lower_band);
        println!("   → Mua 0.1 SOL từ USDC");
        
        // Tính số lượng USDC cần để mua 0.1 SOL
        // current_price là giá SOL/USDC, nên để mua 0.1 SOL cần: 0.1 * current_price USDC
        let sol_amount = 0.1;
        let usdc_needed = sol_amount * current_price;
        
        println!("   → Cần {} USDC để mua {} SOL", usdc_needed, sol_amount);
        
        let result = swap_with_record(
            input_mint,  // USDC
            output_mint, // SOL
            usdc_needed, // Số lượng USDC
            1.0,         // Slippage 1%
            keypair,
            "BUY".to_string(),
            current_price,
            upper_band,
            lower_band,
            app_state.clone(),
        ).await;
        
        match result {
            Ok(sig) => {
                println!("   ✅ Mua thành công! Transaction: {}", sig);
                // Cập nhật thời gian swap cuối cùng
                *app_state.last_swap_time.write().await = Some(std::time::Instant::now());
            }
            Err(e) => {
                eprintln!("   ❌ Lỗi khi mua: {}", e);
                eprintln!("   → Bot sẽ thử lại ở lần kiểm tra tiếp theo");
            }
        }
        
    } else if current_price >= upper_band {
        // Bán 0.1 SOL khi giá đóng nến trên band trên
        println!("📉 TÍN HIỆU BÁN: Giá ({:.4}) >= BB Upper ({:.4})", current_price, upper_band);
        println!("   → Bán 0.1 SOL để lấy USDC");
        
        let sol_amount = 0.1;
        println!("   → Bán {} SOL", sol_amount);
        
        let result = swap_with_record(
            output_mint, // SOL (bán)
            input_mint,  // USDC (nhận)
            sol_amount,  // Số lượng SOL
            1.0,         // Slippage 1%
            keypair,
            "SELL".to_string(),
            current_price,
            upper_band,
            lower_band,
            app_state.clone(),
        ).await;
        
        match result {
            Ok(sig) => {
                println!("   ✅ Bán thành công! Transaction: {}", sig);
                // Cập nhật thời gian swap cuối cùng
                *app_state.last_swap_time.write().await = Some(std::time::Instant::now());
            }
            Err(e) => {
                eprintln!("   ❌ Lỗi khi bán: {}", e);
                eprintln!("   → Bot sẽ thử lại ở lần kiểm tra tiếp theo");
            }
        }
    } else {
        println!("⏸️  Giá ({:.4}) trong phạm vi Bollinger Bands [Lower: {:.4}, Upper: {:.4}]", 
                 current_price, lower_band, upper_band);
        println!("   → Không thực hiện giao dịch");
    }

    Ok(())
}

// Hàm swap với ghi lại giao dịch
async fn swap_with_record(
    input_mint: Pubkey,
    output_mint: Pubkey,
    ui_amount: f64,
    slippage: f64,
    keypair: Keypair,
    order_type: String,
    entry_price: f64,
    upper_band: f64,
    lower_band: f64,
    app_state: AppState,
) -> Result<String> {
    let start_time = Utc::now();
    let mut transaction_signature = String::new();
    let mut fee = 0.0;
    
    // Thực hiện swap
    let result = swap(
        input_mint,
        output_mint,
        ui_amount,
        slippage,
        keypair,
    ).await;
    
    // Log to CSV
    match &result {
        Ok(swap_result) => {
            if let Err(e) = log_transaction_to_csv(start_time, swap_result, &input_mint, &output_mint, ui_amount) {
                eprintln!("⚠️  Failed to log transaction to CSV: {}", e);
            }
            transaction_signature = swap_result.txid.clone();
        }
        Err(e) => {
            // Log failed swap
            let failed_result = SwapResult {
                txid: "failed".to_string(),
                provider: "unknown".to_string(),
                amount_in: ui_amount,
                amount_out: 0.0,
                expected_out: None,
                status_code: 0,
                error_message: Some(e.to_string()),
                latency_ms: 0,
            };
            if let Err(log_err) = log_transaction_to_csv(start_time, &failed_result, &input_mint, &output_mint, ui_amount) {
                eprintln!("⚠️  Failed to log transaction to CSV: {}", log_err);
            }
            transaction_signature = "failed".to_string();
        }
    }
    
    // Tính phí (ước tính, thực tế cần lấy từ transaction)
    fee = 0.000005; // ~0.000005 SOL per transaction
    
    let status = if result.is_ok() {
        "SUCCESS".to_string()
    } else {
        "FAILED".to_string()
    };
    
    // Tính fee_usdc, đảm bảo không bị NaN
    let fee_usdc_value = if entry_price > 0.0 && fee > 0.0 {
        fee * entry_price
    } else {
        0.0
    };
    
    // Lưu giao dịch vào state
    let trade_record = TradeRecord {
        timestamp: start_time,
        order_type: order_type.clone(),
        amount: if order_type == "BUY" { 0.1 } else { 0.1 }, // 0.1 SOL
        price: entry_price,
        upper_band,
        lower_band,
        entry_price,
        fee,
        fee_usdc: fee_usdc_value,
        transaction_signature: transaction_signature.clone(),
        status,
    };
    
    {
        let mut trades = app_state.trades.write().await;
        trades.push(trade_record);
        // Giữ tối đa 1000 giao dịch gần nhất
        if trades.len() > 1000 {
            trades.remove(0);
        }
    }
    
    // Cập nhật số dư sau giao dịch (tạo lại keypair từ bytes nếu cần)
    // Note: keypair đã được move vào swap, nên không thể dùng lại ở đây
    // Sẽ được cập nhật bởi background task
    
    match result {
        Ok(_) => Ok(transaction_signature),
        Err(e) => Err(e),
    }
}

// Hàm kiểm tra kết nối API định kỳ
async fn check_api_connections_loop(app_state: AppState, wallet_address: Pubkey) {
    loop {
        check_api_connections(&app_state, wallet_address).await;
        // Kiểm tra mỗi 30 giây
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

// Hàm kiểm tra kết nối API một lần
async fn check_api_connections(app_state: &AppState, wallet_address: Pubkey) {
    println!("🔍 Đang kiểm tra kết nối API... ({} UTC)", Utc::now().format("%H:%M:%S"));
    
    // Kiểm tra RPC endpoint
    let (rpc_status, rpc_url) = {
        println!("  → Kiểm tra RPC endpoints...");
        let rpc_urls = vec![
            "https://api.mainnet-beta.solana.com",
            "https://solana-api.projectserum.com",
            "https://rpc.ankr.com/solana",
            "https://solana.public-rpc.com",
            "https://ssc-dao.genesysgo.net",
        ];
        
        let mut connected_url = None;
        let mut connected = false;
        
        for url in rpc_urls {
            let client = RpcClient::new_with_commitment(
                url.into(),
                CommitmentConfig::confirmed(),
            );
            match tokio::time::timeout(
                Duration::from_secs(5),
                client.get_balance(&wallet_address)
            ).await {
                Ok(Ok(_)) => {
                    connected = true;
                    connected_url = Some(url.to_string());
                    println!("  ✅ RPC Endpoint: {} - Kết nối thành công", url);
                    break;
                },
                Ok(Err(e)) => {
                    println!("  ⚠️  RPC Endpoint: {} - Lỗi: {}", url, e);
                },
                Err(_) => {
                    println!("  ⚠️  RPC Endpoint: {} - Timeout", url);
                },
            }
        }
        
        if !connected {
            eprintln!("  ❌ Tất cả RPC endpoints đều thất bại");
        }
        
        (
            if connected { "connected".to_string() } else { "disconnected".to_string() },
            connected_url
        )
    };
    
    // Kiểm tra Raydium service
    let raydium_status = {
        println!("  → Kiểm tra Raydium service...");
        const RAYDIUM_SERVICE_URL: &str = "http://localhost:3001";
        
        match tokio::time::timeout(
            Duration::from_secs(5),
            reqwest::Client::new()
                .get(&format!("{}/health", RAYDIUM_SERVICE_URL))
                .send()
        ).await {
            Ok(Ok(resp)) => {
                if resp.status().is_success() {
                    // Đọc text trước, rồi parse JSON
                    match resp.text().await {
                        Ok(text) => {
                            match serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json) => {
                                    let initialized = json.get("raydium_initialized")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false);
                                    if initialized {
                                        println!("  ✅ Raydium service: Kết nối thành công và SDK đã khởi tạo");
                                        "connected".to_string()
                                    } else {
                                        println!("  ⚠️  Raydium service: Kết nối được nhưng SDK chưa khởi tạo. Response: {}", text);
                                        "disconnected".to_string()
                                    }
                                },
                                Err(e) => {
                                    eprintln!("  ⚠️  Raydium service: Không parse được JSON: {}. Response: {}", e, text);
                                    "disconnected".to_string()
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("  ⚠️  Raydium service: Không đọc được response text: {}", e);
                            "disconnected".to_string()
                        }
                    }
                } else {
                    eprintln!("  ❌ Raydium service: HTTP status {}", resp.status());
                    "disconnected".to_string()
                }
            },
            Ok(Err(e)) => {
                eprintln!("  ❌ Raydium service: Lỗi kết nối - {}", e);
                "disconnected".to_string()
            },
            Err(_) => {
                eprintln!("  ❌ Raydium service: Timeout sau 5 giây");
                "disconnected".to_string()
            },
        }
    };
    
    // Cập nhật trạng thái
    {
        let mut status = app_state.bot_status.write().await;
        status.api_status = ApiConnectionStatus {
            rpc_endpoint: rpc_status.clone(),
            raydium_service: raydium_status.clone(),
            last_check: Utc::now(),
            rpc_url: rpc_url.clone(),
        };
    }
    
    println!("🔍 Kết quả kiểm tra API:");
    println!("   - RPC Endpoint: {} ({:?})", rpc_status, rpc_url);
    println!("   - Raydium Service: {}", raydium_status);
    
    // Debug: Kiểm tra xem state đã được cập nhật chưa
    {
        let status = app_state.bot_status.read().await;
        println!("   📊 State sau khi cập nhật:");
        println!("      - raydium_service trong state: {}", status.api_status.raydium_service);
    }
    
    // Kiểm tra xem có sẵn sàng swap không
    if raydium_status == "connected" && rpc_status == "connected" {
        println!("✅ Sẵn sàng swap!");
    } else {
        eprintln!("⚠️  Chưa sẵn sàng swap - Kiểm tra kết nối mạng và DNS");
    }
}

// Hàm swap chính - Sử dụng Jupiter làm primary, Raydium làm fallback
async fn swap(
    input_mint: Pubkey,
    output_mint: Pubkey,
    ui_amount: f64,
    slippage: f64,
    _keypair: Keypair,
) -> Result<SwapResult> {
    // Try Jupiter first (more reliable)
    match swap_via_jupiter_service(input_mint, output_mint, ui_amount, slippage).await {
        Ok(result) => Ok(result),
        Err(jupiter_err) => {
            eprintln!("⚠️  Jupiter swap failed: {}, falling back to Raydium...", jupiter_err);
            swap_via_raydium(input_mint, output_mint, ui_amount, slippage).await
        }
    }
}

// Swap qua Raydium service
async fn swap_via_raydium(
    input_mint: Pubkey,
    output_mint: Pubkey,
    ui_amount: f64,
    slippage: f64,
) -> Result<SwapResult> {
    const RAYDIUM_SERVICE_URL: &str = "http://localhost:3001";
    
    println!("🔄 Attempting swap via Raydium service...");
    println!("   Input: {} {} -> Output: {}", ui_amount, input_mint, output_mint);
    
    let client = reqwest::Client::new();
    
    // Kiểm tra health trước
    match client.get(&format!("{}/health", RAYDIUM_SERVICE_URL))
        .timeout(Duration::from_secs(5))
        .send()
        .await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✅ Raydium service is available");
            } else {
                return Err(anyhow::anyhow!("Raydium service health check failed"));
            }
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Cannot connect to Raydium service: {}. Make sure service is running on port 3001", e));
        }
    }
    
    // Thực hiện swap
    let request_body = serde_json::json!({
        "inputMint": input_mint.to_string(),
        "outputMint": output_mint.to_string(),
        "amountIn": ui_amount.to_string(),
        "slippage": slippage
    });
    
    println!("📤 Sending swap request to Raydium service:");
    println!("   {}", serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| "Failed to serialize".to_string()));
    
    let start_time = std::time::Instant::now();
    let response = client
        .post(&format!("{}/swap", RAYDIUM_SERVICE_URL))
        .json(&request_body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to send request to Raydium service: {}", e);
            anyhow::anyhow!("Raydium service request failed: {}", e)
        })?;
    
    let latency_ms = start_time.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        eprintln!("❌ Raydium service HTTP error ({}): {}", status_code, error_text);
        return Ok(SwapResult {
            txid: "failed".to_string(),
            provider: "fallback".to_string(),
            amount_in: ui_amount,
            amount_out: 0.0,
            expected_out: None,
            status_code,
            error_message: Some(error_text),
            latency_ms,
        });
    }
    
    let result: serde_json::Value = response.json().await
        .map_err(|e| {
            eprintln!("❌ Failed to parse Raydium response JSON: {}", e);
            anyhow::anyhow!("Failed to parse Raydium response: {}", e)
        })?;
    
    // Debug: log full response
    println!("📋 Raydium service response: {}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Failed to serialize".to_string()));
    
    if let Some(success) = result.get("success").and_then(|s| s.as_bool()) {
        if success {
            let tx_sig = result.get("transactionSignature")
                .and_then(|s| s.as_str())
                .ok_or_else(|| {
                    eprintln!("❌ Response missing transactionSignature field");
                    anyhow::anyhow!("No transaction signature in response")
                })?;
            
            let amount_out = result.get("outputAmount")
                .and_then(|a| a.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            
            println!("✅ Raydium swap successful: {}", tx_sig);
            return Ok(SwapResult {
                txid: tx_sig.to_string(),
                provider: "fallback".to_string(), // Raydium is fallback
                amount_in: ui_amount,
                amount_out,
                expected_out: None, // Raydium doesn't provide expected output in response
                status_code: 200,
                error_message: None,
                latency_ms,
            });
        }
    }
    
    // Extract error message
    let error_msg_str = result.get("error")
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let json_str = serde_json::to_string(&result).unwrap_or_else(|_| "Unknown error".to_string());
            json_str
        });
    eprintln!("❌ Raydium swap failed: {}", error_msg_str);
    
    // Fallback to Jupiter API if Raydium fails
    if error_msg_str.contains("fetch vault info error") || error_msg_str.contains("vault") {
        eprintln!("⚠️  Raydium vault error detected, falling back to Jupiter API...");
        return swap_via_jupiter_service(input_mint, output_mint, ui_amount, slippage).await;
    }
    
    Ok(SwapResult {
        txid: "failed".to_string(),
        provider: "fallback".to_string(),
        amount_in: ui_amount,
        amount_out: 0.0,
        expected_out: None,
        status_code,
        error_message: Some(error_msg_str),
        latency_ms,
    })
}

// Primary: Swap via Jupiter service
async fn swap_via_jupiter_service(
    input_mint: Pubkey,
    output_mint: Pubkey,
    ui_amount: f64,
    slippage: f64,
) -> Result<SwapResult> {
    const JUPITER_SERVICE_URL: &str = "http://localhost:3002";
    
    println!("🔄 Attempting swap via Jupiter service...");
    println!("   Input: {} {} -> Output: {}", ui_amount, input_mint, output_mint);
    
    let client = reqwest::Client::new();
    
    // Kiểm tra health trước
    match client.get(&format!("{}/health", JUPITER_SERVICE_URL))
        .timeout(Duration::from_secs(5))
        .send()
        .await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✅ Jupiter service is available");
            } else {
                return Err(anyhow::anyhow!("Jupiter service health check failed"));
            }
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Cannot connect to Jupiter service: {}. Make sure service is running on port 3002", e));
        }
    }
    
    // Get quote first to get expected output (optional, don't fail if it errors)
    let expected_out = match client
        .post(&format!("{}/quote", JUPITER_SERVICE_URL))
        .json(&serde_json::json!({
            "inputMint": input_mint.to_string(),
            "outputMint": output_mint.to_string(),
            "amountIn": ui_amount.to_string(),
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
    {
        Ok(resp) => {
            match resp.json::<serde_json::Value>().await {
                Ok(v) => v.get("outputAmount")
                    .and_then(|a| a.as_str())
                    .and_then(|s| s.parse::<f64>().ok()),
                Err(_) => None,
            }
        }
        Err(_) => None,
    };
    
    // Thực hiện swap
    let request_body = serde_json::json!({
        "inputMint": input_mint.to_string(),
        "outputMint": output_mint.to_string(),
        "amountIn": ui_amount.to_string(),
        "slippage": slippage
    });
    
    println!("📤 Sending swap request to Jupiter service:");
    println!("   {}", serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| "Failed to serialize".to_string()));
    
    let start_time = std::time::Instant::now();
    let response = client
        .post(&format!("{}/swap", JUPITER_SERVICE_URL))
        .json(&request_body)
        .timeout(Duration::from_secs(120)) // 2 minutes for Jupiter
        .send()
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to send request to Jupiter service: {}", e);
            anyhow::anyhow!("Jupiter service request failed: {}", e)
        })?;
    
    let latency_ms = start_time.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        eprintln!("❌ Jupiter service HTTP error ({}): {}", status_code, error_text);
        return Ok(SwapResult {
            txid: "failed".to_string(),
            provider: "jupiter".to_string(),
            amount_in: ui_amount,
            amount_out: 0.0,
            expected_out,
            status_code,
            error_message: Some(error_text),
            latency_ms,
        });
    }
    
    let result: serde_json::Value = response.json().await
        .map_err(|e| {
            eprintln!("❌ Failed to parse Jupiter response JSON: {}", e);
            anyhow::anyhow!("Failed to parse Jupiter response: {}", e)
        })?;
    
    // Debug: log full response
    println!("📋 Jupiter service response: {}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Failed to serialize".to_string()));
    
    if let Some(success) = result.get("success").and_then(|s| s.as_bool()) {
        if success {
            let tx_sig = result.get("transactionSignature")
                .and_then(|s| s.as_str())
                .ok_or_else(|| {
                    eprintln!("❌ Response missing transactionSignature field");
                    anyhow::anyhow!("No transaction signature in response")
                })?;
            
            let provider = result.get("provider")
                .and_then(|p| p.as_str())
                .unwrap_or("jupiter")
                .to_string();
            
            let amount_out = result.get("outputAmount")
                .and_then(|a| a.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            
            println!("✅ {} swap successful: {}", provider, tx_sig);
            return Ok(SwapResult {
                txid: tx_sig.to_string(),
                provider: if provider.contains("fallback") { "fallback".to_string() } else { "jupiter".to_string() },
                amount_in: ui_amount,
                amount_out,
                expected_out,
                status_code: 200,
                error_message: None,
                latency_ms,
            });
        }
    }
    
    // Extract error message
    let error_msg_str = result.get("error")
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let json_str = serde_json::to_string(&result).unwrap_or_else(|_| "Unknown error".to_string());
            json_str
        });
    eprintln!("❌ Jupiter swap failed: {}", error_msg_str);
    
    Ok(SwapResult {
        txid: "failed".to_string(),
        provider: "jupiter".to_string(),
        amount_in: ui_amount,
        amount_out: 0.0,
        expected_out,
        status_code,
        error_message: Some(error_msg_str),
        latency_ms,
    })
}

