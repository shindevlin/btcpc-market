# btcpc-market

Rust/Axum commerce sidecar for the [BTCPC](https://github.com/shindevlin/btcpc) sovereign blockchain.

Vendors run this alongside a BTCPC node to open a store, list products, and process escrow-protected orders — all recorded as signed ledger entries on the BTCPC chain.

## Features

- **Stores** — open, update, close; capacity staking; chain-verified reputation score
- **Products** — create, update, delist; unlimited or finite inventory; flash sale pricing with countdown; auto-deliver digital goods via BTCPC-FS CID
- **Orders** — escrow on-chain; buyer/seller cancel; dispute flow; 40-hour fulfillment deadline with automatic sweep
- **Shipping** — link carrier accounts (UPS, FedEx, USPS, DHL, PirateShip); masked account IDs; tracking on fulfillment
- **Privacy** — Tor hidden-service setup registers `.onion` address on-chain; catalog reads served by every BTCPC node — vendor IP never in the read path
- **Q&A** — per-product questions and seller answers; public read, auth-gated ask/answer
- **Reputation** — weighted vote; verified-buyer gate (must have a `delivered` order)
- **P2P catalog** — every BTCPC node mirrors the full commerce ledger at `GET /api/peer/commerce/*`, no auth, no single point of failure

## Stack

- Rust, Axum 0.7, Tokio
- `parking_lot::RwLock` in-memory state replayed from append-only `pending-entries.jsonl`
- Ledger format wire-compatible with the Node.js BTCPC chain
- JWT auth (BTCPC posting key) or X-Posting-Key header

## Quick start

```bash
# From the btcpc-market directory
cargo build --release

BTCPC_DATA_DIR=~/.btcpc \
BTCPC_API_PORT=7042 \
BTCPC_JWT_SECRET=your-secret \
./target/release/btcpc-market
```

## API

Base path: `POST|GET /api/commerce/`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/stores` | ✅ | Open a store |
| PATCH | `/stores/:seller` | ✅ | Update store |
| DELETE | `/stores/:seller` | ✅ | Close store |
| POST | `/stores/:seller/shipping` | ✅ | Link carrier account |
| DELETE | `/stores/:seller/shipping/:carrier` | ✅ | Unlink carrier |
| POST | `/stores/:seller/tor/setup` | ✅ | Enable Tor hidden service |
| DELETE | `/stores/:seller/tor` | ✅ | Disable Tor |
| GET | `/stores` | — | List stores |
| GET | `/stores/:seller` | — | Get store |
| POST | `/products` | ✅ | Create product |
| PATCH | `/products/*pid` | ✅ | Update product |
| DELETE | `/products/*pid` | ✅ | Delist product |
| GET | `/products` | — | List products |
| GET | `/products/*pid` | — | Get product |
| POST | `/orders` | ✅ | Place order |
| GET | `/orders/my` | ✅ | My orders |
| GET | `/orders/:oid` | ✅ | Get order |
| POST | `/orders/:oid/fulfill` | ✅ | Mark shipped |
| POST | `/orders/:oid/deliver` | ✅ | Confirm receipt |
| POST | `/orders/:oid/cancel` | ✅ | Cancel order |
| POST | `/orders/:oid/dispute` | ✅ | Raise dispute |
| POST | `/reputation/vote` | ✅ | Vote (verified buyer only) |
| POST | `/products/:seller/:slug/qa` | ✅ | Ask question |
| PATCH | `/products/:seller/:slug/qa/:id` | ✅ | Answer question |
| GET | `/products/:seller/:slug/qa` | — | List Q&A |
| GET | `/quote/capacity` | — | Capacity pricing quote |
| POST | `/import/amazon` | — | Import Amazon listings |
| GET | `/health` | — | Health check |

## Ledger entries

All mutations append a signed entry to `$BTCPC_DATA_DIR/pending-entries.jsonl` and apply it to in-memory state. Finalized entries are replayed from `$BTCPC_DATA_DIR/blocks/*.bin` on startup.

Entry types: `STORE_OPEN`, `STORE_UPDATE`, `STORE_CLOSE`, `STORE_SHIPPING_LINK`, `STORE_SHIPPING_UNLINK`, `PRODUCT_CREATE`, `PRODUCT_UPDATE`, `PRODUCT_DELIST`, `ORDER_PLACE`, `ORDER_FULFILL`, `ORDER_DELIVERED`, `ORDER_CANCEL`, `ORDER_DISPUTE`, `REPUTATION_VOTE`, `PRODUCT_QA_ASK`, `PRODUCT_QA_ANSWER`

## License

MIT — Shin Devlin / BTCPC Network
