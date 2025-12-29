# Jupiter Swap Service

Service swap dựa trên Jupiter API - ổn định và đáng tin cậy hơn Raydium SDK.

## ✨ Tính năng

- ✅ **Jupiter API v6** - API swap phổ biến nhất trên Solana
- ✅ **Đơn giản và ổn định** - Không cần SDK phức tạp, ít lỗi hơn
- ✅ **Tự động retry** - Xử lý lỗi network tự động
- ✅ **Hỗ trợ nhiều token** - Swap bất kỳ token nào trên Solana
- ✅ **Slippage protection** - Bảo vệ khỏi slippage cao

## 🚀 Cài đặt

```bash
cd jupiter-service
npm install
```

## ⚙️ Cấu hình

Tạo file `.env`:

```bash
RPC_URL=https://solana-mainnet.g.alchemy.com/v2/YOUR_API_KEY
WALLET_PRIVATE_KEY=~/.config/solana/id.json
PORT=3002
```

## 📡 Chạy service

```bash
npm start
```

Service sẽ chạy trên `http://localhost:3002`

## 🧪 Test

```bash
node test-swap.js
```

## 📋 API Endpoints

### GET /health
Kiểm tra trạng thái service

### POST /quote
Lấy quote cho swap
```json
{
  "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "outputMint": "So11111111111111111111111111111111111111112",
  "amountIn": "10.0",
  "slippageBps": 100
}
```

### POST /swap
Thực hiện swap
```json
{
  "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "outputMint": "So11111111111111111111111111111111111111112",
  "amountIn": "10.0",
  "slippage": 1.0
}
```

Response:
```json
{
  "success": true,
  "transactionSignature": "...",
  "inputAmount": "10.0",
  "outputAmount": "...",
  "provider": "Jupiter"
}
```

## 🔄 Tích hợp với Rust Bot

Rust bot đã được cập nhật để sử dụng Jupiter service làm **primary**, Raydium làm **fallback**.

Khi bot gọi `swap()`, nó sẽ:
1. Thử Jupiter service trước (port 3002)
2. Nếu Jupiter fail, tự động fallback sang Raydium (port 3001)

## 📊 So sánh với Raydium

| Tính năng | Jupiter | Raydium |
|-----------|---------|---------|
| Độ ổn định | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| Dễ tích hợp | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| Tốc độ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| Hỗ trợ token | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| RPC requirements | Thấp | Cao |

## ⚠️ Lưu ý

- Đảm bảo ví có đủ SOL để trả phí gas
- Đảm bảo có đủ token để swap
- Slippage mặc định là 1% (100 bps)

## 🐛 Troubleshooting

### Service không khởi động
- Kiểm tra `.env` file có đúng không
- Kiểm tra port 3002 có bị chiếm không: `lsof -i:3002`

### Swap fail
- Kiểm tra log: `tail -f /tmp/jupiter-server.log`
- Kiểm tra ví có đủ balance không
- Kiểm tra RPC endpoint có hoạt động không


