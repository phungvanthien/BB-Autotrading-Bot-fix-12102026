# 🚀 Khởi động nhanh

## Trạng thái hiện tại

✅ **Jupiter Service**: Đang chạy trên port 3002  
⚠️ **Raydium Service**: Cần JUPITER_API_KEY (không bắt buộc, bot vẫn chạy được)

## Khởi động Bot chính

Bot chính có thể chạy chỉ với Jupiter service. Chạy lệnh sau:

```bash
cargo run
```

Bot sẽ:
- ✅ Kết nối với ví của bạn
- ✅ Khởi động web dashboard trên http://localhost:8080
- ✅ Bắt đầu thực hiện các Bollinger Bands jobs

## Truy cập Dashboard

Sau khi bot chạy, mở trình duyệt:
```
http://localhost:8080
```

## Nếu muốn chạy Raydium service (tùy chọn)

Raydium service cần `JUPITER_API_KEY`. Nếu bạn muốn chạy:

1. Lấy API key từ: https://station.jup.ag/docs/apis/lite-api
2. Tạo file `raydium-service/.env`:
   ```bash
   JUPITER_API_KEY=your_api_key_here
   WALLET_PRIVATE_KEY=your_wallet_key_here
   RPC_URL=your_rpc_url_here
   ```
3. Khởi động lại:
   ```bash
   cd raydium-service
   npm start
   ```

## Dừng services

```bash
# Tìm và dừng processes
pkill -f "node.*jupiter"
pkill -f "node.*raydium"
pkill -f "cargo run"
```

## Xem logs

```bash
# Jupiter service
tail -f logs/jupiter-service.log

# Raydium service (nếu có)
tail -f logs/raydium-service.log

# Bot chính
# Xem trong terminal chạy cargo run
```

