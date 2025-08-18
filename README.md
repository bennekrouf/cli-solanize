# Solana CLI Client

A disruptive, terminal-based Rust client for basic Solana operations with clean error handling and YAML configuration.

## Features

✅ **Wallet Management** - Generate and manage Solana wallets  
✅ **Balance Checking** - Query SOL balances  
✅ **Testnet Faucet** - Request SOL airdrops for testing  
✅ **Transaction Creation** - Create transfer transactions  
✅ **Transaction Broadcasting** - Send transactions to the network  
✅ **Token Swaps** - Jupiter-powered SOL ↔ USDC swaps  
✅ **Real-time Pricing** - Get current token prices  
✅ **Token Search** - Find tokens by symbol, name, or address  
✅ **Interactive Menu** - Clean terminal interface  
✅ **YAML Configuration** - Centralized parameter management  
✅ **Structured Logging** - Trace-based logging with configurable levels  

## Quick Start

```bash
# Clone and build
git clone <repository-url>
cd solana-cli-client
cargo build --release

# Run interactive mode
cargo run -- menu

# Or use direct commands
cargo run -- generate-wallet
cargo run -- balance
cargo run -- faucet --amount 2.0
cargo run -- swap --from SOL --to USDC --amount 1.5
cargo run -- price --token SOL
cargo run -- search --query "ray"
```

## Configuration

All parameters are managed in `config.yaml`:

```yaml
solana:
  network: "devnet"
  rpc_url: "https://api.devnet.solana.com"
  commitment: "confirmed"

wallet:
  keypair_path: "./wallet.json"

faucet:
  airdrop_amount: 1.0

jupiter:
  api_url: "https://quote-api.jup.ag/v6"
  price_api_url: "https://price.jup.ag/v4"
  slippage_bps: 50  # 0.5%

tokens:
  sol: "So11111111111111111111111111111111111111112"
  usdc: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"

logging:
  level: "info"
  format: "pretty"
```

## Commands

- `menu` - Interactive terminal menu (default)
- `generate-wallet` - Create new wallet keypair
- `balance` - Check current SOL balance  
- `faucet --amount <SOL>` - Request testnet airdrop
- `create-tx --to <ADDRESS> --amount <SOL>` - Create transaction
- `send-tx --signature <TX_DATA>` - Broadcast transaction
- `swap --from <TOKEN> --to <TOKEN> --amount <AMOUNT>` - Token swap via Jupiter
- `price --token <SYMBOL>` - Get current token price
- `search --query <TERM>` - Search tokens by symbol/name/address

## Error Handling

Comprehensive error types with clear messaging:
- Wallet not found
- Insufficient balance  
- Network connectivity issues
- Invalid addresses
- Transaction failures

## Future API Integration

Prepared for HTTP API endpoints:
- `POST /api/v1/auth/challenge/{wallet_address}`
- `POST /api/v1/auth/verify`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/transactions/create`
- `POST /api/v1/transactions/confirm`
- `GET /api/v1/transactions/history`

## Architecture

- **Modular Design** - Separated concerns (wallet, transactions, config)
- **Generic Error Handling** - No unwrap() calls
- **Async/Await** - Modern Rust patterns
- **Structured Configuration** - YAML-based parameters
- **Clean Logging** - Configurable tracing integration
