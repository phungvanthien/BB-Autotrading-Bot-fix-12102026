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
            jupiter_service: "checking".to_string(),
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
            let timeframe_minutes = conditional_job.check_interval_seconds / 60; // Chuyển từ giây sang phút
            let price_history = if let Some(ref initial_bands) = conditional_job.initial_bands {
                println!("📊 Khởi tạo Bollinger Bands với giá trị thủ công:");
                println!("   Upper Band: {:.4}", initial_bands.upper_band);
                println!("   Middle Band: {:.4}", initial_bands.middle_band);
                println!("   Lower Band: {:.4}", initial_bands.lower_band);
                println!("   → Bot sẽ có thể chạy ngay lập tức!");
                let mut history = PriceHistory::new_with_initial_bands(
                    period,
                    initial_bands,
                    conditional_job.std_dev,
                );
                // Cập nhật timeframe từ config
                history.timeframe_minutes = timeframe_minutes;
                Arc::new(Mutex::new(history))
            } else {
                Arc::new(Mutex::new(PriceHistory::new_with_timeframe(period, timeframe_minutes)))
            };

            let kp_buf = keypair_buf.clone();
            let job_clone = conditional_job.clone();
            let price_history_clone = price_history.clone();
            let app_state_clone = app_state.clone();

            // Tạo task kiểm tra định kỳ (nến 5 phút = 300 giây)
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

            // Nến 5 phút = kiểm tra mỗi 5 phút
            // Format: second minute hour day month weekday
            // Chạy mỗi 5 phút ở giây 0: "0 */5 * * * *"
            // Hoặc sử dụng check_interval_seconds từ config nếu có
            let interval_seconds = conditional_job.check_interval_seconds;
            let cron_str = if interval_seconds >= 60 {
                // Nếu >= 60 giây, tính số phút
                let minutes = interval_seconds / 60;
                if minutes == 1 {
                    "0 * * * * *".to_string() // Mỗi phút
                } else {
                    format!("0 */{} * * * *", minutes) // Mỗi N phút
                }
            } else {
                // Nếu < 60 giây, chạy mỗi N giây (không khuyến nghị)
                format!("*/{} * * * * *", interval_seconds)
            };

            println!("Building conditional task {} with {}-minute candle interval (every {} seconds)", 
                     i + 1, interval_seconds / 60, interval_seconds);
            println!("   📅 Cron string: {}", cron_str);

            let task = TaskBuilder::default()
                .set_task_id(i.try_into().unwrap())
                .set_frequency_repeated_by_cron_str(&cron_str)
                .spawn_async_routine(body)?;

            delay_timer.add_task(task)?;
            i = i + 1;
        }
    }
    
    // Cập nhật số dư định kỳ (ngay lập tức và sau đó mỗi 1 giờ)
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
    
    // Cập nhật số dư sau 10 giây khi khởi động (để đảm bảo RPC sẵn sàng)
    let app_state_delayed = app_state.clone();
    let keypair_delayed_buf = keypair_buf.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let kp = Keypair::from_bytes(&keypair_delayed_buf).unwrap();
        if let Err(e) = update_balances(&app_state_delayed, &kp).await {
            eprintln!("⚠️  Error updating balances after startup: {}", e);
        } else {
            println!("✅ Balances updated after startup (10 seconds delay)");
        }
    });
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes = 300 seconds (thay vì 1 giờ)
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

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct InitialBands {
    upper_band: f64,
    middle_band: f64,
    lower_band: f64,
}

#[derive(Deserialize, Clone, serde::Serialize)]
struct ConditionalJob {
    #[serde(rename = "type")]
    job_type: String, // "bollinger_bands"
    input_mint: String,
    output_mint: String,
    base_mint: String, // Token để tính giá (ví dụ: USDC)
    quote_mint: String, // Token để so sánh (ví dụ: SOL)
    buy_amount: f64, // Số lượng SOL mua
    sell_amount: f64, // Số lượng SOL bán
    period: usize, // Số kỳ cho Bollinger Bands (mặc định 20)
    std_dev: f64, // Độ lệch chuẩn (mặc định 2.0)
    check_interval_seconds: u64, // Khoảng thời gian kiểm tra (giây)
    #[serde(default)]
    initial_bands: Option<InitialBands>, // Giá trị Bollinger Bands ban đầu (tùy chọn)
}

// Lưu trữ lịch sử giá cho Bollinger Bands (kèm timestamp)
struct PriceHistory {
    prices: Vec<PricePoint>,
    period: usize,     // số kỳ để tính Bollinger
    max_store: usize,  // số điểm lưu tối đa để hiển thị (vd 60 điểm = 5 giờ với timeframe 5 phút)
    last_added_time: Option<DateTime<Utc>>, // Thời gian thêm giá cuối cùng
    timeframe_minutes: u64, // Timeframe (phút) - mặc định 5 phút
}

impl PriceHistory {
    fn new(period: usize) -> Self {
        let max_store = 60_usize.max(period);
        Self {
            prices: Vec::with_capacity(max_store),
            period,
            max_store,
            last_added_time: None,
            timeframe_minutes: 5, // Mặc định 5 phút
        }
    }
    
    // Tạo mới với timeframe tùy chỉnh
    fn new_with_timeframe(period: usize, timeframe_minutes: u64) -> Self {
        let max_store = 60_usize.max(period);
        Self {
            prices: Vec::with_capacity(max_store),
            period,
            max_store,
            last_added_time: None,
            timeframe_minutes,
        }
    }
    
    // Khởi tạo với giá trị Bollinger Bands ban đầu
    // Tạo đủ số lượng giá giả lập để khi tính Bollinger Bands sẽ ra đúng giá trị đã nhập
    fn new_with_initial_bands(period: usize, initial_bands: &InitialBands, std_dev: f64) -> Self {
        let max_store = 60_usize.max(period);
        let timeframe_minutes = 5; // Timeframe 5 phút
        let mut prices = Vec::with_capacity(max_store);
        
        // Tính toán độ lệch chuẩn từ các giá trị đã cho
        // upper_band = middle_band + (std_dev * std)
        // lower_band = middle_band - (std_dev * std)
        // Lấy trung bình của 2 giá trị để đảm bảo tính nhất quán
        let std_upper = (initial_bands.upper_band - initial_bands.middle_band) / std_dev;
        let std_lower = (initial_bands.middle_band - initial_bands.lower_band) / std_dev;
        let std = (std_upper + std_lower) / 2.0;
        
        // Tạo đủ period giá giả lập xung quanh middle_band
        // Để khi tính Bollinger Bands sẽ ra đúng giá trị đã nhập
        // Sử dụng phân phối chuẩn với độ lệch chuẩn đã tính
        let now = Utc::now();
        for i in 0..period {
            // Tạo giá dao động xung quanh middle_band
            // Sử dụng phân phối chuẩn giả lập với độ lệch chuẩn std
            // Tạo giá từ -2*std đến +2*std để bao phủ toàn bộ phạm vi
            let normalized_pos = (i as f64 / (period - 1) as f64) * 2.0 - 1.0; // Từ -1 đến +1
            let z_score = normalized_pos * 2.0; // Từ -2 đến +2 (bao phủ ~95% phân phối chuẩn)
            let offset = z_score * std;
            let price = initial_bands.middle_band + offset;
            
            // Tính timestamp giả lập (mỗi giá cách nhau 5 phút cho timeframe 5 phút)
            let timestamp = now - chrono::Duration::minutes((period - i) as i64 * timeframe_minutes as i64);
            prices.push(PricePoint { price, timestamp });
        }
        
        Self {
            prices,
            period,
            max_store,
            last_added_time: Some(now),
            timeframe_minutes,
        }
    }

    fn add_price(&mut self, price: f64, timestamp: DateTime<Utc>) {
        // Chỉ thêm giá nếu đã đủ timeframe (mặc định 5 phút) kể từ lần thêm cuối
        let should_add = match self.last_added_time {
            None => true, // Chưa có giá nào, thêm ngay
            Some(last_time) => {
                let elapsed = timestamp.signed_duration_since(last_time);
                let elapsed_minutes = elapsed.num_minutes();
                let required_minutes = self.timeframe_minutes as i64;
                elapsed_minutes >= required_minutes
            }
        };
        
        if should_add {
            self.prices.push(PricePoint { price, timestamp });
            self.last_added_time = Some(timestamp);
            if self.prices.len() > self.max_store {
                self.prices.remove(0);
            }
        } else {
            // Nếu chưa đủ timeframe, cập nhật giá cuối cùng thay vì thêm mới
            // Điều này đảm bảo mỗi nến chỉ có 1 giá (giá cuối cùng trong khoảng thời gian đó)
            if let Some(last_point) = self.prices.last_mut() {
                last_point.price = price;
                last_point.timestamp = timestamp;
            }
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
    
    // Lưu số dư ban đầu nếu chưa có
    {
        let mut initial_balances = app_state.initial_balances.write().await;
        if initial_balances.is_none() {
            *initial_balances = Some(balances.clone());
            println!("📊 Initial balances saved");
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
        let was_added = history.prices.len();
        history.add_price(current_price, now);
        let count = history.prices.len();
        let actually_added = count > was_added;
        if actually_added {
            println!("📊 Price history: {}/{} prices collected (✅ Added new candle)", count, job.period);
        } else {
            println!("📊 Price history: {}/{} prices collected (⏸️  Updated existing candle - waiting for {} min timeframe)", 
                     count, job.period, history.timeframe_minutes);
        }
        // Snapshot giá kèm timestamp để lưu (cap 60 điểm = 5 giờ với timeframe 5 phút)
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
            // Lưu tối đa 60 điểm để biểu đồ 5 giờ (với timeframe 5 phút)
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

    // Kiểm tra cooldown để tránh swap liên tục (tối thiểu 5 phút = 300 giây giữa các lần swap)
    const MIN_SWAP_INTERVAL_SECS: u64 = 300;
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
    
    // Thêm buffer để tránh giao dịch quá sớm
    // Chỉ mua khi giá < Lower Band - 0.1% để đảm bảo có đủ biên độ lợi nhuận
    let buy_threshold = lower_band * 0.999; // Lower Band - 0.1%
    
    if current_price <= buy_threshold {
        // Mua SOL khi giá đóng nến dưới ngưỡng mua (Lower Band - 0.1%)
        println!("🚀 TÍN HIỆU MUA: Giá ({:.4}) <= Buy Threshold ({:.4}, Lower Band: {:.4})", 
                 current_price, buy_threshold, lower_band);
        
        // Sử dụng buy_amount từ config (số lượng SOL)
        let sol_amount = job.buy_amount;
        
        // Tính số lượng USDC cần để mua SOL
        // current_price là giá SOL/USDC, nên để mua sol_amount SOL cần: sol_amount * current_price USDC
        let usdc_needed = sol_amount * current_price;
        
        println!("   → Mua {} SOL từ USDC (cần {} USDC)", sol_amount, usdc_needed);
        
        // Kiểm tra số dư USDC trước khi swap
        let rpc_client = RpcClient::new_with_commitment(
            "https://api.mainnet-beta.solana.com".into(),
            CommitmentConfig::confirmed(),
        );
        let usdc_mint = Pubkey::try_from(job.input_mint.as_str())?;
        let usdc_address = get_associated_token_address(&keypair.pubkey(), &usdc_mint);
        let usdc_balance = match rpc_client.get_token_account(&usdc_address).await {
            Ok(Some(acc)) => acc.token_amount.ui_amount.unwrap_or(0.0),
            Ok(None) => {
                eprintln!("   ⚠️  Token account USDC chưa được khởi tạo!");
                eprintln!("   → Đang khởi tạo token account...");
                if let Err(e) = maybe_init_token_account(&rpc_client, &keypair, usdc_mint).await {
                    eprintln!("   ❌ Không thể khởi tạo token account: {}", e);
                    return Ok(());
                }
                // Sau khi khởi tạo, kiểm tra lại
                match rpc_client.get_token_account(&usdc_address).await {
                    Ok(Some(acc)) => acc.token_amount.ui_amount.unwrap_or(0.0),
                    _ => 0.0,
                }
            }
            Err(e) => {
                eprintln!("   ⚠️  Không thể kiểm tra số dư USDC: {}", e);
                0.0
            }
        };
        
        if usdc_balance < usdc_needed {
            eprintln!("   ❌ Không đủ USDC! Cần {} USDC nhưng chỉ có {} USDC", usdc_needed, usdc_balance);
            eprintln!("   → Bỏ qua lệnh mua này");
            return Ok(());
        }
        
        println!("   ✅ Số dư USDC: {} (đủ để mua)", usdc_balance);
        
        // Lưu keypair bytes để cập nhật số dư sau swap
        let keypair_bytes = keypair.to_bytes().to_vec();
        
        let result = swap_with_record(
            input_mint,  // USDC
            output_mint, // SOL
            usdc_needed, // Số lượng USDC
            1.5,         // Slippage 1.5% (tăng để giảm số lần thất bại)
            keypair,
            "BUY".to_string(),
            current_price,
            upper_band,
            lower_band,
            app_state.clone(),
            keypair_bytes, // Thêm keypair bytes
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
        
    } else {
        // Thêm buffer và kiểm tra spread trước khi bán
        // Chỉ bán khi giá > Upper Band + 0.1% để đảm bảo có đủ biên độ lợi nhuận
        let sell_threshold = upper_band * 1.001; // Upper Band + 0.1%
        
        // Kiểm tra spread: cần đảm bảo giá bán cao hơn giá mua trung bình ít nhất 0.2%
        // Để bù đắp phí giao dịch và slippage
        let min_profit_spread = 0.002; // 0.2% spread tối thiểu
        
        if current_price >= sell_threshold {
        // Kiểm tra xem có đủ spread để có lợi nhuận không
        // Lấy giá mua trung bình từ lịch sử giao dịch gần đây
        let recent_buy_price = {
            let trades = app_state.trades.read().await;
            let buy_trades: Vec<_> = trades.iter()
                .filter(|t| t.order_type == "BUY" && t.status == "SUCCESS")
                .rev()
                .take(5) // Lấy 5 giao dịch mua gần nhất
                .collect();
            
            if !buy_trades.is_empty() {
                let avg_buy_price: f64 = buy_trades.iter()
                    .map(|t| t.price)
                    .sum::<f64>() / buy_trades.len() as f64;
                Some(avg_buy_price)
            } else {
                None
            }
        };
        
        // Kiểm tra spread
        if let Some(avg_buy) = recent_buy_price {
            let spread = (current_price - avg_buy) / avg_buy;
            if spread < min_profit_spread {
                println!("⏸️  TÍN HIỆU BÁN NHƯNG KHÔNG ĐỦ SPREAD:");
                println!("   Giá hiện tại: {:.4}", current_price);
                println!("   Giá mua trung bình: {:.4}", avg_buy);
                println!("   Spread: {:.4}% (cần ít nhất {:.2}%)", spread * 100.0, min_profit_spread * 100.0);
                println!("   → Bỏ qua lệnh bán để tránh lỗ do phí và slippage");
                return Ok(());
            }
            println!("✅ Spread đủ: {:.4}% (cần ít nhất {:.2}%)", spread * 100.0, min_profit_spread * 100.0);
        }
        
        // Bán SOL khi giá đóng nến trên ngưỡng bán (Upper Band + 0.1%)
        println!("📉 TÍN HIỆU BÁN: Giá ({:.4}) >= Sell Threshold ({:.4}, Upper Band: {:.4})", 
                 current_price, sell_threshold, upper_band);
        
        // Sử dụng sell_amount từ config
        let sol_amount = job.sell_amount;
        println!("   → Bán {} SOL để lấy USDC", sol_amount);
        
        // Kiểm tra số dư SOL trước khi swap
        let rpc_client = RpcClient::new_with_commitment(
            "https://api.mainnet-beta.solana.com".into(),
            CommitmentConfig::confirmed(),
        );
        let sol_balance = match rpc_client.get_balance(&keypair.pubkey()).await {
            Ok(balance) => amount_to_ui_amount(balance, native_mint::DECIMALS),
            Err(e) => {
                eprintln!("   ⚠️  Không thể kiểm tra số dư SOL: {}", e);
                0.0
            }
        };
        
        // Cần thêm một chút SOL để trả phí giao dịch (~0.000005 SOL)
        let min_sol_needed = sol_amount + 0.001; // Thêm buffer cho phí
        
        if sol_balance < min_sol_needed {
            eprintln!("   ❌ Không đủ SOL! Cần {} SOL (bao gồm phí) nhưng chỉ có {} SOL", min_sol_needed, sol_balance);
            eprintln!("   → Bỏ qua lệnh bán này");
            return Ok(());
        }
        
        println!("   ✅ Số dư SOL: {} (đủ để bán)", sol_balance);
        
        // Lưu keypair bytes để cập nhật số dư sau swap
        let keypair_bytes = keypair.to_bytes().to_vec();
        
        let result = swap_with_record(
            output_mint, // SOL (bán)
            input_mint,  // USDC (nhận)
            sol_amount,  // Số lượng SOL
            1.5,         // Slippage 1.5% (tăng để giảm số lần thất bại)
            keypair,
            "SELL".to_string(),
            current_price,
            upper_band,
            lower_band,
            app_state.clone(),
            keypair_bytes, // Thêm keypair bytes
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
    keypair_bytes: Vec<u8>, // Thêm keypair bytes để cập nhật số dư sau swap
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
    
    // Tính số lượng SOL thực tế đã giao dịch
    // BUY: ui_amount là USDC, SOL = USDC / price
    // SELL: ui_amount là SOL
    let sol_amount = if order_type == "BUY" {
        if entry_price > 0.0 {
            ui_amount / entry_price
        } else {
            0.0
        }
    } else {
        ui_amount // SELL: ui_amount is already SOL
    };
    
    // Lưu giao dịch vào state
    let trade_record = TradeRecord {
        timestamp: start_time,
        order_type: order_type.clone(),
        amount: sol_amount, // SOL amount from config
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
    
    // Cập nhật số dư ngay sau khi swap thành công
    if result.is_ok() {
        // Đợi một chút để transaction được confirm
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Tạo lại keypair từ bytes để cập nhật số dư
        let kp = Keypair::from_bytes(&keypair_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to recreate keypair: {}", e))?;
        
        // Cập nhật số dư ngay lập tức
        if let Err(e) = update_balances(&app_state, &kp).await {
            eprintln!("   ⚠️  Không thể cập nhật số dư ngay lập tức: {}", e);
            eprintln!("   → Số dư sẽ được cập nhật trong lần kiểm tra tiếp theo");
        } else {
            println!("   ✅ Đã cập nhật số dư sau swap");
        }
    }
    
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
    
    // Kiểm tra Jupiter service
    let jupiter_status = {
        println!("  → Kiểm tra Jupiter service...");
        const JUPITER_SERVICE_URL: &str = "http://localhost:3002";
        
        match tokio::time::timeout(
            Duration::from_secs(5),
            reqwest::Client::new()
                .get(&format!("{}/health", JUPITER_SERVICE_URL))
                .send()
        ).await {
            Ok(Ok(resp)) => {
                if resp.status().is_success() {
                    println!("  ✅ Jupiter service: Kết nối thành công");
                    "connected".to_string()
                } else {
                    eprintln!("  ❌ Jupiter service: HTTP status {}", resp.status());
                    "disconnected".to_string()
                }
            },
            Ok(Err(e)) => {
                eprintln!("  ❌ Jupiter service: Lỗi kết nối - {}", e);
                "disconnected".to_string()
            },
            Err(_) => {
                eprintln!("  ❌ Jupiter service: Timeout sau 5 giây");
                "disconnected".to_string()
            },
        }
    };
    
    // Cập nhật trạng thái
    {
        let mut status = app_state.bot_status.write().await;
        status.api_status = ApiConnectionStatus {
            rpc_endpoint: rpc_status.clone(),
            jupiter_service: jupiter_status.clone(),
            last_check: Utc::now(),
            rpc_url: rpc_url.clone(),
        };
    }
    
    println!("🔍 Kết quả kiểm tra API:");
    println!("   - RPC Endpoint: {} ({:?})", rpc_status, rpc_url);
    println!("   - Jupiter Service: {}", jupiter_status);
    
    // Kiểm tra xem có sẵn sàng swap không
    if jupiter_status == "connected" && rpc_status == "connected" {
        println!("✅ Sẵn sàng swap!");
    } else {
        eprintln!("⚠️  Chưa sẵn sàng swap - Kiểm tra kết nối mạng và DNS");
    }
}

// Hàm swap chính - Chỉ sử dụng Jupiter
async fn swap(
    input_mint: Pubkey,
    output_mint: Pubkey,
    ui_amount: f64,
    slippage: f64,
    _keypair: Keypair,
) -> Result<SwapResult> {
    swap_via_jupiter_service(input_mint, output_mint, ui_amount, slippage).await
}

// Swap via Jupiter service
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

