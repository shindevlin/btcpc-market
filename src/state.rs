use crate::models::{Order, Product, Store};
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct MarketState {
    pub stores: HashMap<String, Store>,
    pub products: HashMap<String, Product>,
    pub orders: HashMap<String, Order>,
    pub reputation: HashMap<String, Vec<i64>>, // target_id → [weighted_sum, weight_total]
}

pub type SharedState = Arc<RwLock<MarketState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(MarketState::default()))
}

impl MarketState {
    pub fn apply_entry(&mut self, entry: &Value) {
        let entry_type = match entry.get("type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return,
        };
        match entry_type {
            "STORE_OPEN"       => self.apply_store_open(entry),
            "STORE_UPDATE"     => self.apply_store_update(entry),
            "STORE_CLOSE"      => self.apply_store_close(entry),
            "PRODUCT_CREATE"   => self.apply_product_create(entry),
            "PRODUCT_UPDATE"   => self.apply_product_update(entry),
            "PRODUCT_DELIST"   => self.apply_product_delist(entry),
            "ORDER_PLACE"      => self.apply_order_place(entry),
            "ORDER_FULFILL"    => self.apply_order_fulfill(entry),
            "ORDER_DELIVERED"  => self.apply_order_delivered(entry),
            "ORDER_CANCEL"     => self.apply_order_cancel(entry),
            "ORDER_DISPUTE"    => self.apply_order_dispute(entry),
            "REPUTATION_VOTE"  => self.apply_reputation_vote(entry),
            _ => {}
        }
    }

    fn apply_store_open(&mut self, e: &Value) {
        let seller = s(e, "from");
        let sd = &e["store_data"];
        self.stores.insert(seller.clone(), Store {
            seller: seller.clone(),
            name: sd["name"].as_str().unwrap_or(&seller).to_string(),
            banner_cid: sd["banner_cid"].as_str().map(str::to_string),
            description_cid: sd["description_cid"].as_str().map(str::to_string),
            categories: arr_strings(&sd["categories"]),
            capacity: sd["capacity"].as_u64().unwrap_or(0) as u32,
            used_capacity: 0,
            stake_amount: sd["stake_amount"].as_f64().unwrap_or(0.0),
            status: "active".to_string(),
            opened_at: e["timestamp"].as_u64().unwrap_or(0),
            score: 0.0,
        });
    }

    fn apply_store_update(&mut self, e: &Value) {
        let seller = s(e, "from");
        let sd = &e["store_data"];
        if let Some(store) = self.stores.get_mut(&seller) {
            if let Some(name) = sd["name"].as_str() { store.name = name.to_string(); }
            if sd["banner_cid"].is_string() {
                store.banner_cid = sd["banner_cid"].as_str().map(str::to_string);
            }
            if sd["description_cid"].is_string() {
                store.description_cid = sd["description_cid"].as_str().map(str::to_string);
            }
            if sd["categories"].is_array() {
                store.categories = arr_strings(&sd["categories"]);
            }
        }
    }

    fn apply_store_close(&mut self, e: &Value) {
        let seller = s(e, "from");
        if let Some(store) = self.stores.get_mut(&seller) {
            store.status = "closed".to_string();
        }
    }

    fn apply_product_create(&mut self, e: &Value) {
        let seller = s(e, "from");
        let pd = &e["product_data"];
        let product_id = pd["product_id"].as_str().unwrap_or("").to_string();
        if product_id.is_empty() { return; }
        if let Some(store) = self.stores.get_mut(&seller) {
            store.used_capacity = store.used_capacity.saturating_add(1);
        }
        self.products.insert(product_id.clone(), Product {
            product_id,
            seller,
            title: pd["title"].as_str().unwrap_or("").to_string(),
            description: pd["description"].as_str().map(str::to_string),
            price: pd["price"].as_f64().unwrap_or(0.0),
            token: pd["token"].as_str().unwrap_or("BTCPC").to_string(),
            image_cid: pd["image_cid"].as_str().map(str::to_string),
            inventory: pd["inventory"].as_u64().map(|n| n as u32),
            categories: arr_strings(&pd["categories"]),
            status: "active".to_string(),
            created_epoch: e["epoch"].as_u64().unwrap_or(0),
        });
    }

    fn apply_product_update(&mut self, e: &Value) {
        let pd = &e["product_data"];
        let product_id = pd["product_id"].as_str().unwrap_or("").to_string();
        if let Some(product) = self.products.get_mut(&product_id) {
            if let Some(title) = pd["title"].as_str() { product.title = title.to_string(); }
            if let Some(desc) = pd["description"].as_str() {
                product.description = Some(desc.to_string());
            }
            if let Some(price) = pd["price"].as_f64() { product.price = price; }
            if let Some(cid) = pd["image_cid"].as_str() {
                product.image_cid = Some(cid.to_string());
            }
            if let Some(inv) = pd["inventory"].as_i64() {
                product.inventory = if inv < 0 { None } else { Some(inv as u32) };
            }
        }
    }

    fn apply_product_delist(&mut self, e: &Value) {
        let pd = &e["product_data"];
        let product_id = pd["product_id"].as_str().unwrap_or("").to_string();
        // Read seller before mutating product (avoids double mutable borrow)
        let seller = self.products.get(&product_id).map(|p| p.seller.clone());
        if let Some(product) = self.products.get_mut(&product_id) {
            product.status = "delisted".to_string();
        }
        if let Some(seller) = seller {
            if let Some(store) = self.stores.get_mut(&seller) {
                store.used_capacity = store.used_capacity.saturating_sub(1);
            }
        }
    }

    fn apply_order_place(&mut self, e: &Value) {
        let od = &e["order_data"];
        let order_id = od["order_id"].as_str().unwrap_or("").to_string();
        if order_id.is_empty() { return; }
        let quantity = od["quantity"].as_u64().unwrap_or(1) as u32;
        if let Some(pid) = od["product_id"].as_str() {
            if let Some(product) = self.products.get_mut(pid) {
                if let Some(inv) = product.inventory {
                    product.inventory = Some(inv.saturating_sub(quantity));
                }
            }
        }
        self.orders.insert(order_id.clone(), Order {
            order_id,
            buyer: s(e, "from"),
            seller: s(e, "to"),
            product_id: od["product_id"].as_str().unwrap_or("").to_string(),
            quantity,
            unit_price: od["unit_price"].as_f64().unwrap_or(0.0),
            total: od["total"].as_f64().unwrap_or(0.0),
            token: od["token"].as_str().unwrap_or("BTCPC").to_string(),
            escrow_id: od["escrow_id"].as_str().map(str::to_string),
            status: "pending".to_string(),
            fulfillment_cid: None,
            placed_epoch: e["epoch"].as_u64().unwrap_or(0),
        });
    }

    fn apply_order_fulfill(&mut self, e: &Value) {
        let od = &e["order_data"];
        if let Some(oid) = od["order_id"].as_str() {
            if let Some(order) = self.orders.get_mut(oid) {
                order.status = "fulfilled".to_string();
                order.fulfillment_cid = od["fulfillment_cid"].as_str().map(str::to_string);
            }
        }
    }

    fn apply_order_delivered(&mut self, e: &Value) {
        let od = &e["order_data"];
        if let Some(oid) = od["order_id"].as_str() {
            if let Some(order) = self.orders.get_mut(oid) {
                order.status = "delivered".to_string();
            }
        }
    }

    fn apply_order_cancel(&mut self, e: &Value) {
        let od = &e["order_data"];
        if let Some(oid) = od["order_id"].as_str() {
            // Read product_id + quantity before mutably borrowing orders
            let restore = self.orders.get(oid)
                .map(|o| (o.product_id.clone(), o.quantity));
            if let Some(order) = self.orders.get_mut(oid) {
                order.status = "cancelled".to_string();
            }
            if let Some((pid, qty)) = restore {
                if let Some(product) = self.products.get_mut(&pid) {
                    if let Some(inv) = product.inventory {
                        product.inventory = Some(inv + qty);
                    }
                }
            }
        }
    }

    fn apply_order_dispute(&mut self, e: &Value) {
        let od = &e["order_data"];
        if let Some(oid) = od["order_id"].as_str() {
            if let Some(order) = self.orders.get_mut(oid) {
                order.status = "disputed".to_string();
            }
        }
    }

    fn apply_reputation_vote(&mut self, e: &Value) {
        let rd = &e["reputation_data"];
        let target_id = rd["target_id"].as_str().unwrap_or("").to_string();
        if target_id.is_empty() { return; }
        let target_type = rd["target_type"].as_str().unwrap_or("").to_string();
        let vote = rd["vote"].as_i64().unwrap_or(0);
        let weight = rd["weight"].as_i64().unwrap_or(1);

        // Update reputation map, capturing results before releasing borrow
        let (sum, count) = {
            let rep = self.reputation.entry(target_id.clone()).or_insert_with(|| vec![0, 0]);
            rep[0] += vote * weight;
            rep[1] += weight;
            (rep[0], rep[1])
        };

        // Now we can safely borrow stores
        if target_type == "store" && count > 0 {
            if let Some(store) = self.stores.get_mut(&target_id) {
                store.score = (sum as f64 / count as f64 * 5.0).clamp(0.0, 5.0);
            }
        }
    }
}

fn s(e: &Value, key: &str) -> String {
    e[key].as_str().unwrap_or("").to_string()
}

fn arr_strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}
