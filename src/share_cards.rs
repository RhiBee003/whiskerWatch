use chrono::{Duration, NaiveDate};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::UserProfile;

type HmacSha256 = Hmac<Sha256>;

pub const STREAK_MILESTONES: [u32; 6] = [3, 7, 14, 30, 60, 100];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareCardKind {
    LevelUp(u32),
    Streak(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareCardPayload {
    pub pet_name: String,
    pub kind: String,
    pub value: u32,
    pub headline: String,
    pub subline: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShareCardOffer {
    pub token: String,
    pub url: String,
    pub headline: String,
    pub subline: String,
    pub kind: String,
    pub value: u32,
}

const VET_APPOINTMENT_TASK_ID: &str = "vet_appointment_asap";
const FEEDING_TASK_IDS: &[&str] = &[
    "feed_breakfast",
    "feed_lunch",
    "feed_afternoon",
    "feed_dinner",
];

pub fn is_care_streak_task(task_id: &str) -> bool {
    if task_id.starts_with("custom_") {
        return true;
    }

    if task_id == VET_APPOINTMENT_TASK_ID || task_id == "replace_litter" {
        return false;
    }

    if FEEDING_TASK_IDS.contains(&task_id) {
        return true;
    }

    matches!(
        task_id,
        "water_bowl_morning" | "water_bowl_night" | "litter_check" | "play_session"
    )
}

pub fn update_care_streak(profile: &mut UserProfile, today: NaiveDate) -> Option<u32> {
    let today_str = today.format("%Y-%m-%d").to_string();
    if profile.care_streak_last_date.as_deref() == Some(today_str.as_str()) {
        return None;
    }

    let yesterday = today - Duration::days(1);
    let yesterday_str = yesterday.format("%Y-%m-%d").to_string();

    if profile.care_streak_last_date.as_deref() == Some(yesterday_str.as_str()) {
        profile.care_streak_days = profile.care_streak_days.saturating_add(1);
    } else {
        profile.care_streak_days = 1;
    }

    profile.care_streak_last_date = Some(today_str);
    if profile.care_streak_days > profile.best_care_streak {
        profile.best_care_streak = profile.care_streak_days;
    }

    streak_milestone_hit(profile.care_streak_days)
}

fn streak_milestone_hit(days: u32) -> Option<u32> {
    STREAK_MILESTONES
        .iter()
        .copied()
        .find(|milestone| *milestone == days)
}

pub fn share_pet_name(profile: &UserProfile) -> String {
    let name = profile.pet_name.trim();
    if name.is_empty()
        || name.eq_ignore_ascii_case("your cat")
        || name.eq_ignore_ascii_case("no pet yet")
    {
        "my cat".to_string()
    } else {
        name.to_string()
    }
}

pub fn headline_for_kind(pet_name: &str, kind: ShareCardKind) -> (String, u32, String, String) {
    match kind {
        ShareCardKind::LevelUp(level) => (
            "level".to_string(),
            level,
            format!("My cat {pet_name} hit level {level}! 🐾"),
            format!("Level {level} cat parent on WhiskerWatch"),
        ),
        ShareCardKind::Streak(days) => (
            "streak".to_string(),
            days,
            format!("{days}-day care streak for {pet_name}! 🔥🐾"),
            format!("{days} days of cat care on WhiskerWatch"),
        ),
    }
}

pub fn create_share_offer(
    profile: &UserProfile,
    kind: ShareCardKind,
    app_base_url: &str,
    created_at: u64,
) -> Option<ShareCardOffer> {
    let pet = share_pet_name(profile);
    let (kind_label, value, headline, subline) = headline_for_kind(&pet, kind);
    let payload = ShareCardPayload {
        pet_name: pet,
        kind: kind_label.clone(),
        value,
        headline: headline.clone(),
        subline: subline.clone(),
        created_at,
    };
    let token = encode_share_token(&payload)?;
    let base = app_base_url.trim_end_matches('/');
    Some(ShareCardOffer {
        token: token.clone(),
        url: format!("{base}/share/{token}"),
        headline,
        subline,
        kind: kind_label,
        value,
    })
}

pub fn share_offer_for_task_completion(
    profile: &UserProfile,
    level_up: Option<u32>,
    streak_milestone: Option<u32>,
    app_base_url: &str,
    created_at: u64,
) -> Option<ShareCardOffer> {
    if let Some(level) = level_up {
        return create_share_offer(
            profile,
            ShareCardKind::LevelUp(level),
            app_base_url,
            created_at,
        );
    }
    if let Some(days) = streak_milestone {
        return create_share_offer(
            profile,
            ShareCardKind::Streak(days),
            app_base_url,
            created_at,
        );
    }
    None
}

pub fn decode_share_token(token: &str) -> Option<ShareCardPayload> {
    let (payload_hex, signature) = token.split_once('.')?;
    if !constant_time_eq(signature, &sign_payload(payload_hex)) {
        return None;
    }

    let bytes = hex::decode(payload_hex).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn encode_share_token(payload: &ShareCardPayload) -> Option<String> {
    let json = serde_json::to_vec(payload).ok()?;
    let payload_hex = hex::encode(json);
    let signature = sign_payload(&payload_hex);
    Some(format!("{payload_hex}.{signature}"))
}

fn share_signing_secret() -> String {
    std::env::var("SHARE_SIGNING_SECRET")
        .or_else(|_| std::env::var("STRIPE_SECRET_KEY"))
        .unwrap_or_else(|_| "whiskerwatch-dev-share-secret".to_string())
}

fn sign_payload(payload_hex: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(share_signing_secret().as_bytes()).expect("HMAC key length");
    mac.update(payload_hex.as_bytes());
    hex::encode(mac.finalize().into_bytes())
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

pub fn render_share_page_html(payload: &ShareCardPayload, signup_url: &str) -> String {
    let pet = escape_html(&payload.pet_name);
    let headline = escape_html(&payload.headline);
    let subline = escape_html(&payload.subline);
    let badge = if payload.kind == "streak" {
        format!(
            r#"<span class="share-card-badge share-card-badge-streak">{days}-day streak</span>"#,
            days = payload.value
        )
    } else {
        format!(
            r#"<span class="share-card-badge share-card-badge-level">Level {level}</span>"#,
            level = payload.value
        )
    };

    format!(
        r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
    <meta name="theme-color" content="#fac8dd" />
    <title>{headline}</title>
    <meta property="og:title" content="{headline}" />
    <meta property="og:description" content="{subline}" />
    <meta property="og:type" content="website" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content="{headline}" />
    <meta name="twitter:description" content="{subline}" />
    <link rel="stylesheet" href="/styles.css" />
  </head>
  <body class="share-card-page-body">
    <header class="topbar share-card-topbar">
      <a class="brand" href="/" aria-label="WhiskerWatch home">
        <img class="brand-logo" src="/images/logo.png" alt="WhiskerWatch" />
      </a>
      <nav>
        <a href="/signup">Join free</a>
      </nav>
    </header>
    <main class="share-card-page-main section">
      <article class="share-card-public-card" aria-label="Share card">
        <div class="share-card-public-glow" aria-hidden="true"></div>
        {badge}
        <p class="share-card-public-emoji" aria-hidden="true">🐾</p>
        <h1>{headline}</h1>
        <p class="share-card-public-subline">{subline}</p>
        <p class="share-card-public-pet">Celebrating <strong>{pet}</strong> on WhiskerWatch</p>
      </article>
      <div class="share-card-page-cta">
        <p>Track care tasks, earn paw points, and level up as a cat parent.</p>
        <a class="download-btn share-card-signup-btn" href="{signup_url}">Start your cat's journey</a>
      </div>
    </main>
  </body>
</html>"##,
        headline = headline,
        subline = subline,
        pet = pet,
        badge = badge,
        signup_url = escape_html_attr(signup_url),
    )
}

pub fn render_streak_card(profile: &UserProfile, app_base_url: &str, created_at: u64) -> String {
    if profile.care_streak_days == 0 {
        return format!(
            r#"<article class="dashboard-card care-streak-card care-streak-card--empty">
  <h2>Care streak</h2>
  <p class="care-streak-big">Start today</p>
  <p class="field-hint">Complete at least one daily care task each day to start your streak.</p>
  <div class="care-streak-actions"><p class="field-hint">Hit a {next}-day streak to unlock a shareable card.</p></div>
</article>"#,
            next = STREAK_MILESTONES[0],
        );
    }

    let streak_label = if profile.care_streak_days == 1 {
        "1 day".to_string()
    } else {
        format!("{} days", profile.care_streak_days)
    };

    let best_line = if profile.best_care_streak > profile.care_streak_days {
        format!(
            r#"<p class="field-hint streak-best-line">Personal best: {best} days</p>"#,
            best = profile.best_care_streak
        )
    } else {
        String::new()
    };

    let share_button = if profile.care_streak_days >= STREAK_MILESTONES[0] {
        if let Some(offer) = create_share_offer(
            profile,
            ShareCardKind::Streak(profile.care_streak_days),
            app_base_url,
            created_at,
        ) {
            format!(
                r#"<button type="button" class="download-btn share-streak-btn" data-share-url="{url}" data-share-headline="{headline}" data-share-kind="streak" data-share-value="{value}">Share streak</button>"#,
                url = escape_html_attr(&offer.url),
                headline = escape_html_attr(&offer.headline),
                value = offer.value,
            )
        } else {
            String::new()
        }
    } else {
        format!(
            r#"<p class="field-hint">Hit a {next}-day streak to unlock a shareable card.</p>"#,
            next = STREAK_MILESTONES[0],
        )
    };

    format!(
        r#"<article class="dashboard-card care-streak-card">
  <h2>Care streak</h2>
  <p class="care-streak-big">{streak_label}</p>
  <p class="field-hint">Complete at least one daily care task each day to keep your streak alive.</p>
  {best_line}
  <div class="care-streak-actions">{share_button}</div>
</article>"#,
        streak_label = streak_label,
        best_line = best_line,
        share_button = share_button,
    )
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_token_round_trips() {
        let payload = ShareCardPayload {
            pet_name: "Mochi".to_string(),
            kind: "level".to_string(),
            value: 10,
            headline: "My cat Mochi hit level 10! 🐾".to_string(),
            subline: "Level 10 cat parent on WhiskerWatch".to_string(),
            created_at: 1_700_000_000,
        };
        let token = encode_share_token(&payload).expect("token");
        assert_eq!(decode_share_token(&token), Some(payload));
    }

    #[test]
    fn level_headline_matches_example() {
        let (kind, value, headline, _subline) =
            headline_for_kind("Mochi", ShareCardKind::LevelUp(10));
        assert_eq!(kind, "level");
        assert_eq!(value, 10);
        assert_eq!(headline, "My cat Mochi hit level 10! 🐾");
    }
}
