use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::{
    app::AppState,
    auth::AuthUser,
    bonding_curve::{capacity_for_payment, cost_for_capacity, stake_for_capacity},
    ledger,
    models::{current_epoch, LedgerEntry, OpenStoreRequest, PaginationQuery, UpdateStoreRequest, CapacityQuoteRequest},
};

/// POST /api/commerce/stores — open a storefront
pub async fn open_store(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(body): Json<OpenStoreRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let seller = user.username;
    {
        let state = app.state.read();
        if let Some(store) = state.stores.get(&seller) {
            if store.status == "active" {
                return Err((StatusCode::CONFLICT, Json(json!({"error": "store already open"}))));
            }
        }
    }

    let name = body.name
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| seller.clone());

    let capacity = body.initial_capacity.unwrap_or(10).max(1).min(10_000);
    let stake_paid_usd = body.stake_paid_usd.unwrap_or(0.0).max(0.0);
    let stake_amount = stake_for_capacity(capacity);
    let epoch = current_epoch();

    let mut entry = LedgerEntry::new("STORE_OPEN", &seller, epoch);
    entry.store_data = Some(json!({
        "action": "open",
        "name": name,
        "banner_cid": body.banner_cid,
        "description_cid": body.description_cid,
        "categories": body.categories.unwrap_or_default(),
        "capacity": capacity,
        "stake_amount": stake_amount,
        "stake_paid_usd": stake_paid_usd,
    }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({
        "ok": true,
        "seller": seller,
        "name": name,
        "capacity": capacity,
        "stake_amount": stake_amount,
        "epoch": epoch,
    })))
}

/// GET /api/commerce/stores — list all active stores
pub async fn list_stores(
    State(app): State<AppState>,
    Query(q): Query<PaginationQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0);
    let state = app.state.read();
    let mut stores: Vec<&crate::models::Store> = state.stores.values()
        .filter(|s| s.status == "active")
        .collect();
    stores.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));
    let total = stores.len();
    let page: Vec<Value> = stores.into_iter().skip(offset).take(limit)
        .map(|s| serde_json::to_value(s).unwrap_or(json!({})))
        .collect();
    Json(json!({ "stores": page, "total": total, "limit": limit, "offset": offset }))
}

/// GET /api/commerce/stores/:seller — get a single store
pub async fn get_store(
    State(app): State<AppState>,
    Path(seller): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let state = app.state.read();
    state.stores.get(&seller)
        .map(|s| Json(serde_json::to_value(s).unwrap_or(json!({}))))
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "store not found"}))))
}

/// PATCH /api/commerce/stores/:seller — update store metadata
pub async fn update_store(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(seller): Path<String>,
    Json(body): Json<UpdateStoreRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if user.username != seller {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "not your store"}))));
    }
    {
        let state = app.state.read();
        match state.stores.get(&seller) {
            Some(s) if s.status == "active" => {}
            _ => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "store not found or closed"})))),
        }
    }
    let epoch = current_epoch();
    let mut entry = LedgerEntry::new("STORE_UPDATE", &seller, epoch);
    entry.store_data = Some(json!({
        "action": "update",
        "name": body.name,
        "banner_cid": body.banner_cid,
        "description_cid": body.description_cid,
        "categories": body.categories,
    }));
    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    Ok(Json(json!({ "ok": true })))
}

/// DELETE /api/commerce/stores/:seller — close store
pub async fn close_store(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(seller): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if user.username != seller {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "not your store"}))));
    }
    let epoch = current_epoch();
    let mut entry = LedgerEntry::new("STORE_CLOSE", &seller, epoch);
    entry.store_data = Some(json!({ "action": "close" }));
    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    Ok(Json(json!({ "ok": true, "seller": seller })))
}

/// GET /api/commerce/quote/capacity — bonding curve price quote
pub async fn quote_capacity(
    Query(q): Query<CapacityQuoteRequest>,
) -> Json<Value> {
    let current = q.current_capacity.unwrap_or(0);
    let units = q.units.max(1).min(100_000);
    let cost_usd = cost_for_capacity(current, units);
    let slots_for_1 = capacity_for_payment(current, 1.0);
    let stake_needed = stake_for_capacity(units);
    Json(json!({
        "current_capacity": current,
        "units": units,
        "cost_usd": cost_usd,
        "slots_for_1_usd": slots_for_1,
        "stake_btcpc_required": stake_needed,
    }))
}
