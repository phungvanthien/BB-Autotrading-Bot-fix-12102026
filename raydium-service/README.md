# Raydium Swap Service

Node.js service để wrap Raydium SDK V2 cho Rust bot.

## Cài đặt

```bash
cd raydium-service
npm install
```

## Cấu hình

Tạo file `.env`:

```bash
RPC_URL=https://api.mainnet-beta.solana.com
WALLET_PRIVATE_KEY=~/.config/solana/id.json
```

Hoặc có thể dùng:
- Base58 string: `WALLET_PRIVATE_KEY=your_base58_private_key`
- JSON array: `WALLET_PRIVATE_KEY=[1,2,3,...]`

## Chạy service

```bash
npm start
```

Service sẽ chạy trên `http://localhost:3001`

## API Endpoints

### GET /health
Kiểm tra trạng thái service

### POST /quote
Lấy quote cho swap
```json
{
  "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "outputMint": "So11111111111111111111111111111111111111112",
  "amountIn": "10.0",
  "slippage": 1.0
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

## Lưu ý

- Service phải chạy trước khi Rust bot khởi động
- Đảm bảo ví có đủ SOL và token để swap
