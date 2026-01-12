# 🚀 Hướng dẫn khởi động dự án

## Cách 1: Khởi động tự động (Khuyến nghị)

Chạy script tự động để khởi động tất cả services:

```bash
./start.sh
```

Script này sẽ:
- ✅ Kiểm tra dependencies
- ✅ Cài đặt npm packages nếu cần
- ✅ Khởi động Jupiter service (port 3002)
- ✅ Khởi động Raydium service (port 3001)
- ✅ Khởi động bot chính (port 8080)

## Cách 2: Khởi động thủ công

### Bước 1: Cài đặt dependencies (nếu chưa có)

```bash
# Cài đặt dependencies cho Jupiter service
cd jupiter-service
npm install
cd ..

# Cài đặt dependencies cho Raydium service
cd raydium-service
npm install
cd ..
```

### Bước 2: Khởi động các services

Mở **3 terminal windows** riêng biệt:

#### Terminal 1: Jupiter Service
```bash
cd jupiter-service
npm start
```
Service sẽ chạy trên: http://localhost:3002

#### Terminal 2: Raydium Service
```bash
cd raydium-service
npm start
```
Service sẽ chạy trên: http://localhost:3001

#### Terminal 3: Bot chính
```bash
cargo run
```
Bot sẽ chạy trên: http://localhost:8080

## Kiểm tra services đã chạy

Sau khi khởi động, kiểm tra:

1. **Jupiter service**: http://localhost:3002/health
2. **Raydium service**: http://localhost:3001/health
3. **Bot dashboard**: http://localhost:8080

## Truy cập Dashboard

Mở trình duyệt và truy cập:
```
http://localhost:8080
```

Bạn sẽ thấy:
- 📊 Số dư token (SOL, USDC)
- 📈 Biểu đồ giá với Bollinger Bands
- 💼 Biểu đồ portfolio
- 📋 Lịch sử giao dịch

## Dừng services

- **Nếu dùng script**: Nhấn `Ctrl+C` trong terminal chạy script
- **Nếu chạy thủ công**: Nhấn `Ctrl+C` trong từng terminal

## Xử lý lỗi

### Lỗi: Port đã được sử dụng

Nếu gặp lỗi "port already in use":
```bash
# Kiểm tra process đang dùng port
lsof -i :3001  # Raydium
lsof -i :3002  # Jupiter
lsof -i :8080  # Bot

# Dừng process
kill -9 <PID>
```

### Lỗi: Dependencies chưa cài

```bash
# Cài lại dependencies
cd jupiter-service && npm install && cd ..
cd raydium-service && npm install && cd ..
```

### Lỗi: Bot không kết nối được services

Đảm bảo:
1. Jupiter service đã chạy trên port 3002
2. Raydium service đã chạy trên port 3001
3. Kiểm tra file `.env` đã đúng

## Logs

- Bot logs: Xem trong terminal chạy `cargo run`
- Service logs: Xem trong terminal chạy `npm start`
- Nếu dùng script: Xem trong thư mục `logs/`

