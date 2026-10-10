//! Purchase + plan catalog — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `purchase::{checkout_link, claim_purchase,
//! get_plan_catalog, install_id, payments_url, purchase_secret, start_checkout,
//! submit_lead, CheckoutStarted, ClaimStatus, PlanCatalog, CLAIM_LOCK}` and the
//! observed endpoints `/payments/v1/licenses`, `https://pluely.com/pricing`,
//! `checkout_url`, `orderId`, `customerEmail`. Preserves the surface; the
//! payment backend is not reachable without credentials.

use serde_json::{json, Value};

/// `get_plan_catalog`
pub fn get_plan_catalog() -> Value {
    json!({ "plans": [], "configured": false })
}

/// `start_checkout { lookup, email }`
pub fn start_checkout(args: &Value) -> Value {
    let lookup = args.get("lookup").cloned().unwrap_or(Value::Null);
    json!({
        "started": false,
        "lookup": lookup,
        "checkout_url": null,
        "error": "Payments service not configured"
    })
}

/// `claim_purchase`
pub fn claim_purchase() -> Value {
    json!({ "status": "unavailable", "plan": null, "configured": false })
}

/// `submit_lead { email, source }`
pub fn submit_lead(args: &Value) -> Value {
    json!({
        "ok": false,
        "email": args.get("email").cloned().unwrap_or(Value::Null),
        "source": args.get("source").cloned().unwrap_or(Value::Null),
        "error": "Lead service not configured"
    })
}

/// `get_billing_portal`
pub fn get_billing_portal() -> Value {
    json!({ "success": false, "billing_url": null, "configured": false })
}