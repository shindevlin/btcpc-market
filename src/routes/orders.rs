use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::AuthUser,
    ledger,
    models::{
        current_epoch, DisputeOrderRequest, FulfillOrderRequest, LedgerEntry, PlaceOrderRequest,
    },
};

fn gen_order_id() -> String {
    let id = Uuid::new_v4().to_string().replace('-', "");
    format!("ord-{}", &id[..24])
}

fn gen_escrow_id() -> String {
    let id = Uuid::new_v4().to_string().replace('-', "");
    format!("esc-{}", &id[..16])
}

/// POST /api/commerce/orders — place an order
pub async fn place_order(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(body): Json<PlaceOrderRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let buyer = user.username;
    let quantity = body.quantity.unwrap_or(1).max(1);

    let (seller, unit_price, token, auto_deliver, delivery_cid) = {
        let state = app.state.read();
        let product = state.products.get(&body.product_id)
            .filter(|p| p.status == "active")
            .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "product not found or unlisted"}))))?;

        if let Some(inv) = product.inventory {
            if inv < quantity {
                return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(json!({
                    "error": "insufficient inventory",
                    "available": inv,
                }))));
            }
        }

        // Flash sale: use sale_price if active
        let effective_price = match (product.sale_price, product.sale_ends_epoch) {
            (Some(sp), Some(se)) if se > current_epoch() => sp,
            _ => product.price,
        };

        (
            product.seller.clone(),
            effective_price,
            body.token.clone().unwrap_or_else(|| product.token.clone()),
            product.auto_deliver,
            product.delivery_cid.clone(),
        )
    };

    if buyer == seller {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "cannot buy your own product"}))));
    }

    let total = (unit_price * quantity as f64 * 1e10).round() / 1e10;
    let order_id = gen_order_id();
    let escrow_id = gen_escrow_id();
    let epoch = current_epoch();

    let mut entry = LedgerEntry::new("ORDER_PLACE", &buyer, epoch);
    entry.to = Some(seller.clone());
    entry.token = token.clone();
    entry.amount = total;
    entry.order_data = Some(json!({
        "order_id": order_id,
        "product_id": body.product_id,
        "quantity": quantity,
        "unit_price": unit_price,
        "total": total,
        "token": token,
        "escrow_id": escrow_id,
        "shipping_address": body.shipping_address,
    }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // Auto-deliver digital goods
    let mut auto_delivered = false;
    if auto_deliver {
        if let Some(cid) = delivery_cid {
            let mut fulfill_entry = LedgerEntry::new("ORDER_FULFILL", &seller, current_epoch());
            fulfill_entry.order_data = Some(json!({
                "order_id": order_id,
                "fulfillment_cid": cid,
                "auto_delivered": true,
            }));
            ledger::persist(&app.cfg, &app.state, &fulfill_entry).ok();
            auto_delivered = true;
        }
    }

    Ok(Json(json!({
        "ok": true,
        "order_id": order_id,
        "buyer": buyer,
        "seller": seller,
        "product_id": body.product_id,
        "quantity": quantity,
        "total": total,
        "token": token,
        "escrow_id": escrow_id,
        "status": if auto_delivered { "fulfilled" } else { "pending" },
        "auto_delivered": auto_delivered,
        "epoch": epoch,
    })))
}

/// GET /api/commerce/orders/my — list caller's orders (as buyer or seller)
pub async fn my_orders(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<Value> {
    let username = user.username;
    let state = app.state.read();
    let orders: Vec<Value> = state.orders.values()
        .filter(|o| o.buyer == username || o.seller == username)
        .map(|o| serde_json::to_value(o).unwrap_or(json!({})))
        .collect();
    let total = orders.len();
    Json(json!({ "orders": orders, "total": total }))
}

/// GET /api/commerce/orders/:orderId — get a single order (buyer or seller only)
pub async fn get_order(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let username = user.username;
    let state = app.state.read();
    let order = state.orders.get(&order_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "order not found"}))))?;
    if order.buyer != username && order.seller != username {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "not your order"}))));
    }
    Ok(Json(serde_json::to_value(order).unwrap_or(json!({}))))
}

/// POST /api/commerce/orders/:orderId/fulfill — seller marks shipped
pub async fn fulfill_order(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(order_id): Path<String>,
    Json(body): Json<FulfillOrderRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let seller = user.username;
    {
        let state = app.state.read();
        let order = state.orders.get(&order_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "order not found"}))))?;
        if order.seller != seller {
            return Err((StatusCode::FORBIDDEN, Json(json!({"error": "not your order to fulfill"}))));
        }
        if order.status != "pending" {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "order not in pending state"}))));
        }
    }

    let epoch = current_epoch();
    let mut entry = LedgerEntry::new("ORDER_FULFILL", &seller, epoch);
    entry.order_data = Some(json!({
        "order_id": order_id,
        "fulfillment_cid": body.fulfillment_cid,
        "carrier": body.carrier,
        "tracking_number": body.tracking_number,
        "shipping_service": body.shipping_service,
        "shipping_note": body.shipping_note,
    }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    Ok(Json(json!({ "ok": true, "order_id": order_id, "status": "fulfilled" })))
}

/// POST /api/commerce/orders/:orderId/deliver — buyer confirms receipt
pub async fn deliver_order(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let buyer = user.username;
    {
        let state = app.state.read();
        let order = state.orders.get(&order_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "order not found"}))))?;
        if order.buyer != buyer {
            return Err((StatusCode::FORBIDDEN, Json(json!({"error": "not your order"}))));
        }
        if order.status != "fulfilled" {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "order not fulfilled yet"}))));
        }
    }

    let epoch = current_epoch();
    let mut entry = LedgerEntry::new("ORDER_DELIVERED", &buyer, epoch);
    entry.order_data = Some(json!({ "order_id": order_id }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    Ok(Json(json!({ "ok": true, "order_id": order_id, "status": "delivered" })))
}

/// POST /api/commerce/orders/:orderId/cancel — buyer or seller cancels
pub async fn cancel_order(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let party = user.username;
    {
        let state = app.state.read();
        let order = state.orders.get(&order_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "order not found"}))))?;
        if order.buyer != party && order.seller != party {
            return Err((StatusCode::FORBIDDEN, Json(json!({"error": "not your order"}))));
        }
        if !matches!(order.status.as_str(), "pending" | "fulfilled") {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "order cannot be cancelled in current state"}))));
        }
    }

    let epoch = current_epoch();
    let mut entry = LedgerEntry::new("ORDER_CANCEL", &party, epoch);
    entry.order_data = Some(json!({ "order_id": order_id }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    Ok(Json(json!({ "ok": true, "order_id": order_id, "status": "cancelled" })))
}

/// POST /api/commerce/orders/:orderId/dispute — buyer raises a dispute
pub async fn dispute_order(
    State(app): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(order_id): Path<String>,
    Json(body): Json<DisputeOrderRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let buyer = user.username;
    {
        let state = app.state.read();
        let order = state.orders.get(&order_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "order not found"}))))?;
        if order.buyer != buyer {
            return Err((StatusCode::FORBIDDEN, Json(json!({"error": "only the buyer can dispute"}))));
        }
        if !matches!(order.status.as_str(), "pending" | "fulfilled") {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "order cannot be disputed in current state"}))));
        }
    }

    let epoch = current_epoch();
    let mut entry = LedgerEntry::new("ORDER_DISPUTE", &buyer, epoch);
    entry.order_data = Some(json!({
        "order_id": order_id,
        "memo": body.memo,
    }));

    ledger::persist(&app.cfg, &app.state, &entry)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    Ok(Json(json!({ "ok": true, "order_id": order_id, "status": "disputed" })))
}
