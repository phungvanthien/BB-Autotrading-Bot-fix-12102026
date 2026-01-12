#!/bin/bash

# Script khởi động dự án Bollinger Bands Trading Bot
# Chạy tất cả services cần thiết

set -e

echo "🚀 KHỞI ĐỘNG BOLLINGER BANDS TRADING BOT"
echo "========================================"
echo ""

# Màu sắc cho output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Kiểm tra dependencies
echo "📦 Kiểm tra dependencies..."

# Kiểm tra Node.js
if ! command -v node &> /dev/null; then
    echo -e "${RED}❌ Node.js chưa được cài đặt${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Node.js: $(node --version)${NC}"

# Kiểm tra Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo (Rust) chưa được cài đặt${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Cargo: $(cargo --version)${NC}"

# Kiểm tra file .env
if [ ! -f .env ]; then
    echo -e "${RED}❌ File .env không tồn tại!${NC}"
    echo "   Vui lòng tạo file .env với WALLET_PRIVATE_KEY"
    exit 1
fi
echo -e "${GREEN}✅ File .env tồn tại${NC}"

echo ""
echo "🔧 Cài đặt dependencies (nếu cần)..."

# Cài đặt dependencies cho Jupiter service
if [ ! -d "jupiter-service/node_modules" ]; then
    echo "   📦 Cài đặt dependencies cho Jupiter service..."
    cd jupiter-service
    npm install
    cd ..
else
    echo -e "   ${GREEN}✅ Jupiter service dependencies đã sẵn sàng${NC}"
fi

# Cài đặt dependencies cho Raydium service
if [ ! -d "raydium-service/node_modules" ]; then
    echo "   📦 Cài đặt dependencies cho Raydium service..."
    cd raydium-service
    npm install
    cd ..
else
    echo -e "   ${GREEN}✅ Raydium service dependencies đã sẵn sàng${NC}"
fi

echo ""
echo "🌐 Khởi động các services..."
echo ""

# Tạo thư mục logs nếu chưa có
mkdir -p logs

# Function để cleanup khi exit
cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Đang dừng tất cả services...${NC}"
    kill $(jobs -p) 2>/dev/null || true
    wait
    echo -e "${GREEN}✅ Đã dừng tất cả services${NC}"
    exit 0
}

# Trap Ctrl+C
trap cleanup SIGINT SIGTERM

# Khởi động Jupiter service (port 3002)
echo -e "${YELLOW}1. Khởi động Jupiter service (port 3002)...${NC}"
cd jupiter-service
npm start > ../logs/jupiter-service.log 2>&1 &
JUPITER_PID=$!
cd ..
sleep 2

# Kiểm tra Jupiter service
if ps -p $JUPITER_PID > /dev/null; then
    echo -e "${GREEN}   ✅ Jupiter service đã khởi động (PID: $JUPITER_PID)${NC}"
else
    echo -e "${RED}   ❌ Jupiter service không khởi động được${NC}"
    echo "   Xem logs: tail -f logs/jupiter-service.log"
    exit 1
fi

# Khởi động Raydium service (port 3001)
echo -e "${YELLOW}2. Khởi động Raydium service (port 3001)...${NC}"
cd raydium-service
npm start > ../logs/raydium-service.log 2>&1 &
RAYDIUM_PID=$!
cd ..
sleep 3

# Kiểm tra Raydium service
if ps -p $RAYDIUM_PID > /dev/null; then
    echo -e "${GREEN}   ✅ Raydium service đã khởi động (PID: $RAYDIUM_PID)${NC}"
else
    echo -e "${RED}   ❌ Raydium service không khởi động được${NC}"
    echo "   Xem logs: tail -f logs/raydium-service.log"
    kill $JUPITER_PID 2>/dev/null || true
    exit 1
fi

# Đợi một chút để services sẵn sàng
echo ""
echo -e "${YELLOW}⏳ Đợi services sẵn sàng...${NC}"
sleep 3

# Kiểm tra health của các services
echo ""
echo "🔍 Kiểm tra health của các services..."

# Kiểm tra Jupiter service
if curl -s http://localhost:3002/health > /dev/null 2>&1; then
    echo -e "${GREEN}   ✅ Jupiter service: Healthy${NC}"
else
    echo -e "${YELLOW}   ⚠️  Jupiter service: Chưa sẵn sàng (có thể đang khởi tạo)${NC}"
fi

# Kiểm tra Raydium service
if curl -s http://localhost:3001/health > /dev/null 2>&1; then
    echo -e "${GREEN}   ✅ Raydium service: Healthy${NC}"
else
    echo -e "${YELLOW}   ⚠️  Raydium service: Chưa sẵn sàng (có thể đang khởi tạo SDK)${NC}"
fi

echo ""
echo "🤖 Khởi động bot chính (Rust)..."
echo ""

# Khởi động bot chính
cargo run 2>&1 | tee logs/bot.log &
BOT_PID=$!

# Đợi bot khởi động
sleep 5

# Kiểm tra bot
if ps -p $BOT_PID > /dev/null; then
    echo -e "${GREEN}   ✅ Bot đã khởi động (PID: $BOT_PID)${NC}"
else
    echo -e "${RED}   ❌ Bot không khởi động được${NC}"
    echo "   Xem logs: tail -f logs/bot.log"
    kill $JUPITER_PID $RAYDIUM_PID 2>/dev/null || true
    exit 1
fi

echo ""
echo "========================================"
echo -e "${GREEN}✅ TẤT CẢ SERVICES ĐÃ KHỞI ĐỘNG!${NC}"
echo "========================================"
echo ""
echo "📊 Thông tin services:"
echo "   • Jupiter service:  http://localhost:3002"
echo "   • Raydium service:   http://localhost:3001"
echo "   • Bot web dashboard: http://localhost:8080"
echo ""
echo "📝 Logs:"
echo "   • Bot:              tail -f logs/bot.log"
echo "   • Jupiter service:  tail -f logs/jupiter-service.log"
echo "   • Raydium service:  tail -f logs/raydium-service.log"
echo ""
echo -e "${YELLOW}💡 Nhấn Ctrl+C để dừng tất cả services${NC}"
echo ""

# Đợi cho đến khi nhận signal
wait

