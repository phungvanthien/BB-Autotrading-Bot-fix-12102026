# Hướng dẫn khắc phục lỗi "fetch vault info error"

## Vấn đề
Lỗi "fetch vault info error" xảy ra khi Raydium SDK cố gắng fetch thông tin vault từ RPC endpoint. RPC endpoint công khai (public) thường bị rate limit và không đủ mạnh để xử lý nhiều requests.

## Giải pháp

### 1. Sử dụng RPC endpoint với API key (Khuyến nghị)

Thêm vào file `.env`:

```bash
# Sử dụng Helius (miễn phí 100k requests/ngày)
RPC_URL=https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY
# Hoặc
RPC_API_KEY=YOUR_API_KEY

# Hoặc sử dụng QuickNode
RPC_URL=https://YOUR_ENDPOINT.solana-mainnet.quiknode.pro/YOUR_API_KEY/

# Hoặc sử dụng Alchemy
RPC_URL=https://solana-mainnet.g.alchemy.com/v2/YOUR_API_KEY
```

### 2. Lấy API key miễn phí

- **Helius**: https://www.helius.dev/ (100k requests/ngày miễn phí)
- **QuickNode**: https://www.quicknode.com/ (có tier miễn phí)
- **Alchemy**: https://www.alchemy.com/ (có tier miễn phí)

### 3. Test RPC endpoint

Sau khi cấu hình, restart server và test:

```bash
cd raydium-service
npm start
# Trong terminal khác
node test-swap.js
```

### 4. Nếu vẫn gặp lỗi

- Kiểm tra API key có hợp lệ không
- Kiểm tra rate limit của RPC provider
- Thử RPC provider khác
- Tăng delay giữa các requests trong code

## Lưu ý

- RPC endpoint công khai (`https://api.mainnet-beta.solana.com`) thường bị rate limit
- Với API key, bạn sẽ có rate limit cao hơn và performance tốt hơn
- Một số RPC providers có tier miễn phí đủ cho testing


