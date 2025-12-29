# Trạng thái Swap Raydium

## Vấn đề hiện tại

Lỗi "fetch vault info error" xảy ra khi Raydium SDK cố gắng fetch thông tin vault từ RPC cho các pools. Ngay cả với Alchemy RPC API key, vẫn gặp lỗi này.

## Nguyên nhân có thể

1. **Một số pools có vault addresses không hợp lệ**: Các pools được fetch từ API có thể chứa vault addresses không tồn tại hoặc không accessible
2. **SDK version**: Có thể SDK version hiện tại có bug hoặc cần cách sử dụng khác
3. **RPC limitations**: Ngay cả với API key, một số RPC operations vẫn có thể fail

## Đã thử

✅ Thêm retry logic với exponential backoff  
✅ Thử với nhiều batch sizes (10, 5, 3, 1 pools)  
✅ Skip pools bị lỗi và thử pools khác  
✅ Sử dụng Alchemy RPC với API key  
✅ Thêm Jupiter API fallback (chưa test đầy đủ)  

## Giải pháp đề xuất

### Option 1: Sử dụng Jupiter API thay thế (Khuyến nghị)

Jupiter API đơn giản hơn, không cần fetch vault info, và thường hoạt động tốt hơn:

```javascript
// Thay vì Raydium, sử dụng Jupiter trực tiếp
// Jupiter API: https://quote-api.jup.ag/v6/quote
// Jupiter Swap: https://quote-api.jup.ag/v6/swap
```

### Option 2: Filter pools tốt hơn

- Chỉ sử dụng pools có TVL cao (>$10k)
- Chỉ sử dụng pools active (status = 1)
- Skip pools có vault addresses bắt đầu bằng "11111" (có thể là invalid)

### Option 3: Sử dụng Raydium API trực tiếp

Thay vì SDK, có thể sử dụng Raydium API trực tiếp:
- https://api.raydium.io/v2/main/pairs
- https://api.raydium.io/v2/ammV3/ammPools

### Option 4: Chờ SDK update

Có thể đây là bug của SDK và sẽ được fix trong version mới.

## Cấu hình hiện tại

- RPC: Alchemy (đã cấu hình với API key)
- SDK: @raydium-io/raydium-sdk-v2@0.2.31-alpha
- Retry: 5 lần với exponential backoff
- Timeout: 90 giây cho vault fetching

## Next Steps

1. Test Jupiter API fallback
2. Hoặc chuyển sang Jupiter API hoàn toàn
3. Hoặc filter pools tốt hơn trước khi fetch vault info


