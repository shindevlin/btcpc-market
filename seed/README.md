# Demo seed data

Appends a pre-populated demo store (`demovendor`) to the pending-entries ledger so visitors can see a complete store without needing to create one.

## Usage

```bash
cat seed/demo-entries.jsonl >> ~/.btcpc/pending-entries.jsonl
```

Then restart `btcpc-market`. The `demovendor` store and its products will appear immediately.

## Demo login credentials

| Field | Value |
|-------|-------|
| Username | `demovendor` |
| Posting key | `0000000000000000000000000000000000000000000000000000000000000001` |

Use these in the vendor dashboard login modal to explore the full seller experience.

**Note:** The posting key is format-validated only in local/dev mode. In production, configure `BTCPC_JWT_SECRET` to something private — the demo key will still let you log in since key validation is local-sidecar trust.
