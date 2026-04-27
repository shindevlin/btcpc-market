use serde::{Deserialize, Serialize};

// ── Ledger entry (wire format matches Node.js ledger) ────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LedgerEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub token: String,
    pub amount: f64,
    pub epoch: u64,
    pub signature: Option<String>,
    pub signed_by: Option<String>,
    pub memo: Option<String>,
    pub timestamp: u64,
    // Commerce-specific payloads (None for non-commerce entries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation_data: Option<serde_json::Value>,
}

impl LedgerEntry {
    pub fn new(entry_type: &str, from: &str, epoch: u64) -> Self {
        LedgerEntry {
            entry_type: entry_type.to_string(),
            from: Some(from.to_string()),
            to: None,
            token: "BTCPC".to_string(),
            amount: 0.0,
            epoch,
            signature: None,
            signed_by: None,
            memo: None,
            timestamp: now_ms(),
            store_data: None,
            product_data: None,
            order_data: None,
            reputation_data: None,
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// Genesis timestamp: 2026-04-15T07:00:00.000Z
const GENESIS_MS: u64 = 1776236400000;
const EPOCH_DURATION_MS: u64 = 30_000;

pub fn current_epoch() -> u64 {
    let now = now_ms();
    if now < GENESIS_MS {
        return 0;
    }
    (now - GENESIS_MS) / EPOCH_DURATION_MS
}

// ── In-memory state snapshots ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Store {
    pub seller: String,
    pub name: String,
    pub banner_cid: Option<String>,
    pub description_cid: Option<String>,
    pub categories: Vec<String>,
    pub capacity: u32,
    pub used_capacity: u32,
    pub stake_amount: f64,
    pub status: String, // "active" | "closed"
    pub opened_at: u64,
    pub score: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Product {
    pub product_id: String,
    pub seller: String,
    pub title: String,
    pub description: Option<String>,
    pub price: f64,
    pub token: String,
    pub image_cid: Option<String>,
    pub inventory: Option<u32>,
    pub categories: Vec<String>,
    pub status: String, // "active" | "delisted"
    pub created_epoch: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Order {
    pub order_id: String,
    pub buyer: String,
    pub seller: String,
    pub product_id: String,
    pub quantity: u32,
    pub unit_price: f64,
    pub total: f64,
    pub token: String,
    pub escrow_id: Option<String>,
    pub status: String, // "pending" | "fulfilled" | "delivered" | "cancelled" | "disputed"
    pub fulfillment_cid: Option<String>,
    pub placed_epoch: u64,
}

// ── Request bodies ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OpenStoreRequest {
    pub name: Option<String>,
    pub banner_cid: Option<String>,
    pub description_cid: Option<String>,
    pub categories: Option<Vec<String>>,
    pub initial_capacity: Option<u32>,
    pub stake_paid_usd: Option<f64>,
}

#[derive(Deserialize)]
pub struct UpdateStoreRequest {
    pub name: Option<String>,
    pub banner_cid: Option<String>,
    pub description_cid: Option<String>,
    pub categories: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct CreateProductRequest {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub price: f64,
    pub token: Option<String>,
    pub image_cid: Option<String>,
    pub inventory: Option<u32>,
    pub categories: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateProductRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub image_cid: Option<String>,
    pub inventory: Option<i64>,
}

#[derive(Deserialize)]
pub struct PlaceOrderRequest {
    pub product_id: String,
    pub quantity: Option<u32>,
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct FulfillOrderRequest {
    pub fulfillment_cid: Option<String>,
}

#[derive(Deserialize)]
pub struct DisputeOrderRequest {
    pub memo: Option<String>,
}

#[derive(Deserialize)]
pub struct ReputationVoteRequest {
    pub target_type: String,
    pub target_id: String,
    pub vote: i8,
    pub weight: Option<u8>,
    pub memo: Option<String>,
}

#[derive(Deserialize)]
pub struct CapacityQuoteRequest {
    pub current_capacity: Option<u32>,
    pub units: u32,
}

#[derive(Deserialize)]
pub struct ImportUrlRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub seller: Option<String>,
}

