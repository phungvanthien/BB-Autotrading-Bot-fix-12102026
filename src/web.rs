use actix_web::{web, App, HttpServer, Responder, HttpResponse, Result as ActixResult};
use actix_files::Files;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub mint: String,
    pub symbol: String,
    pub balance: f64,
    pub decimals: u8,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub timestamp: DateTime<Utc>,
    pub order_type: String, // "BUY" or "SELL"
    pub amount: f64,
    pub price: f64,
    pub upper_band: f64,
    pub lower_band: f64,
    pub entry_price: f64,
    pub fee: f64,          // phí ước tính theo SOL
    pub fee_usdc: f64,     // phí quy đổi sang USDC
    pub transaction_signature: String,
    pub status: String, // "SUCCESS" or "FAILED"
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BotStatus {
    pub is_running: bool,
    pub wallet_address: String,
    pub last_update: DateTime<Utc>,
    pub api_status: ApiConnectionStatus, // Trạng thái kết nối API/SDK
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiConnectionStatus {
    pub rpc_endpoint: String,     // "connected" | "disconnected" | "checking"
    pub raydium_service: String,  // "connected" | "disconnected" | "checking"
    pub last_check: DateTime<Utc>,
    pub rpc_url: Option<String>,  // RPC endpoint đang sử dụng
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub balances: Vec<TokenBalance>,
    pub initial_balances: Option<Vec<TokenBalance>>, // Số dư ban đầu khi bot khởi động
    pub bot_status: BotStatus,
    pub trades: Vec<TradeRecord>,
    pub current_price: Option<f64>,
    pub bollinger_bands: Option<BollingerBandsInfo>,
    pub price_history: Vec<PricePoint>, // Lịch sử giá để vẽ biểu đồ (kèm timestamp)
    pub band_history: Vec<BandPoint>,   // Lịch sử band để vẽ biểu đồ
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BollingerBandsInfo {
    pub upper_band: f64,
    pub middle_band: f64,
    pub lower_band: f64,
    pub current_price: f64,
}

// Dữ liệu giá + timestamp để vẽ biểu đồ
#[derive(Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub price: f64,
    pub timestamp: DateTime<Utc>,
}

// Dữ liệu band theo thời gian để vẽ biểu đồ
#[derive(Clone, Serialize, Deserialize)]
pub struct BandPoint {
    pub upper: Option<f64>,
    pub middle: Option<f64>,
    pub lower: Option<f64>,
}

// Shared state cho web server
#[derive(Clone)]
pub struct AppState {
    pub balances: Arc<RwLock<Vec<TokenBalance>>>,
    pub initial_balances: Arc<RwLock<Option<Vec<TokenBalance>>>>, // Số dư ban đầu khi bot khởi động
    pub bot_status: Arc<RwLock<BotStatus>>,
    pub trades: Arc<RwLock<Vec<TradeRecord>>>,
    pub current_price: Arc<RwLock<Option<f64>>>,
    pub bollinger_bands: Arc<RwLock<Option<BollingerBandsInfo>>>,
    pub price_history: Arc<RwLock<Vec<PricePoint>>>, // Lịch sử giá để vẽ biểu đồ
    pub band_history: Arc<RwLock<Vec<BandPoint>>>, // Lịch sử band để vẽ biểu đồ
    pub last_swap_time: Arc<RwLock<Option<std::time::Instant>>>, // Thời gian swap cuối cùng để tránh swap liên tục
}

impl AppState {
    pub fn new() -> Self {
        Self {
            balances: Arc::new(RwLock::new(Vec::new())),
            initial_balances: Arc::new(RwLock::new(None)),
            bot_status: Arc::new(RwLock::new(BotStatus {
                is_running: false,
                wallet_address: String::new(),
                last_update: Utc::now(),
                api_status: ApiConnectionStatus {
                    rpc_endpoint: "checking".to_string(),
                    raydium_service: "checking".to_string(),
                    last_check: Utc::now(),
                    rpc_url: None,
                },
            })),
            trades: Arc::new(RwLock::new(Vec::new())),
            current_price: Arc::new(RwLock::new(None)),
            bollinger_bands: Arc::new(RwLock::new(None)),
            price_history: Arc::new(RwLock::new(Vec::new())),
            band_history: Arc::new(RwLock::new(Vec::new())),
            last_swap_time: Arc::new(RwLock::new(None)),
        }
    }
}

// API endpoints
pub async fn get_data(state: web::Data<AppState>) -> ActixResult<impl Responder> {
    let balances = state.balances.read().await.clone();
    let initial_balances = state.initial_balances.read().await.clone();
    let bot_status = state.bot_status.read().await.clone();
    let trades = state.trades.read().await.clone();
    let current_price = *state.current_price.read().await;
    let bollinger_bands = state.bollinger_bands.read().await.clone();
    let price_history = state.price_history.read().await.clone();
    let band_history = state.band_history.read().await.clone();

    let response = ApiResponse {
        balances,
        initial_balances,
        bot_status,
        trades,
        current_price,
        bollinger_bands,
        price_history,
        band_history,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_balances(state: web::Data<AppState>) -> ActixResult<impl Responder> {
    let balances = state.balances.read().await.clone();
    Ok(HttpResponse::Ok().json(balances))
}

pub async fn get_bot_status(state: web::Data<AppState>) -> ActixResult<impl Responder> {
    let status = state.bot_status.read().await.clone();
    Ok(HttpResponse::Ok().json(status))
}

pub async fn get_trades(state: web::Data<AppState>) -> ActixResult<impl Responder> {
    let trades = state.trades.read().await.clone();
    Ok(HttpResponse::Ok().json(trades))
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/data", web::get().to(get_data))
            .route("/balances", web::get().to(get_balances))
            .route("/status", web::get().to(get_bot_status))
            .route("/trades", web::get().to(get_trades)),
    );
}

// Handler cho favicon và các file không tồn tại
async fn not_found() -> ActixResult<impl Responder> {
    Ok(HttpResponse::NotFound().body(""))
}

pub async fn start_web_server(state: AppState) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(init_routes)
            .service(Files::new("/", "./static").index_file("index.html"))
            // Xử lý favicon và các request không tồn tại
            .default_service(
                web::route().to(not_found)
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

