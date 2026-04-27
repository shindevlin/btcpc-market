mod app;
mod auth;
mod bonding_curve;
mod config;
mod ledger;
mod models;
mod routes;
mod state;

use anyhow::Result;
use axum::{
    middleware,
    routing::{get, post, patch},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use app::AppState;
use config::Config;
use state::new_shared_state;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("btcpc_market=info".parse()?)
                .add_directive("tower_http=warn".parse()?)
        )
        .init();

    let cfg = Config::from_env();
    let state = new_shared_state();

    let block_count  = ledger::load_block_files(&cfg, &state);
    let pending_count = ledger::load_pending_entries(&cfg, &state);
    info!("BTCPC Market v{} — replayed {} block + {} pending entries",
        env!("CARGO_PKG_VERSION"), block_count, pending_count);

    let port = cfg.port;
    let app_state = AppState::new(cfg, state);

    // ── Protected routes — require JWT or posting key ──────────────────────
    let auth_mw = middleware::from_fn_with_state(
        app_state.clone(),
        auth::require_auth,
    );

    let protected = Router::new()
        // Stores (mutations)
        .route("/stores",          post(routes::stores::open_store))
        .route("/stores/:seller",  patch(routes::stores::update_store)
                                   .delete(routes::stores::close_store))
        // Products (mutations)
        .route("/products",           post(routes::products::create_product))
        .route("/products/*pid",      patch(routes::products::update_product)
                                      .delete(routes::products::delist_product))
        // Orders (all authenticated — order data is private)
        .route("/orders",                    post(routes::orders::place_order))
        .route("/orders/my",                 get(routes::orders::my_orders))
        .route("/orders/:oid",               get(routes::orders::get_order))
        .route("/orders/:oid/fulfill",       post(routes::orders::fulfill_order))
        .route("/orders/:oid/deliver",       post(routes::orders::deliver_order))
        .route("/orders/:oid/cancel",        post(routes::orders::cancel_order))
        .route("/orders/:oid/dispute",       post(routes::orders::dispute_order))
        // Reputation
        .route("/reputation/vote",    post(routes::reputation::vote))
        .route_layer(auth_mw);

    // ── Public routes — no auth required ──────────────────────────────────
    let public = Router::new()
        .route("/stores",          get(routes::stores::list_stores))
        .route("/stores/:seller",  get(routes::stores::get_store))
        .route("/products",        get(routes::products::list_products))
        .route("/products/*pid",   get(routes::products::get_product))
        .route("/quote/capacity",  get(routes::stores::quote_capacity))
        .route("/import/amazon",   post(routes::import::import_amazon));

    let commerce = protected.merge(public);

    let app = Router::new()
        .nest("/api/commerce", commerce)
        .route("/health", get(health))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr = format!("0.0.0.0:{port}");
    info!("BTCPC Market listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "ok": true,
        "service": "btcpc-market",
        "version": env!("CARGO_PKG_VERSION"),
        "epoch": models::current_epoch(),
    }))
}
