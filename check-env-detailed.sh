#!/bin/bash

echo "🔍 KIỂM TRA CHI TIẾT FILE .ENV CHO BOT CHÍNH (Rust)"
echo "=================================================="
echo ""

if [ ! -f .env ]; then
    echo "❌ File .env không tồn tại!"
    exit 1
fi

echo "✅ File .env tồn tại"
echo ""

# Đọc các biến
WALLET_KEY=$(grep "^WALLET_PRIVATE_KEY=" .env | cut -d'=' -f2- | tr -d ' ')
RPC_URL=$(grep "^RPC_URL=" .env | cut -d'=' -f2- | tr -d ' ')
PORT=$(grep "^PORT=" .env | cut -d'=' -f2- | tr -d ' ')
JUPITER_KEY=$(grep "^JUPITER_API_KEY=" .env | cut -d'=' -f2- | tr -d ' ')

ERRORS=0
WARNINGS=0

# 1. WALLET_PRIVATE_KEY (BẮT BUỘC cho bot chính)
echo "1. WALLET_PRIVATE_KEY (BẮT BUỘC cho bot chính):"
if [ -z "$WALLET_KEY" ]; then
    echo "   ❌ CHƯA ĐƯỢC ĐIỀN"
    ERRORS=$((ERRORS + 1))
else
    # Kiểm tra format
    if [[ "$WALLET_KEY" == /* ]] || [[ "$WALLET_KEY" == ./* ]] || [[ "$WALLET_KEY" == ~/* ]]; then
        EXPANDED_PATH=$(echo "$WALLET_KEY" | sed "s|^~|$HOME|")
        if [ -f "$EXPANDED_PATH" ]; then
            echo "   ✅ Đã điền (file path) - File tồn tại: $WALLET_KEY"
        else
            echo "   ⚠️  Đã điền (file path) - File không tồn tại: $WALLET_KEY"
            WARNINGS=$((WARNINGS + 1))
        fi
    elif [[ "$WALLET_KEY" == \[* ]]; then
        echo "   ✅ Đã điền (JSON array format)"
    elif [ ${#WALLET_KEY} -gt 50 ]; then
        echo "   ✅ Đã điền (Base58 format - ${#WALLET_KEY} ký tự)"
    else
        echo "   ⚠️  Đã điền nhưng format có vẻ không đúng"
        WARNINGS=$((WARNINGS + 1))
    fi
fi
echo ""

# 2. RPC_URL (TÙY CHỌN - bot có default)
echo "2. RPC_URL (TÙY CHỌN - có default trong code):"
if [ -z "$RPC_URL" ]; then
    echo "   ⚠️  Chưa cấu hình (bot sẽ dùng RPC mặc định trong code)"
    WARNINGS=$((WARNINGS + 1))
else
    if [[ "$RPC_URL" == *alchemy* ]] || [[ "$RPC_URL" == *helius* ]] || [[ "$RPC_URL" == *quiknode* ]]; then
        echo "   ✅ Đã cấu hình RPC premium: $RPC_URL"
    elif [[ "$RPC_URL" == *mainnet-beta.solana.com* ]]; then
        echo "   ⚠️  Đang dùng RPC công cộng (có thể bị rate limit)"
        WARNINGS=$((WARNINGS + 1))
    else
        echo "   ✅ Đã cấu hình: $RPC_URL"
    fi
fi
echo ""

# 3. PORT (TÙY CHỌN - chỉ cho Jupiter service)
echo "3. PORT (TÙY CHỌN - chỉ cho Jupiter service):"
if [ -z "$PORT" ]; then
    echo "   ℹ️  Chưa cấu hình (Jupiter service sẽ dùng default: 3002)"
else
    echo "   ✅ Đã cấu hình: $PORT"
fi
echo ""

# 4. JUPITER_API_KEY (KHÔNG CẦN cho bot chính Rust)
echo "4. JUPITER_API_KEY:"
if [ -z "$JUPITER_KEY" ]; then
    echo "   ℹ️  Không có (KHÔNG CẦN cho bot chính Rust)"
    echo "   → Chỉ cần nếu bạn chạy Raydium service riêng"
else
    echo "   ✅ Đã có (dùng cho Raydium service nếu chạy riêng)"
fi
echo ""

# Tổng kết
echo "=================================================="
echo "📊 KẾT QUẢ KIỂM TRA CHO BOT CHÍNH (Rust):"
echo "=================================================="

if [ $ERRORS -eq 0 ]; then
    echo "✅ BOT CHÍNH CÓ THỂ CHẠY ĐƯỢC!"
    echo ""
    echo "📝 Lưu ý:"
    echo "   - Bot chính (Rust) chỉ cần WALLET_PRIVATE_KEY"
    echo "   - Bot sẽ gọi Jupiter service (port 3002) và Raydium service (port 3001)"
    echo "   - Các service cần chạy riêng và có .env riêng của chúng"
    if [ $WARNINGS -gt 0 ]; then
        echo ""
        echo "⚠️  Có $WARNINGS cảnh báo (bot vẫn chạy được nhưng không tối ưu)"
    fi
else
    echo "❌ Có $ERRORS lỗi cần sửa trước khi chạy bot"
    if [ $WARNINGS -gt 0 ]; then
        echo "⚠️  Có $WARNINGS cảnh báo"
    fi
fi
echo ""

# Kiểm tra các service
echo "🔍 KIỂM TRA CÁC SERVICE:"
echo ""

# Jupiter service
if [ -d "jupiter-service" ]; then
    if [ -f "jupiter-service/.env" ]; then
        echo "   ✅ Jupiter service có file .env riêng"
    else
        echo "   ⚠️  Jupiter service chưa có .env (có thể dùng .env ở root)"
    fi
fi

# Raydium service
if [ -d "raydium-service" ]; then
    if [ -f "raydium-service/.env" ]; then
        RAYDIUM_JUPITER=$(grep "^JUPITER_API_KEY=" raydium-service/.env 2>/dev/null | cut -d'=' -f2- | tr -d ' ')
        if [ -n "$RAYDIUM_JUPITER" ] && [ "$RAYDIUM_JUPITER" != "your_jupiter_api_key_here" ]; then
            echo "   ✅ Raydium service có JUPITER_API_KEY trong .env riêng"
        else
            echo "   ⚠️  Raydium service cần JUPITER_API_KEY trong .env riêng"
        fi
    else
        echo "   ⚠️  Raydium service chưa có .env riêng"
    fi
fi

echo ""
exit $ERRORS
