//! PCI DSS: card data is collected only on Stripe's hosted Checkout page (SAQ A eligible).
//! This server never receives, stores, or logs PAN, CVV, or magnetic-stripe data—only
//! Stripe customer ids (`cus_...`), session ids, and safe card metadata (brand, last4, exp)
//! fetched from the Stripe API for display.

use crate::AppState;
use crate::UserProfile;
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
        r#"<p>Pay with card via Stripe Checkout—your card details never touch our servers. You can save your card for faster checkout next time.</p><div class="buy-points-packages">"#,
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

/// Account tab: saved cards from Stripe (brand, last4, exp only).
pub async fn render_saved_payment_methods(_state: &AppState, profile: &UserProfile) -> String {
    if !stripe_checkout_enabled() {
        return r#"<p class="muted">Saved cards appear here when Stripe payments are configured.</p>"#
            .to_string();
    }

    let customer_id = match profile.stripe_customer_id.as_deref() {
        Some(id) if id.starts_with("cus_") => id.to_string(),
        _ => {
            return r#"<p class="muted">No saved cards yet. Complete a paw points purchase and choose to save your card at checkout.</p>"#
                .to_string();
        }
    };

    match list_card_payment_methods(&customer_id).await {
        Ok(methods) if methods.is_empty() => {
            r#"<p class="muted">No saved cards yet. On your next purchase, Stripe Checkout will offer to save your card for faster checkout.</p>"#
                .to_string()
        }
        Ok(methods) => render_payment_method_list(&methods),
        Err(_) => {
            r#"<p class="auth-error" role="alert">Could not load saved cards. Try again later.</p>"#
                .to_string()
        }
    }
}

fn render_payment_method_list(methods: &[SavedCardDisplay]) -> String {
    let mut html =
        String::from(r#"<ul class="saved-payment-methods" aria-label="Saved payment methods">"#);
    for card in methods {
        html.push_str(&format!(
            r#"<li><span class="saved-card-brand">{brand}</span> <span class="saved-card-last4">•••• {last4}</span> <span class="saved-card-exp">Exp {exp}</span></li>"#,
            brand = crate::escape_html(&card.brand_label),
            last4 = crate::escape_html(&card.last4),
            exp = crate::escape_html(&card.exp),
        ));
    }
    html.push_str("</ul>");
    html.push_str(
        r#"<p class="muted saved-cards-note">Cards are stored securely by Stripe. WhiskerWatch never sees your full card number or CVV.</p>"#,
    );
    html
}

struct SavedCardDisplay {
    brand_label: String,
    last4: String,
    exp: String,
}

#[derive(Debug)]
pub enum CheckoutError {
    NotConfigured,
    StripeApi(String),
    MissingUrl,
}

/// Link user to a Stripe Customer (stored as `cus_...` on profile only).
pub async fn ensure_stripe_customer(
    state: &AppState,
    profile: &mut UserProfile,
) -> Result<String, CheckoutError> {
    if let Some(id) = profile.stripe_customer_id.as_ref() {
        let trimmed = id.trim();
        if trimmed.starts_with("cus_") {
            return Ok(trimmed.to_string());
        }
    }

    let email = profile.email.trim();
    let customer_id = if let Some(existing) = find_customer_by_email(email).await? {
        existing
    } else {
        create_stripe_customer(email).await?
    };

    profile.stripe_customer_id = Some(customer_id.clone());
    state
        .storage
        .save_profile(profile)
        .map_err(|e| CheckoutError::StripeApi(format!("{e:?}")))?;

    Ok(customer_id)
}

async fn find_customer_by_email(email: &str) -> Result<Option<String>, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let client = Client::new();
    let response = client
        .get("https://api.stripe.com/v1/customers")
        .basic_auth(&secret, None::<&str>)
        .query(&[("email", email), ("limit", "1")])
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
    struct CustomerList {
        data: Vec<CustomerRow>,
    }

    #[derive(Deserialize)]
    struct CustomerRow {
        id: String,
    }

    let parsed: CustomerList =
        serde_json::from_str(&body).map_err(|e| CheckoutError::StripeApi(e.to_string()))?;

    Ok(parsed.data.into_iter().next().map(|c| c.id))
}

async fn create_stripe_customer(email: &str) -> Result<String, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let client = Client::new();
    let response = client
        .post("https://api.stripe.com/v1/customers")
        .basic_auth(&secret, None::<&str>)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "email={email}&metadata[user_email]={email}",
            email = urlencoding::encode(email),
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
    struct CustomerResponse {
        id: String,
    }

    let parsed: CustomerResponse =
        serde_json::from_str(&body).map_err(|e| CheckoutError::StripeApi(e.to_string()))?;

    Ok(parsed.id)
}

async fn list_card_payment_methods(
    customer_id: &str,
) -> Result<Vec<SavedCardDisplay>, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let client = Client::new();
    let response = client
        .get("https://api.stripe.com/v1/payment_methods")
        .basic_auth(&secret, None::<&str>)
        .query(&[("customer", customer_id), ("type", "card")])
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
    struct PaymentMethodList {
        data: Vec<PaymentMethodRow>,
    }

    #[derive(Deserialize)]
    struct PaymentMethodRow {
        card: Option<CardDetails>,
    }

    #[derive(Deserialize)]
    struct CardDetails {
        brand: Option<String>,
        last4: Option<String>,
        exp_month: Option<u32>,
        exp_year: Option<u32>,
    }

    let parsed: PaymentMethodList =
        serde_json::from_str(&body).map_err(|e| CheckoutError::StripeApi(e.to_string()))?;

    let mut cards = Vec::new();
    for pm in parsed.data {
        let Some(card) = pm.card else {
            continue;
        };
        let brand = card.brand.unwrap_or_else(|| "card".to_string());
        let last4 = card.last4.unwrap_or_else(|| "????".to_string());
        let exp_month = card.exp_month.unwrap_or(0);
        let exp_year = card.exp_year.unwrap_or(0);
        let exp = if exp_month > 0 && exp_year > 0 {
            let yy = exp_year % 100;
            format!("{exp_month:02}/{yy:02}")
        } else {
            "—".to_string()
        };
        cards.push(SavedCardDisplay {
            brand_label: format_card_brand(&brand),
            last4,
            exp,
        });
    }

    Ok(cards)
}

fn format_card_brand(brand: &str) -> String {
    match brand.to_lowercase().as_str() {
        "visa" => "Visa".to_string(),
        "mastercard" => "Mastercard".to_string(),
        "amex" => "American Express".to_string(),
        "discover" => "Discover".to_string(),
        "diners" => "Diners Club".to_string(),
        "jcb" => "JCB".to_string(),
        "unionpay" => "UnionPay".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                None => "Card".to_string(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

pub async fn create_checkout_session(
    state: &AppState,
    user_email: &str,
    package: &PawPointPackage,
) -> Result<String, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let mut profile = crate::get_or_create_profile(state, user_email).await;
    let customer_id = ensure_stripe_customer(state, &mut profile).await?;

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
&customer={customer}\
&client_reference_id={email}\
&success_url={success}\
&cancel_url={cancel}\
&metadata[user_email]={email}\
&metadata[paw_points]={points}\
&metadata[package_id]={package_id}\
&payment_intent_data[setup_future_usage]=off_session\
&saved_payment_method_options[payment_method_save]=enabled\
&line_items[0][quantity]=1\
&line_items[0][price_data][currency]=usd\
&line_items[0][price_data][unit_amount]={cents}\
&line_items[0][price_data][product_data][name]={name}",
            customer = urlencoding::encode(&customer_id),
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

    if metadata.get("product_type").map(String::as_str) == Some("breed_guide") {
        let breed_slug = metadata
            .get("breed_slug")
            .cloned()
            .ok_or_else(|| "missing breed_slug metadata".to_string())?;
        return unlock_breed_guide_if_new(state, &session.id, &email, &breed_slug).await;
    }

    if metadata.get("product_type").map(String::as_str) == Some("premium_plus") {
        return unlock_premium_if_new(state, &session.id, &email).await;
    }

    let points: u32 = metadata
        .get("paw_points")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| "missing paw_points metadata".to_string())?;

    credit_points_if_new(state, &session.id, &email, points).await
}

pub async fn create_breed_guide_checkout_session(
    state: &AppState,
    user_email: &str,
    breed_slug: &str,
    breed_name: &str,
) -> Result<String, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let mut profile = crate::get_or_create_profile(state, user_email).await;
    let customer_id = ensure_stripe_customer(state, &mut profile).await?;

    let base = public_app_url();
    let success_url = format!(
        "{base}/home/breed-guide/{slug}?status=guide_bought&session_id={{CHECKOUT_SESSION_ID}}",
        slug = urlencoding::encode(breed_slug),
    );
    let cancel_url = format!(
        "{base}/home/breed-guide/{slug}?status=guide_cancelled",
        slug = urlencoding::encode(breed_slug),
    );

    let product_name = format!("{breed_name} Premium Care Guide");
    let cents = crate::breed_guides::PRICE_CENTS;
    let client = Client::new();
    let response = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&secret, None::<&str>)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "mode=payment\
&customer={customer}\
&client_reference_id={email}\
&success_url={success}\
&cancel_url={cancel}\
&metadata[user_email]={email}\
&metadata[product_type]=breed_guide\
&metadata[breed_slug]={breed_slug}\
&metadata[paw_points]=0\
&payment_intent_data[setup_future_usage]=off_session\
&saved_payment_method_options[payment_method_save]=enabled\
&line_items[0][quantity]=1\
&line_items[0][price_data][currency]=usd\
&line_items[0][price_data][unit_amount]={cents}\
&line_items[0][price_data][product_data][name]={name}",
            customer = urlencoding::encode(&customer_id),
            email = urlencoding::encode(user_email),
            success = urlencoding::encode(&success_url),
            cancel = urlencoding::encode(&cancel_url),
            breed_slug = urlencoding::encode(breed_slug),
            cents = cents,
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

pub async fn create_premium_checkout_session(
    state: &AppState,
    user_email: &str,
) -> Result<String, CheckoutError> {
    let secret = stripe_secret_key().ok_or(CheckoutError::NotConfigured)?;
    let mut profile = crate::get_or_create_profile(state, user_email).await;
    if crate::entitlements::has_premium(profile.premium_unlocked, user_email) {
        return Err(CheckoutError::StripeApi("already premium".to_string()));
    }

    let customer_id = ensure_stripe_customer(state, &mut profile).await?;

    let base = public_app_url();
    let success_url = format!(
        "{base}/home?tab=account&status=premium_bought&session_id={{CHECKOUT_SESSION_ID}}"
    );
    let cancel_url = format!("{base}/home?tab=account&status=premium_cancelled");

    let product_name = "WhiskerWatch Plus";
    let cents = crate::entitlements::PREMIUM_PRICE_CENTS;
    let client = Client::new();
    let response = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&secret, None::<&str>)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "mode=payment\
&customer={customer}\
&client_reference_id={email}\
&success_url={success}\
&cancel_url={cancel}\
&metadata[user_email]={email}\
&metadata[product_type]=premium_plus\
&metadata[paw_points]=0\
&payment_intent_data[setup_future_usage]=off_session\
&saved_payment_method_options[payment_method_save]=enabled\
&line_items[0][quantity]=1\
&line_items[0][price_data][currency]=usd\
&line_items[0][price_data][unit_amount]={cents}\
&line_items[0][price_data][product_data][name]={name}",
            customer = urlencoding::encode(&customer_id),
            email = urlencoding::encode(user_email),
            success = urlencoding::encode(&success_url),
            cancel = urlencoding::encode(&cancel_url),
            cents = cents,
            name = urlencoding::encode(product_name),
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

pub async fn unlock_premium_if_new(
    state: &AppState,
    session_id: &str,
    email: &str,
) -> Result<bool, String> {
    let inserted = state
        .storage
        .try_record_stripe_fulfillment(session_id, email, 0)
        .map_err(|e| format!("{e:?}"))?;

    if !inserted {
        return Ok(false);
    }

    let mut profile = crate::get_or_create_profile(state, email).await;
    if !profile.premium_unlocked {
        profile.premium_unlocked = true;
        crate::push_activity(
            &mut profile,
            "Unlocked WhiskerWatch Plus — health records, vet logging, and multi-pet support.",
        );
        let today = chrono::Local::now().date_naive();
        profile.calendar_events = crate::merge_calendar_events(&profile, today);
        let _ = crate::refresh_profile_tasks(&mut profile);
        state
            .storage
            .save_profile(&profile)
            .map_err(|e| format!("{e:?}"))?;
    }

    Ok(true)
}

pub async fn unlock_breed_guide_if_new(
    state: &AppState,
    session_id: &str,
    email: &str,
    breed_slug: &str,
) -> Result<bool, String> {
    let inserted = state
        .storage
        .try_record_stripe_fulfillment(session_id, email, 0)
        .map_err(|e| format!("{e:?}"))?;

    if !inserted {
        return Ok(false);
    }

    let Some(guide) = crate::breed_guides::guide_for_slug(breed_slug) else {
        return Err("unknown breed guide".to_string());
    };

    let mut profile = crate::get_or_create_profile(state, email).await;
    if !crate::breed_guides::user_owns_guide(&profile.owned_breed_guides, &guide.slug) {
        profile.owned_breed_guides.push(guide.slug.clone());
        crate::push_activity(
            &mut profile,
            &format!(
                "Unlocked the {} premium care guide.",
                guide.breed_name
            ),
        );
        state
            .storage
            .save_profile(&profile)
            .map_err(|e| format!("{e:?}"))?;
    }

    Ok(true)
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
    if profile.stripe_customer_id.is_none() && stripe_checkout_enabled() {
        let _ = ensure_stripe_customer(state, &mut profile)
            .await
            .map_err(|e| format!("{e:?}"))?;
    }

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
