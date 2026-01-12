#!/bin/bash

echo "🔍 KIỂM TRA FILE .ENV"
echo "===================="
echo ""

# Đọc file .env
if [ ! -f .env ]; then
    echo "❌ File .env không tồn tại!"
    exit 1
fi

echo "✅ File .env tồn tại"
echo ""

# Kiểm tra các biến bắt buộc
ERRORS=0
WARNINGS=0

# 1. WALLET_PRIVATE_KEY (BẮT BUỘC)
echo "1. WALLET_PRIVATE_KEY:"
WALLET_KEY=$(grep "^WALLET_PRIVATE_KEY=" .env | cut -d'=' -f2- | tr -d ' ')
if [ -z "$WALLET_KEY" ] || [ "$WALLET_KEY" = "/path/to/your/keypair.json" ] || [ "$WALLET_KEY" = "your_base58_private_key_string_here" ]; then
    echo "   ❌ CHƯA ĐƯỢC ĐIỀN (vẫn là placeholder)"
    echo "   → Cần điền đường dẫn file keypair hoặc private key"
    ERRORS=$((ERRORS + 1))
else
    # Kiểm tra xem có phải file path không
    if [[ "$WALLET_KEY" == /* ]] || [[ "$WALLET_KEY" == ./* ]] || [[ "$WALLET_KEY" == ~/* ]]; then
        # Expand ~
        EXPANDED_PATH=$(echo "$WALLET_KEY" | sed "s|^~|$HOME|")
        if [ -f "$EXPANDED_PATH" ]; then
            echo "   ✅ Đã điền và file tồn tại: $WALLET_KEY"
        else
            echo "   ⚠️  Đã điền nhưng file không tồn tại: $WALLET_KEY"
            WARNINGS=$((WARNINGS + 1))
        fi
    else
        echo "   ✅ Đã điền (Base58 hoặc JSON array format)"
    fi
fi
echo ""

# 2. JUPITER_API_KEY (BẮT BUỘC cho Raydium service)
echo "2. JUPITER_API_KEY:"
JUPITER_KEY=$(grep "^JUPITER_API_KEY=" .env | cut -d'=' -f2- | tr -d ' ')
if [ -z "$JUPITER_KEY" ] || [ "$JUPITER_KEY" = "your_jupiter_api_key_here" ]; then
    echo "   ❌ CHƯA ĐƯỢC ĐIỀN (vẫn là placeholder)"
    echo "   → Cần lấy từ: https://station.jup.ag/docs/apis/lite-api"
    ERRORS=$((ERRORS + 1))
else
    if [[ "$JUPITER_KEY" == jup_* ]] || [[ "$JUPITER_KEY" == *api* ]] || [ ${#JUPITER_KEY} -gt 20 ]; then
        echo "   ✅ Đã điền (có vẻ hợp lệ)"
    else
        echo "   ⚠️  Đã điền nhưng có vẻ không đúng format"
        WARNINGS=$((WARNINGS + 1))
    fi
fi
echo ""

# 3. RPC_URL hoặc RPC_API_KEY (TÙY CHỌN nhưng khuyến nghị)
echo "3. RPC Configuration:"
RPC_URL=$(grep "^RPC_URL=" .env | cut -d'=' -f2- | tr -d ' ')
RPC_API_KEY=$(grep "^RPC_API_KEY=" .env | cut -d'=' -f2- | tr -d ' ')
if [ -n "$RPC_URL" ] && [ "$RPC_URL" != "https://api.mainnet-beta.solana.com" ]; then
    echo "   ✅ RPC_URL đã được cấu hình: $RPC_URL"
elif [ -n "$RPC_API_KEY" ] && [ "$RPC_API_KEY" != "your_helius_api_key_here" ] && [ "$RPC_API_KEY" != "your_alchemy_api_key_here" ]; then
    RPC_PROVIDER=$(grep "^RPC_PROVIDER=" .env | cut -d'=' -f2- | tr -d ' ')
    if [ -n "$RPC_PROVIDER" ]; then
        echo "   ✅ RPC_API_KEY đã được cấu hình với provider: $RPC_PROVIDER"
    else
        echo "   ⚠️  RPC_API_KEY đã điền nhưng thiếu RPC_PROVIDER"
        WARNINGS=$((WARNINGS + 1))
    fi
else
    echo "   ⚠️  Chưa cấu hình RPC (sẽ dùng RPC công cộng - có thể bị rate limit)"
    WARNINGS=$((WARNINGS + 1))
fi
echo ""

# Tổng kết
echo "===================="
echo "📊 KẾT QUẢ KIỂM TRA:"
echo "===================="
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo "✅ TẤT CẢ ĐỀU ỔN! Bot có thể chạy được."
elif [ $ERRORS -eq 0 ]; then
    echo "⚠️  Có $WARNINGS cảnh báo (bot vẫn có thể chạy nhưng không tối ưu)"
else
    echo "❌ Có $ERRORS lỗi cần sửa trước khi chạy bot"
    echo "⚠️  Có $WARNINGS cảnh báo"
fi
echo ""

if [ $ERRORS -gt 0 ]; then
    echo "🔧 CẦN SỬA:"
    echo "   1. Điền WALLET_PRIVATE_KEY với đường dẫn file keypair thực tế"
    echo "   2. Điền JUPITER_API_KEY (lấy từ https://station.jup.ag/docs/apis/lite-api)"
    echo ""
fi

if [ $WARNINGS -gt 0 ]; then
    echo "💡 KHUYẾN NGHỊ:"
    echo "   - Cấu hình RPC_API_KEY để tránh rate limit"
    echo "   - Xem hướng dẫn trong file .env"
    echo ""
fi

exit $ERRORS
