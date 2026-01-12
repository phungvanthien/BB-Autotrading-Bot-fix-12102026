# 🧪 Hướng dẫn Test Swap

## Script test mua 1 SOL

Script `test-buy-1sol.js` cho phép bạn test swap mua 1 SOL từ USDC mà **KHÔNG ảnh hưởng** đến bot chính.

### Cách sử dụng

```bash
node test-buy-1sol.js
```

### Yêu cầu

1. **Jupiter service đang chạy**:
   ```bash
   cd jupiter-service
   npm start
   ```

2. **Đủ số dư USDC**:
   - Cần ít nhất ~136-140 USDC để mua 1 SOL (tùy giá hiện tại)
   - Kiểm tra số dư trên dashboard: http://localhost:8080

3. **Đủ SOL để trả phí**:
   - Cần ~0.001 SOL để trả phí giao dịch

### Quy trình

1. Script sẽ:
   - ✅ Kiểm tra Jupiter service
   - ✅ Lấy giá SOL/USDC từ Binance
   - ✅ Lấy quote từ Jupiter
   - ✅ Hiển thị thông tin giao dịch
   - ✅ Đợi 5 giây để bạn xác nhận (Ctrl+C để hủy)
   - ✅ Thực hiện swap

2. Sau khi swap:
   - ✅ Hiển thị transaction signature
   - ✅ Link đến Solscan để xem chi tiết
   - ✅ Bot chính vẫn chạy bình thường

### Lưu ý

- ⚠️ Script này **KHÔNG ảnh hưởng** đến thuật toán Bollinger Bands
- ⚠️ Bot chính sẽ tiếp tục chạy bình thường
- ⚠️ Số dư mới sẽ được cập nhật trong dashboard sau vài phút
- ⚠️ Đảm bảo có đủ USDC trước khi chạy

### Xử lý lỗi

Nếu gặp lỗi "Attempt to debit an account but found no record of a prior credit":
1. Kiểm tra số dư USDC trong ví
2. Đảm bảo token account USDC đã được khởi tạo
3. Kiểm tra xem có đủ USDC không (cần ~136-140 USDC)

### Kiểm tra số dư

Xem số dư trên dashboard:
```
http://localhost:8080
```

Hoặc kiểm tra trực tiếp:
```bash
curl http://localhost:8080/api/balances
```

