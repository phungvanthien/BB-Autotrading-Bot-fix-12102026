#!/bin/bash

# Script để reset tất cả dữ liệu và restart bot với timeframe 5 phút

echo "🔄 Đang reset và restart bot với timeframe 5 phút..."
echo ""

# Dừng bot hiện tại nếu đang chạy
echo "⏹️  Dừng bot hiện tại..."
pkill -f "target/release/jup-dca" || echo "   Bot không đang chạy"
sleep 2

# Reset dữ liệu thông qua API (nếu bot đang chạy)
echo "🗑️  Xóa dữ liệu cũ..."
curl -X POST http://localhost:8080/api/reset-all-data 2>/dev/null || echo "   API không khả dụng (bot đã dừng)"

# Đợi một chút
sleep 1

# Build lại bot
echo "🔨 Build lại bot..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build thất bại!"
    exit 1
fi

echo ""
echo "✅ Build thành công!"
echo ""
echo "📊 Bot sẽ chạy với:"
echo "   • Timeframe: 5 phút (check_interval_seconds: 300)"
echo "   • Chiến lược: Bollinger Bands"
echo "   • Buy amount: 0.5 SOL"
echo "   • Sell amount: 0.5 SOL"
echo ""
echo "🚀 Khởi động bot..."
echo ""

# Chạy bot trong background
nohup ./target/release/jup-dca > bot.log 2>&1 &

# Đợi bot khởi động
sleep 3

# Kiểm tra bot có chạy không
if pgrep -f "target/release/jup-dca" > /dev/null; then
    echo "✅ Bot đã khởi động thành công!"
    echo ""
    echo "📝 Logs: tail -f bot.log"
    echo "🌐 Dashboard: http://localhost:8080"
    echo ""
    echo "📊 Bot sẽ bắt đầu thu thập dữ liệu với timeframe 5 phút"
    echo "   • Mỗi 5 phút: Lấy giá và cập nhật Bollinger Bands"
    echo "   • Cần 20 nến 5 phút để tính Bollinger Bands (100 phút = ~1.7 giờ)"
else
    echo "❌ Bot không khởi động được. Kiểm tra bot.log để xem lỗi."
    exit 1
fi

