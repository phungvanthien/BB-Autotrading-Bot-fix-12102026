# Test Swap Interface - Hướng dẫn sử dụng

## ✅ Đã thêm vào giao diện

### Section mới: "🧪 Test Swap: 0.1 SOL → USDC"

Section này cho phép bạn:
1. **Xem lệnh swap** - Hiển thị thông tin lệnh swap 0.1 SOL → USDC
2. **Thực hiện swap test** - Gọi API để thực hiện swap thực tế
3. **Xem quote** - Lấy quote trước khi swap
4. **Hiển thị kết quả** - Xem chi tiết kết quả swap trên giao diện

## 📋 Lệnh swap được hiển thị

```json
{
  "inputMint": "So11111111111111111111111111111111111111112",
  "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "amountIn": "0.1",
  "slippage": 1.0
}
```

**Chi tiết:**
- **Input:** 0.1 SOL
- **Output:** USDC
- **Slippage:** 1.0%
- **Service:** Jupiter API

## 🚀 Cách sử dụng

### 1. Mở giao diện dashboard
Truy cập: `http://localhost:8080` (hoặc port mà bot đang chạy)

### 2. Tìm section "🧪 Test Swap: 0.1 SOL → USDC"
Section này nằm giữa "Swap thủ công" và "Báo cáo Giao dịch"

### 3. Chọn hành động:
- **🚀 Thực hiện Swap Test** - Thực hiện swap thực tế 0.1 SOL → USDC
- **💰 Xem Quote** - Chỉ xem quote, không thực hiện swap

### 4. Xem kết quả
Kết quả sẽ hiển thị:
- ✅ Status (thành công/thất bại)
- 📊 Chi tiết swap (input, output, transaction)
- 🔗 Link đến Solscan để xem transaction
- 📝 JSON của lệnh swap và kết quả

## 📊 Kết quả hiển thị

### Khi swap thành công:
```
✅ Swap thành công!
- Provider: Jupiter
- Input: 0.1 SOL
- Output: [số lượng] USDC
- Transaction: [link đến Solscan]
```

### Khi swap thất bại:
```
❌ Swap thất bại
- Lỗi: [chi tiết lỗi]
- Lệnh swap đã thử: [JSON]
```

## ⚠️ Lưu ý

1. **Jupiter service phải chạy** - Đảm bảo service đang chạy trên port 3002
2. **Ví phải có đủ SOL** - Cần SOL để trả phí gas và swap
3. **Network connection** - Cần kết nối internet để gọi Jupiter API

## 🔧 Troubleshooting

### Service không khả dụng
```bash
# Kiểm tra service
curl http://localhost:3002/health

# Khởi động service nếu cần
cd jupiter-service
npm start
```

### Swap fail
- Kiểm tra log: `tail -f /tmp/jupiter-server.log`
- Kiểm tra ví có đủ balance
- Kiểm tra network connection

## 📝 Code đã thêm

### HTML Section
- Card mới với title "🧪 Test Swap: 0.1 SOL → USDC"
- 2 buttons: "Thực hiện Swap Test" và "Xem Quote"
- Result div để hiển thị kết quả

### JavaScript Functions
- `testSwap()` - Thực hiện swap 0.1 SOL → USDC
- `getSwapQuote()` - Lấy quote cho swap

### API Calls
- `POST /quote` - Lấy quote từ Jupiter service
- `POST /swap` - Thực hiện swap qua Jupiter service

## 🎯 Mục đích

Section này giúp:
- ✅ Test swap functionality dễ dàng
- ✅ Hiển thị lệnh swap rõ ràng
- ✅ Xem kết quả swap trực tiếp trên giao diện
- ✅ Debug swap issues nhanh chóng


