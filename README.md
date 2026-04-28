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

## Key management

`btcpc-market` currently operates exclusively with the **posting key**. The posting key is a 64-character hex Ed25519 private key that signs all non-financial ledger entries: store mutations, product listings, order actions, Q&A, and reputation votes.

Two additional keys are defined in the protocol but not yet enforced by this service:

| Key | Purpose | Status |
|-----|---------|--------|
| **Posting key** | Signs catalog and order-status entries | Implemented (Phase G) |
| **Active key** | Signs token transfers — ESCROW_LOCK on ORDER_PLACE, ESCROW_RELEASE on ORDER_DELIVER | Roadmap (Phase H) |
| **Memo key** | Encrypts/signs buyer-to-seller and seller-to-buyer reputation memos (REPUTATION_MEMO entries) | Roadmap (Phase H) |

Keys never leave the client. `btcpc-market` verifies the posting key signature on every mutating request and rejects entries with invalid signatures before they touch the ledger.

See [Appendix N of the BTCPC whitepaper](https://github.com/shindevlin/btcpc/blob/main/docs/BTCPC_WHITEPAPER.md#appendix-n--key-architecture) for the full key architecture, escrow flow, and buyer staking design.

## Roadmap

The full roadmap lives at [`docs/ROADMAP.md`](https://github.com/shindevlin/btcpc/blob/main/docs/ROADMAP.md) in the main BTCPC repo. Commerce-relevant phases:

| Phase | Description |
|-------|-------------|
| **H — Auth & Wallet Integration** | Active key escrow debit/release; memo key reputation memos; buyer staking pool; multi-sig dispute escrow |
| **I — Discovery & Search** | Full-text product search across all sellers; category browsing; featured stores; real ledger analytics; seller verification badge |
| **J — Payments & Tokens** | wBTCPC bridge to Ethereum ERC-20; multi-token checkout via bonding curve; discount codes; subscription products; affiliate/referral on-chain entries |
| **K — Infrastructure & Scale** | Docker Compose single-validator deployment; Kubernetes gateway tier; BTCPC-FS CDN; Telegram order notifications; mobile order tracking in Android app |
| **L — Governance & Compliance** | On-chain dispute arbitration with staked arbiters; BTCPC Verified Seller program; privacy mode with memo-key-encrypted order data |

## License

MIT — Shin Devlin / BTCPC Network
