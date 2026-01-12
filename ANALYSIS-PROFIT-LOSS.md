# Phân Tích Lãi/Lỗ Trading Bot

## 📊 Tổng Quan

- **Số dư ban đầu**: 2000 USDC + 0.2 SOL (≈ 2027.24 USDC)
- **Số dư hiện tại**: 1855.94 USDC + 1.224538 SOL (≈ 2022.72 USDC)
- **Lỗ**: -4.52 USDC (-0.22%)

## 🔍 Nguyên Nhân Chính

### 1. **Spread Âm (Vấn Đề Nghiêm Trọng Nhất)**

- **Giá mua trung bình**: 136.1656 USDC/SOL
- **Giá bán trung bình**: 136.0898 USDC/SOL
- **Spread**: -0.0758 USDC (-0.06%)

**Vấn đề**: Bot đang mua với giá CAO HƠN giá bán! Điều này ngược lại với chiến lược "mua thấp bán cao".

**Nguyên nhân có thể**:
- Slippage khi swap (giá thực tế khác với giá tham chiếu)
- Price impact khi swap khối lượng lớn
- Spread của DEX (chênh lệch giữa giá bid và ask)
- Timing: Bot có thể đang mua/bán không đúng thời điểm tối ưu

### 2. **Slippage Tích Lũy**

Mỗi giao dịch có slippage khoảng 0.05-0.06%:
- Khi mua: Nhận được ít SOL hơn expected (~0.0006 SOL mỗi lần)
- Khi bán: Nhận được ít USDC hơn expected (~0.09 USDC mỗi lần)

Với 151 giao dịch, slippage tích lũy gây thiệt hại đáng kể.

### 3. **Số Lượng Giao Dịch Quá Nhiều**

- **Tổng giao dịch**: 151 lần
- **Mua**: 70 lần (70.027 SOL)
- **Bán**: 81 lần (81.000 SOL)

Bot đang giao dịch quá thường xuyên, dẫn đến:
- Phí giao dịch tích lũy
- Slippage tích lũy
- Không tận dụng được biến động giá lớn

### 4. **Phân Tích Chi Tiết**

**Ví dụ giao dịch**:
- SELL @ 136.0422: Expected 136.1306, Nhận được 136.0422 → Slippage -0.09 USDC (-0.06%)
- BUY @ 136.3974: Expected 1.0009 SOL, Nhận được 1.0003 SOL → Slippage -0.0006 SOL (-0.06%)

## 💡 Giải Pháp Đề Xuất

### 1. **Tăng Slippage Tolerance**
- Hiện tại: 1% slippage
- Đề xuất: Tăng lên 1.5-2% để giảm số lần giao dịch thất bại và cải thiện giá thực tế

### 2. **Tăng Cooldown Period**
- Hiện tại: 60 giây
- Đề xuất: Tăng lên 5-10 phút để tránh giao dịch quá thường xuyên

### 3. **Kiểm Tra Price Impact**
- Thêm kiểm tra price impact trước khi swap
- Không swap nếu price impact > 0.5%

### 4. **Cải Thiện Timing**
- Chờ giá vượt qua band một khoảng nhất định trước khi swap
- Không swap ngay khi giá chạm band

### 5. **Giảm Số Lượng Giao Dịch**
- Tăng `check_interval_seconds` từ 60 lên 300-600 giây
- Chỉ swap khi giá cách band một khoảng đáng kể

### 6. **Theo Dõi Spread**
- Thêm logic để theo dõi spread giữa giá mua và giá bán
- Không bán nếu spread < 0.1% (để bù đắp phí và slippage)

## 📈 Kết Luận

Bot đang hoạt động đúng logic (mua thấp, bán cao) nhưng bị thiệt hại do:
1. **Spread âm** từ slippage và price impact
2. **Giao dịch quá thường xuyên** dẫn đến phí và slippage tích lũy
3. **Slippage** mỗi giao dịch tuy nhỏ nhưng tích lũy lại đáng kể

**Khuyến nghị**: Cần điều chỉnh các thông số để giảm số lượng giao dịch và cải thiện timing của các giao dịch.

