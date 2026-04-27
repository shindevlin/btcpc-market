use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::{
    app::AppState,
    auth::AuthUser,
    ledger,
    models::{current_epoch, LedgerEntry, ReputationVoteRequest},
};

/// POST /api/commerce/reputation/vote
pub async fn vote(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(body): Json<ReputationVoteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let voter = user.username;

    if !matches!(body.target_type.as_str(), "store" | "miner" | "product") {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "target_type must be store, miner, or product"}))));
    }
    if !matches!(body.vote, 1 | -1) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "vote must be +1 or -1"}))));
    }
    if body.target_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "target_id required"}))));
    }

    // Verified purchase required for product reviews
    if body.target_type == "product" {
        let state = app.state.read();
        let has_purchased = state.orders.values().any(|o| {
            o.buyer == voter && o.product_id == body.target_id && o.status == "delivered"
        });
        if !has_purchased {
            return Err((StatusCode::FORBIDDEN, Json(json!({"error": "verified purchase required to review this product"}))));
        }
    }

    let weight = body.weight.unwrap_or(1).max(1).min(100);
    let epoch = current_epoch();

    let mut entry = LedgerEntry::new("REPUTATION_VOTE", &voter, epoch);
    entry.reputation_data = Some(json!({
        "voter": voter,
        "target_type": body.target_type,
        "target_id": body.target_id,
        "vote": body.vote,
        "weight": weight,
        "memo": body.memo,
    }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({
        "ok": true,
        "voter": voter,
        "target_type": body.target_type,
        "target_id": body.target_id,
        "vote": body.vote,
        "weight": weight,
        "epoch": epoch,
    })))
}
