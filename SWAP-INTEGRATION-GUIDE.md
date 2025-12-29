# Hướng dẫn tích hợp Swap Service

## 🎯 Tổng quan

Dự án hiện có **2 swap services**:

1. **Jupiter Service** (Port 3002) - **PRIMARY** ⭐
   - Dựa trên Jupiter API v6
   - Ổn định, đơn giản, ít lỗi
   - **Được sử dụng làm phương thức chính**

2. **Raydium Service** (Port 3001) - **FALLBACK**
   - Dựa trên Raydium SDK v2
   - Phức tạp hơn, có thể gặp lỗi vault
   - **Tự động fallback khi Jupiter fail**

## 🚀 Cách sử dụng

### 1. Khởi động services

```bash
# Terminal 1: Jupiter service (primary)
cd jupiter-service
npm install
npm start

# Terminal 2: Raydium service (fallback - optional)
cd raydium-service
npm install
npm start
```

### 2. Rust bot tự động sử dụng

Khi Rust bot gọi hàm `swap()`, nó sẽ:
1. ✅ Thử **Jupiter service** trước (port 3002)
2. ⚠️ Nếu Jupiter fail → tự động fallback sang **Raydium service** (port 3001)

**Bạn không cần thay đổi code Rust** - nó đã được cập nhật tự động!

## 📋 API Endpoints

### Jupiter Service (Port 3002)

**Health Check:**
```bash
curl http://localhost:3002/health
```

**Swap:**
```bash
curl -X POST http://localhost:3002/swap \
  -H "Content-Type: application/json" \
  -d '{
    "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "outputMint": "So11111111111111111111111111111111111111112",
    "amountIn": "1.0",
    "slippage": 1.0
  }'
```

### Raydium Service (Port 3001)

**Health Check:**
```bash
curl http://localhost:3001/health
```

**Swap:**
```bash
curl -X POST http://localhost:3001/swap \
  -H "Content-Type: application/json" \
  -d '{
    "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "outputMint": "So11111111111111111111111111111111111111112",
    "amountIn": "1.0",
    "slippage": 1.0
  }'
```

## 🧪 Test

### Test Jupiter service
```bash
cd jupiter-service
node test-swap.js
```

### Test Raydium service
```bash
cd raydium-service
node test-swap.js
```

## 📊 So sánh

| Tính năng | Jupiter | Raydium |
|-----------|---------|---------|
| **Độ ổn định** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Dễ tích hợp** | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| **Tốc độ** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Hỗ trợ token** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **RPC requirements** | Thấp | Cao (cần API key) |
| **Vault errors** | ❌ Không có | ⚠️ Có thể gặp |

## ⚙️ Cấu hình

### Jupiter Service (.env)
```bash
RPC_URL=https://solana-mainnet.g.alchemy.com/v2/YOUR_API_KEY
WALLET_PRIVATE_KEY=~/.config/solana/id.json
PORT=3002
```

### Raydium Service (.env)
```bash
RPC_URL=https://solana-mainnet.g.alchemy.com/v2/YOUR_API_KEY
WALLET_PRIVATE_KEY=~/.config/solana/id.json
```

## 🔄 Flow hoạt động

```
Rust Bot
    │
    ├─→ swap()
    │
    ├─→ Try Jupiter Service (port 3002)
    │   │
    │   ├─→ ✅ Success → Return transaction signature
    │   │
    │   └─→ ❌ Fail → Fallback to Raydium
    │
    └─→ Try Raydium Service (port 3001)
        │
        ├─→ ✅ Success → Return transaction signature
        │
        └─→ ❌ Fail → Return error
```

## ⚠️ Lưu ý quan trọng

1. **Jupiter service là primary** - Nên chạy service này trước
2. **Raydium service là optional** - Chỉ cần khi Jupiter fail
3. **Cả 2 services dùng chung RPC và ví** - Đảm bảo cấu hình đúng
4. **Ví phải có đủ SOL** - Để trả phí gas
5. **Ví phải có đủ token** - Để swap

## 🐛 Troubleshooting

### Jupiter service không khởi động
```bash
# Kiểm tra port
lsof -i:3002

# Kiểm tra log
tail -f /tmp/jupiter-server.log

# Kiểm tra .env
cat jupiter-service/.env
```

### Raydium service không khởi động
```bash
# Kiểm tra port
lsof -i:3001

# Kiểm tra log
tail -f /tmp/raydium-server.log

# Kiểm tra .env
cat raydium-service/.env
```

### Swap fail từ Rust bot
1. Kiểm tra cả 2 services đang chạy
2. Kiểm tra health endpoints
3. Kiểm tra log của cả 2 services
4. Kiểm tra ví có đủ balance

## 📝 Tóm tắt

✅ **Jupiter service** đã được tích hợp làm **primary swap method**
✅ **Raydium service** vẫn hoạt động như **fallback**
✅ **Rust bot** tự động chọn service tốt nhất
✅ **Không cần thay đổi code** - Tất cả đã được cấu hình sẵn

**Swap sẽ hoạt động ổn định hơn với Jupiter API!** 🎉


