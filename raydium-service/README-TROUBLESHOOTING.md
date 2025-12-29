# Troubleshooting Raydium Swap

## Các bước kiểm tra khi swap không hoạt động:

### 1. Kiểm tra Raydium Service đang chạy
```bash
curl http://localhost:3001/health
```

Kết quả mong đợi:
```json
{
  "status": "ok",
  "raydium_initialized": true,
  "wallet": "8kYFkPtGrCjSJJyxve8v2SHhb36CavTEDBfhbb9ftDBk"
}
```

### 2. Kiểm tra log khi swap
Khi có tín hiệu mua/bán, xem log trong terminal chạy raydium-service để thấy:
- Cấu trúc của swapResult
- Có execute() hay sendTx() method không
- Lỗi cụ thể nếu có

### 3. Các nguyên nhân thường gặp:

#### a) Token không có trong tokenMap
- **Triệu chứng**: "Token info not found for..."
- **Giải pháp**: Đảm bảo Raydium SDK đã load token list đầy đủ

#### b) Không đủ số dư
- **Triệu chứng**: Transaction failed hoặc insufficient balance
- **Giải pháp**: Kiểm tra số dư trong ví

#### c) SwapResult không có execute() method
- **Triệu chứng**: "Swap result does not have execute() method"
- **Giải pháp**: Code đã được cập nhật để hỗ trợ sendTx() và transaction property

#### d) RPC endpoint không hoạt động
- **Triệu chứng**: Timeout hoặc connection error
- **Giải pháp**: Kiểm tra RPC_URL trong .env file

### 4. Test thủ công
```bash
cd raydium-service
node test-swap.js
```

### 5. Xem log chi tiết
Code đã được cập nhật với logging chi tiết:
- Cấu trúc của swapResult
- Các methods có sẵn
- Lỗi cụ thể nếu có

Hãy xem log trong terminal khi swap để tìm nguyên nhân.
