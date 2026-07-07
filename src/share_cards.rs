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
    pub pet_name: String,
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

/// Returns the streak day count that should be shown today.
/// A streak stays alive through today if the last qualifying task was yesterday or today.
pub fn effective_care_streak_days(profile: &UserProfile, today: NaiveDate) -> u32 {
    if profile.care_streak_days == 0 {
        return 0;
    }

    let Some(last_date_str) = profile.care_streak_last_date.as_deref() else {
        return 0;
    };

    let Ok(last_date) = NaiveDate::parse_from_str(last_date_str, "%Y-%m-%d") else {
        return 0;
    };

    let yesterday = today - Duration::days(1);
    if last_date >= yesterday {
        profile.care_streak_days
    } else {
        0
    }
}

/// Resets a stale streak when the user missed one or more days.
pub fn reconcile_care_streak(profile: &mut UserProfile, today: NaiveDate) -> bool {
    let effective = effective_care_streak_days(profile, today);
    if effective == profile.care_streak_days {
        return false;
    }

    profile.care_streak_days = effective;
    true
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
            format!("{pet_name} leveled up to {level}! 🐾✨"),
            format!(
                "Level {level} unlocked — earn XP from daily care tasks, track routines & paw points"
            ),
        ),
        ShareCardKind::Streak(days) => (
            "streak".to_string(),
            days,
            format!("{days} days of loving {pet_name}! 🔥💕"),
            format!("{days}-day care streak — premium cat care made easy, one task at a time"),
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
        pet_name: pet.clone(),
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
        pet_name: pet,
    })
}

pub fn share_offer_for_task_completion(
    profile: &UserProfile,
    streak_milestone: Option<u32>,
    app_base_url: &str,
    created_at: u64,
) -> Option<ShareCardOffer> {
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

pub fn share_win_brand_footer_html() -> &'static str {
    r#"<footer class="share-win-brand">
  <img class="share-win-brand-logo" src="/images/logo.png" alt="" width="52" height="52" decoding="async" aria-hidden="true" />
  <p class="share-win-brand-name">WhiskerWatch</p>
  <p class="share-win-brand-tagline">The all-in-one app for serious cat care — daily routines, paw points &amp; real progress.</p>
  <p class="share-win-brand-cta">Join free at whiskerwatch.com</p>
</footer>"#
}

pub fn render_share_win_card_html(
    pet_name: &str,
    kind: &str,
    value: u32,
    headline: &str,
    subline: &str,
    celebrate_button: bool,
) -> String {
    let pet = escape_html(pet_name);
    let headline_html = escape_html(headline);
    let subline_html = escape_html(subline);
    let kind_class = if kind == "streak" {
        "share-win-card--streak"
    } else {
        "share-win-card--level"
    };
    let kicker = if kind == "streak" {
        "Care streak milestone"
    } else {
        "Parent level up"
    };
    let hero_value = if kind == "streak" {
        format!("{value}")
    } else {
        format!("{value}")
    };
    let hero_label = if kind == "streak" {
        if value == 1 {
            "DAY"
        } else {
            "DAYS"
        }
    } else {
        "LEVEL"
    };
    let hero_emoji = if kind == "streak" { "🔥" } else { "⭐" };
    let tagline = if kind == "streak" {
        "Daily cat care streak — keep the momentum going!"
    } else {
        "Cat parent goals unlocked — care tasks that actually count!"
    };
    let celebrate = if celebrate_button {
        r#"<button type="button" class="share-win-celebrate-btn" data-share-celebrate>Tap for confetti 🎉</button>"#
    } else {
        ""
    };

    format!(
        r#"<article class="share-win-card {kind_class}" data-share-kind="{kind}" data-share-value="{value}">
  <div class="share-win-sparkle-field" aria-hidden="true">
    <span class="share-win-sparkle share-win-sparkle-1">✨</span>
    <span class="share-win-sparkle share-win-sparkle-2">💖</span>
    <span class="share-win-sparkle share-win-sparkle-3">🐾</span>
    <span class="share-win-sparkle share-win-sparkle-4">✨</span>
    <span class="share-win-sparkle share-win-sparkle-5">💕</span>
  </div>
  <div class="share-win-confetti-layer" data-share-confetti aria-hidden="true"></div>
  <p class="share-win-kicker">{kicker}</p>
  <div class="share-win-hero">
    <span class="share-win-hero-emoji" aria-hidden="true">{hero_emoji}</span>
    <span class="share-win-hero-value">{hero_value}</span>
    <span class="share-win-hero-label">{hero_label}</span>
  </div>
  <p class="share-win-headline">{headline_html}</p>
  <p class="share-win-pet">Celebrating <strong>{pet}</strong></p>
  <p class="share-win-tagline">{tagline}</p>
  <p class="share-win-subline">{subline_html}</p>
  {brand_footer}
  {celebrate}
</article>"#,
        kind_class = kind_class,
        kind = escape_html_attr(kind),
        value = value,
        kicker = kicker,
        hero_emoji = hero_emoji,
        hero_value = hero_value,
        hero_label = hero_label,
        headline_html = headline_html,
        pet = pet,
        tagline = tagline,
        subline_html = subline_html,
        brand_footer = share_win_brand_footer_html(),
        celebrate = celebrate,
    )
}

pub fn render_share_page_html(payload: &ShareCardPayload, signup_url: &str) -> String {
    let headline = escape_html(&payload.headline);
    let subline = escape_html(&payload.subline);
    let card = render_share_win_card_html(
        &payload.pet_name,
        &payload.kind,
        payload.value,
        &payload.headline,
        &payload.subline,
        true,
    );

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
    <link rel="stylesheet" href="/styles.css?v=20260613b" />
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
      <div class="share-card-preview-wrap share-card-preview-wrap--public">
        {card}
      </div>
      <div class="share-card-page-cta">
        <p>Track care tasks, earn paw points, and celebrate care streaks.</p>
        <a class="download-btn share-card-signup-btn" href="{signup_url}">Start your cat's journey</a>
      </div>
    </main>
    <script src="/share-cards.js?v=20260613b"></script>
  </body>
</html>"##,
        headline = headline,
        subline = subline,
        card = card,
        signup_url = escape_html_attr(signup_url),
    )
}

#[derive(Debug, Clone, Copy)]
pub enum CuteStreakStyle {
    Chip,
    Card,
    Hero,
}

pub fn format_cute_streak_markup(days: u32, style: CuteStreakStyle) -> String {
    if days == 0 {
        let text = match style {
            CuteStreakStyle::Hero => "Start your streak today ✨",
            _ => "Start today",
        };
        return format!(r#"<span class="care-streak-cute care-streak-cute--start">{text}</span>"#);
    }

    let unit = if days == 1 { "day" } else { "days" };
    let hero_class = if matches!(style, CuteStreakStyle::Hero) {
        " care-streak-cute--hero"
    } else {
        ""
    };
    let suffix = if matches!(style, CuteStreakStyle::Hero) {
        " strong"
    } else {
        ""
    };

    format!(
        r#"<span class="care-streak-cute{hero_class}"><span class="care-streak-num">{days}</span><span class="care-streak-unit">{unit}</span>{suffix}</span>"#,
        hero_class = hero_class,
        days = days,
        unit = unit,
        suffix = suffix,
    )
}

pub fn render_streak_card(profile: &UserProfile, app_base_url: &str, created_at: u64) -> String {
    if profile.care_streak_days == 0 {
        return format!(
            r#"<article class="dashboard-card care-streak-card care-streak-card--empty">
  <h2><a href="/home/streak" class="care-streak-card-link">Care streak</a></h2>
  <p class="care-streak-big"><a href="/home/streak" class="care-streak-card-link">Start today</a></p>
  <p class="field-hint">Complete at least one daily care task each day to start your streak. <a href="/home/streak" class="care-streak-card-link">See rewards waiting for you →</a></p>
  <div class="care-streak-actions"><p class="field-hint">Hit a {next}-day streak to unlock a shareable card.</p></div>
</article>"#,
            next = STREAK_MILESTONES[0],
        );
    }

    let streak_label = format_cute_streak_markup(profile.care_streak_days, CuteStreakStyle::Card);

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
                r#"<button type="button" class="download-btn share-streak-btn" data-share-url="{url}" data-share-headline="{headline}" data-share-kind="streak" data-share-value="{value}" data-share-pet="{pet}">Share streak</button>"#,
                url = escape_html_attr(&offer.url),
                headline = escape_html_attr(&offer.headline),
                value = offer.value,
                pet = escape_html_attr(&offer.pet_name),
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
  <h2><a href="/home/streak" class="care-streak-card-link">Care streak</a></h2>
  <p class="care-streak-big"><a href="/home/streak" class="care-streak-card-link">{streak_label}</a></p>
  <p class="field-hint">Complete at least one daily care task each day to keep your streak alive. <a href="/home/streak" class="care-streak-card-link">View streak rewards →</a></p>
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
        assert_eq!(headline, "Mochi leveled up to 10! 🐾✨");
    }

    #[test]
    fn effective_care_streak_resets_after_missed_days() {
        let mut profile = crate::default_profile("streak@test.com");
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).expect("date");
        let yesterday = NaiveDate::from_ymd_opt(2026, 6, 4).expect("date");
        let two_days_ago = NaiveDate::from_ymd_opt(2026, 6, 3).expect("date");

        profile.care_streak_days = 2;
        profile.care_streak_last_date = Some(two_days_ago.format("%Y-%m-%d").to_string());
        assert_eq!(effective_care_streak_days(&profile, today), 0);
        assert!(reconcile_care_streak(&mut profile, today));
        assert_eq!(profile.care_streak_days, 0);

        profile.care_streak_days = 2;
        profile.care_streak_last_date = Some(yesterday.format("%Y-%m-%d").to_string());
        assert_eq!(effective_care_streak_days(&profile, today), 2);
        assert!(!reconcile_care_streak(&mut profile, today));
        assert_eq!(profile.care_streak_days, 2);
    }
}
