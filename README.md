# Solana CLI Client

A disruptive, terminal-based Rust client for basic Solana operations with clean error handling and YAML configuration.

## Features

✅ **Wallet Management** - Generate and manage Solana wallets  
✅ **Balance Checking** - Query SOL balances  
✅ **Testnet Faucet** - Request SOL airdrops for testing  
✅ **Transaction Creation** - Create transfer transactions  
✅ **Transaction Broadcasting** - Send transactions to the network  
✅ **Token Swaps** - Jupiter-powered SOL ↔ USDC swaps  
✅ **Token Discovery** - Scan wallet for all SPL tokens with balances  
✅ **Real-time Pricing** - Get current token prices with USD values  
✅ **REST API Server** - Web services for all operations via HTTP endpoints  
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
cargo run -- list-tokens
cargo run -- web-server --port 8000
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

## Choosing the network per request

The server serves every network under `solana.networks` in `config.yaml`
(`mainnet`, the default, and `devnet`). A request picks one with an optional
field; leaving it out means `solana.network`:

```json
POST /solana/balance
{ "pubkey": "9WzD…", "network": "devnet" }
```

| Route | `network` |
|---|---|
| balance, wallet/tokens, transaction/prepare, transaction/submit, transactions/history, transactions/pending | honoured |
| swap/prepare | mainnet only — Jupiter has no devnet; anything else is refused |
| price, tokens/search | not taken — Jupiter data is always mainnet |

`wallet/tokens` on devnet returns no `usd_value`: devnet tokens have no market.
Submit a signed transaction to the same network it was prepared on — the
blockhash belongs to that cluster.

Numeric fields (`amount`, `limit`) accept a JSON number or a numeric string —
api0 sends every tool argument as a string. `transactions/history` returns at
most 10 per call (the default too): each is fetched separately, ~1.3 s on the
public RPC, and api0 gives a tool 30 s. Page with `before` for more.

Set `SOLANIZE_RPC_URL_MAINNET` (or `_DEVNET`) to use a private RPC; the public
mainnet endpoint is heavily rate-limited.

## Calling it through api0

The server binds to `127.0.0.1` only. Remote callers reach it through nginx at
`https://api.ribh.io/solana/*` ([deploy/nginx-api.ribh.io.conf](deploy/nginx-api.ribh.io.conf)),
the same way api0 reaches cvenom at `api.cvenom.com`, so api0 can run on this VPS or anywhere else.

Every route except `/solana/health` accepts one of:

| Caller | `Authorization: Bearer …` |
|---|---|
| gateway-solanize | `CLI_INTERNAL_SECRET` |
| api0 gateway | a Google OIDC token minted by the api0 tenant's service account |

The OIDC path is on when both are set (env, or `api0:` in `config.yaml`):

```bash
SOLANIZE_OIDC_AUDIENCE=https://api.ribh.io
SOLANIZE_OIDC_SERVICE_ACCOUNT=<api0 tenant service account email>
```

The token's audience and the service account that minted it are both checked;
any Google service account can mint a token for any audience, so the audience
alone proves nothing. On the api0 side the tenant uses downstream auth
`google_service_account` with `target_audience` set to the same URL.

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
