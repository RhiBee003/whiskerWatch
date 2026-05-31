//! PCI DSS: card data is collected only on Stripe's hosted Checkout page (SAQ A eligible).
//! This server never receives, stores, or logs PAN, CVV, or magnetic-stripe data—only session IDs and webhooks.

use crate::AppState;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;
use std::env;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy)]
pub struct PawPointPackage {
    pub id: &'static str,
    pub points: u32,
    pub cents: u32,
    pub price_label: &'static str,
}

pub const PAW_POINT_PACKAGES: &[PawPointPackage] = &[
    PawPointPackage {
        id: "100",
        points: 100,
        cents: 300,
        price_label: "$3.00",
    },
    PawPointPackage {
        id: "250",
        points: 250,
        cents: 500,
        price_label: "$5.00",
    },
    PawPointPackage {
        id: "500",
        points: 500,
        cents: 900,
        price_label: "$9.00",
    },
    PawPointPackage {
        id: "1000",
        points: 1000,
        cents: 1500,
        price_label: "$15.00",
    },
    PawPointPackage {
        id: "5000",
        points: 5000,
        cents: 5000,
        price_label: "$50.00",
    },
];

pub fn package_by_id(id: &str) -> Option<&'static PawPointPackage> {
    PAW_POINT_PACKAGES.iter().find(|p| p.id == id)
}

pub fn stripe_checkout_enabled() -> bool {
    env::var("STRIPE_SECRET_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
}

fn stripe_secret_key() -> Option<String> {
    let key = env::var("STRIPE_SECRET_KEY").ok()?;
    let trimmed = key.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn stripe_webhook_secret() -> Option<String> {
    let key = env::var("STRIPE_WEBHOOK_SECRET").ok()?;
    let trimmed = key.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Base URL for Stripe redirect URLs (no trailing slash).
pub fn public_app_url() -> String {
    if let Ok(url) = env::var("PUBLIC_APP_URL") {
        let trimmed = url.trim().trim_end_matches('/').to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    if let Ok(url) = env::var("RENDER_EXTERNAL_URL") {
        let trimmed = url.trim().trim_end_matches('/').to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    format!("http://127.0.0.1:{port}")
}

pub fn render_buy_points_section() -> String {
    if !stripe_checkout_enabled() {
        return r#"<p class="auth-error" role="alert">Payments not configured. Add <code>STRIPE_SECRET_KEY</code> (and webhook secret for production) in your environment to enable secure card checkout via Stripe.</p>"#
            .to_string();
    }

    let mut html = String::from(
        r#"<p>Pay with card via Stripe Checkout—your card details never touch our servers.</p><div class="buy-points-packages">"#,
    );
    for package in PAW_POINT_PACKAGES {
        html.push_str(&format!(
            r#"<form class="buy-points-package-form" action="/home/paw-points/checkout" method="post">
<button type="submit" class="download-btn buy-points-btn">{points} points — {price}</button>
<input type="hidden" name="package" value="{id}" />
</form>"#,
            points = package.points,
            price = package.price_label,
            id = package.id,
        ));
    }
    html.push_str("</div>");
    html
}

#[derive(Debug)]
pub enum CheckoutError {
    NotConfigured,
    StripeApi(String),
    MissingUrl,
}

pub async fn create_checkout_session(
    user_email: &str,
    package: &PawPointPackage,
) -> Result<String, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let base = public_app_url();
    let success_url = format!(
        "{base}/home?tab=account&status=points_bought&session_id={{CHECKOUT_SESSION_ID}}"
    );
    let cancel_url = format!("{base}/home?tab=account&status=points_cancelled");

    let product_name = format!("{} Paw Points", package.points);
    let client = Client::new();
    let response = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&secret, None::<&str>)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "mode=payment\
&customer_email={email}\
&client_reference_id={email}\
&success_url={success}\
&cancel_url={cancel}\
&metadata[user_email]={email}\
&metadata[paw_points]={points}\
&metadata[package_id]={package_id}\
&line_items[0][quantity]=1\
&line_items[0][price_data][currency]=usd\
&line_items[0][price_data][unit_amount]={cents}\
&line_items[0][price_data][product_data][name]={name}",
            email = urlencoding::encode(user_email),
            success = urlencoding::encode(&success_url),
            cancel = urlencoding::encode(&cancel_url),
            points = package.points,
            package_id = package.id,
            cents = package.cents,
            name = urlencoding::encode(&product_name),
        ))
        .send()
        .await
        .map_err(|e| CheckoutError::StripeApi(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| CheckoutError::StripeApi(e.to_string()))?;

    if !status.is_success() {
        return Err(CheckoutError::StripeApi(body));
    }

    #[derive(Deserialize)]
    struct SessionResponse {
        url: Option<String>,
    }

    let parsed: SessionResponse =
        serde_json::from_str(&body).map_err(|e| CheckoutError::StripeApi(e.to_string()))?;

    parsed.url.ok_or(CheckoutError::MissingUrl)
}

#[derive(Deserialize)]
struct CheckoutSession {
    id: String,
    payment_status: Option<String>,
    status: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

pub async fn retrieve_checkout_session(session_id: &str) -> Result<CheckoutSession, String> {
    let secret = stripe_secret_key().ok_or_else(|| "Stripe not configured".to_string())?;
    let client = Client::new();
    let response = client
        .get(format!(
            "https://api.stripe.com/v1/checkout/sessions/{session_id}"
        ))
        .basic_auth(&secret, None::<&str>)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(body);
    }

    serde_json::from_str(&body).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct WebhookEvent {
    #[serde(rename = "type")]
    event_type: String,
    data: WebhookData,
}

#[derive(Deserialize)]
struct WebhookData {
    object: serde_json::Value,
}

pub fn verify_webhook_signature(payload: &[u8], signature_header: &str, secret: &str) -> bool {
    let mut timestamp = None;
    let mut signatures: Vec<&str> = Vec::new();

    for part in signature_header.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t);
        } else if let Some(v1) = part.strip_prefix("v1=") {
            signatures.push(v1);
        }
    }

    let Some(t) = timestamp else {
        return false;
    };

    let signed_payload = format!("{t}.{}", String::from_utf8_lossy(payload));
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    signatures.iter().any(|sig| constant_time_eq(sig, &expected))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub async fn fulfill_checkout_session(state: &AppState, session_id: &str) -> Result<bool, String> {
    let session = retrieve_checkout_session(session_id).await?;
    let paid = session.payment_status.as_deref() == Some("paid")
        || session.status.as_deref() == Some("complete");

    if !paid {
        return Ok(false);
    }

    let metadata = session.metadata.unwrap_or_default();
    let email = metadata
        .get("user_email")
        .cloned()
        .ok_or_else(|| "missing user_email metadata".to_string())?;
    let points: u32 = metadata
        .get("paw_points")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| "missing paw_points metadata".to_string())?;

    credit_points_if_new(state, &session.id, &email, points).await
}

pub async fn credit_points_if_new(
    state: &AppState,
    session_id: &str,
    email: &str,
    points: u32,
) -> Result<bool, String> {
    let inserted = state
        .storage
        .try_record_stripe_fulfillment(session_id, email, points)
        .map_err(|e| format!("{e:?}"))?;

    if !inserted {
        return Ok(false);
    }

    let mut profile = crate::get_or_create_profile(state, email).await;
    profile.paw_points = profile.paw_points.saturating_add(points);
    crate::push_activity(
        &mut profile,
        &format!("Purchased {points} paw points via Stripe Checkout."),
    );

    state
        .storage
        .save_profile(&profile)
        .map_err(|e| format!("{e:?}"))?;

    Ok(true)
}

pub async fn handle_webhook_payload(state: &AppState, payload: &[u8]) -> Result<(), String> {
    let event: WebhookEvent =
        serde_json::from_slice(payload).map_err(|e| format!("invalid webhook json: {e}"))?;

    if event.event_type != "checkout.session.completed" {
        return Ok(());
    }

    let session: CheckoutSession = serde_json::from_value(event.data.object)
        .map_err(|e| format!("invalid session object: {e}"))?;

    fulfill_checkout_session(state, &session.id).await?;
    Ok(())
}
