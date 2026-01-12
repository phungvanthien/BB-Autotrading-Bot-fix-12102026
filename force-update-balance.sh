#!/bin/bash

# Script để force update số dư trên dashboard
# Script này sẽ trigger bot cập nhật số dư bằng cách đợi đến lần cập nhật tiếp theo

echo "🔄 Đang chờ bot cập nhật số dư..."
echo ""

# Kiểm tra bot có đang chạy không
if ! curl -s http://localhost:8080/api/status > /dev/null 2>&1; then
    echo "❌ Bot không chạy! Vui lòng khởi động bot trước."
    exit 1
fi

echo "✅ Bot đang chạy"
echo ""
echo "📊 Số dư hiện tại trên dashboard:"
curl -s http://localhost:8080/api/balances | python3 -m json.tool 2>/dev/null | grep -E "symbol|balance" | head -6

echo ""
echo "⏳ Bot sẽ tự động cập nhật số dư:"
echo "   - Ngay sau khi swap thành công (đã thêm code)"
echo "   - Hoặc mỗi 5 phút (background task)"
echo ""
echo "💡 Để xem số dư mới nhất, refresh dashboard sau vài phút"
echo "   Hoặc đợi bot tự động cập nhật trong lần kiểm tra tiếp theo"

