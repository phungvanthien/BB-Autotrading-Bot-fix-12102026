# bollingerband-bot
A trading bot that automates swaps on the Solana blockchain using Bollinger Bands strategy. Useful for implementing a simple dollar-cost-averaging strategy. 

## Disclaimer
This project was created for educational purposes. Use at your own risk!

## Setup

### 1. Kết nối ví (Wallet Connection)

Tạo file `.env` trong thư mục gốc với một trong các cách sau:

#### Cách 1: Sử dụng file keypair (Khuyến nghị)
```bash
WALLET_PRIVATE_KEY=/path/to/your/keypair.json
```

**Ví dụ với Solana CLI:**
```bash
# Nếu bạn đã cài Solana CLI và có keypair mặc định
WALLET_PRIVATE_KEY=~/.config/solana/id.json

# Hoặc đường dẫn tuyệt đối
WALLET_PRIVATE_KEY=/Users/username/.config/solana/id.json
```

**Tạo keypair mới (nếu chưa có):**
```bash
solana-keygen new --outfile ~/my-wallet.json
# Sau đó dùng: WALLET_PRIVATE_KEY=~/my-wallet.json
```

#### Cách 2: Sử dụng Base58 private key
```bash
WALLET_PRIVATE_KEY=your_base58_private_key_string_here
```

**Lấy private key từ file keypair:**
```bash
# Từ Solana CLI keypair file
cat ~/.config/solana/id.json | jq -r '.[:64]' | xxd -r -p | base58

# Hoặc export từ Phantom/Backpack wallet (cẩn thận với bảo mật!)
```

#### Cách 3: Sử dụng JSON array (64 bytes)
```bash
WALLET_PRIVATE_KEY=[123,45,67,89,...]
```

**Lấy từ file keypair:**
```bash
cat ~/.config/solana/id.json
# Copy mảng 64 số đầu tiên
```

### 2. Cấu hình DCA Jobs

Set up a `config.json` file in root to represent the DCA jobs to schedule. For example:
```
{
    "jobs": [
        {
            "input_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "output_mint": "So11111111111111111111111111111111111111112",
            "amount": 0.0010,
            "cron": "0 */1 * ? * *"
        },
        {
            "input_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "output_mint": "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
            "amount": 0.0011,
            "cron": "0 */1 * ? * *"
        },
        {
            "input_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "output_mint": "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
            "amount": 0.0012,
            "cron": "0 */1 * ? * *"
        }
    ]
}
```

## Running

```bash
cargo run
```

Bot sẽ tự động:
- ✅ Kết nối với ví của bạn
- ✅ Hiển thị địa chỉ ví để xác nhận
- ✅ Bắt đầu thực hiện các DCA jobs và conditional jobs

## Lưu ý bảo mật

⚠️ **QUAN TRỌNG:**
- **KHÔNG** commit file `.env` vào git
- **KHÔNG** chia sẻ private key của bạn
- File `.env` đã được thêm vào `.gitignore` để bảo vệ
- Chỉ sử dụng ví dành riêng cho trading bot, không dùng ví chính
- Đảm bảo ví có đủ SOL để trả phí giao dịch và đủ token để swap

## Kiểm tra ví

Sau khi chạy bot, bạn sẽ thấy:
```
✅ Wallet connected: <your_wallet_address>
📝 Make sure this wallet has enough SOL and tokens for trading!
```

Kiểm tra số dư ví trên [Solana Explorer](https://explorer.solana.com/) hoặc [Solscan](https://solscan.io/)