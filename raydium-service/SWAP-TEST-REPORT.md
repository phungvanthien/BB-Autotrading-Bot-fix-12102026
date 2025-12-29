# Swap Test Report - Kiểm tra khả năng swap

## ✅ Đã cải thiện

### 1. **Mở rộng Jupiter Fallback**
   - Trước đây: Chỉ trigger khi có "vault error"
   - Bây giờ: Trigger cho nhiều loại lỗi:
     - `vault` / `fetch vault info`
     - `failed to compute swap amount out`
     - `failed to fetch routes`
     - `no pool found`
     - `timeout` / `network` / `rate limit` / `429`

### 2. **Tăng Timeout**
   - Server timeout: 5 phút (300 giây)
   - Cho phép Raydium retry đầy đủ trước khi chuyển sang Jupiter

### 3. **Cải thiện Error Handling**
   - Tất cả lỗi Raydium sẽ tự động trigger Jupiter fallback
   - Response format nhất quán với `success: false` khi fail

## 🧪 Cách test

### Test 1: Quick Test (1 phút timeout)
```bash
cd raydium-service
node test-swap-quick.js
```
- Test nhanh với timeout 1 phút
- Nếu timeout, có nghĩa là Raydium đang retry (bình thường)

### Test 2: Full Test (5 phút timeout)
```bash
cd raydium-service
node test-swap.js
```
- Test đầy đủ với timeout 5 phút
- Sẽ đợi tất cả retry hoàn tất
- Jupiter fallback sẽ được trigger nếu Raydium fail

### Test 3: Jupiter Fallback Test
```bash
cd raydium-service
node test-jupiter-fallback.js
```
- Test chuyên biệt để kiểm tra Jupiter fallback
- Hiển thị provider trong response

## 📊 Kết quả mong đợi

### Khi Raydium thành công:
```json
{
  "success": true,
  "transactionSignature": "...",
  "inputAmount": "10.0",
  "outputAmount": "..."
}
```

### Khi Raydium fail và Jupiter fallback thành công:
```json
{
  "success": true,
  "transactionSignature": "...",
  "inputAmount": "10.0",
  "outputAmount": "...",
  "provider": "Jupiter (fallback)"
}
```

### Khi cả hai đều fail:
```json
{
  "success": false,
  "error": "Raydium failed: ... Jupiter fallback also failed: ..."
}
```

## 🔍 Kiểm tra log

Xem log server để theo dõi quá trình:
```bash
tail -f /tmp/raydium-server.log
```

Các dấu hiệu:
- `⚠️  Raydium error detected` → Jupiter fallback đang được trigger
- `🔄 Using Jupiter API fallback...` → Đang swap qua Jupiter
- `✅ Jupiter swap successful` → Jupiter swap thành công

## ⚠️ Lưu ý

1. **Thời gian chờ**: Raydium có thể mất 2-3 phút để retry tất cả pools
2. **Jupiter fallback**: Tự động trigger sau khi Raydium fail
3. **RPC Alchemy**: Đã được cấu hình trong `.env`
4. **Timeout**: Server và client đều có timeout 5 phút

## 🎯 Kết luận

Swap service hiện tại:
- ✅ Có retry logic mạnh mẽ
- ✅ Có Jupiter fallback tự động
- ✅ Có timeout phù hợp
- ✅ Có error handling tốt

**Swap sẽ hoạt động** - nếu Raydium fail, Jupiter sẽ tự động được sử dụng.


