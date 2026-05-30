use axum::{
    Form, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::{
    Deserialize, Serialize,
    de::{Deserializer, Error as DeError},
};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{fs, net::TcpListener};
use tower_http::services::ServeDir;
use uuid::Uuid;

mod storage;
use storage::Storage;

const ADMIN_SESSION_COOKIE: &str = "ww_admin_session";
const USER_SESSION_COOKIE: &str = "ww_user_session";

#[derive(Clone)]
struct AppState {
    storage: Storage,
    admin_sessions: Arc<Mutex<HashSet<String>>>,
    user_sessions: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

#[derive(Deserialize, Default)]
struct LoginQuery {
    error: Option<String>,
    signup: Option<String>,
}

#[derive(Deserialize, Default)]
struct SignupQuery {
    error: Option<String>,
    reason: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize, Default)]
struct FeedbackQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct SignupForm {
    name: String,
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    name: String,
    email: String,
    password: String,
    created_at: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct UserTask {
    id: String,
    title: String,
    completed: bool,
    due_label: String,
    #[serde(default)]
    due_day: Option<u32>,
    #[serde(default)]
    due_month: Option<u32>,
    #[serde(default)]
    due_year: Option<u32>,
    reward: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct CalendarEvent {
    day: u32,
    #[serde(default = "default_calendar_month")]
    month: u32,
    #[serde(default = "default_calendar_year")]
    year: u32,
    title: String,
    time_label: String,
}

fn default_calendar_month() -> u32 {
    5
}

fn default_calendar_year() -> u32 {
    2026
}

#[derive(Serialize, Deserialize, Clone)]
struct VaccineRecord {
    vaccine_name: String,
    date: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ProfileActivity {
    message: String,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct UserProfile {
    email: String,
    paw_points: u32,
    parent_level: u32,
    parent_xp: u32,
    pet_name: String,
    pet_breed: String,
    pet_mood: String,
    pet_emoji: String,
    equipped_outfit: String,
    owned_outfits: Vec<String>,
    #[serde(default)]
    onboarding_completed: bool,
    #[serde(default)]
    pet_age_weeks: Option<u32>,
    #[serde(default)]
    pet_age_years: Option<u32>,
    #[serde(default)]
    last_vet_date: Option<String>,
    #[serde(default)]
    pet_conditions: String,
    #[serde(default)]
    pet_medications: String,
    #[serde(default)]
    pet_indoor_outdoor: Option<String>,
    #[serde(default)]
    vaccine_history: Vec<VaccineRecord>,
    tasks: Vec<UserTask>,
    calendar_events: Vec<CalendarEvent>,
    activity: Vec<ProfileActivity>,
}

struct OutfitCatalogItem {
    id: &'static str,
    name: &'static str,
    emoji: &'static str,
    price: u32,
}

const OUTFIT_CATALOG: [OutfitCatalogItem; 4] = [
    OutfitCatalogItem {
        id: "cozy_sweater",
        name: "Cozy Sweater",
        emoji: "🧶",
        price: 50,
    },
    OutfitCatalogItem {
        id: "party_bow",
        name: "Party Bow Tie",
        emoji: "🎀",
        price: 75,
    },
    OutfitCatalogItem {
        id: "space_helmet",
        name: "Space Helmet",
        emoji: "🪐",
        price: 120,
    },
    OutfitCatalogItem {
        id: "rainbow_scarf",
        name: "Rainbow Scarf",
        emoji: "🌈",
        price: 90,
    },
];

#[derive(Deserialize, Default)]
struct DashboardQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct OutfitBuyForm {
    outfit_id: String,
}

#[derive(Deserialize)]
struct OutfitEquipForm {
    outfit_id: String,
}

#[derive(Deserialize)]
struct TaskToggleForm {
    task_id: String,
}

fn group_form_fields(pairs: Vec<(String, String)>) -> HashMap<String, Vec<String>> {
    let mut fields = HashMap::new();
    for (key, value) in pairs {
        fields.entry(key).or_insert_with(Vec::new).push(value);
    }
    fields
}

fn form_scalar<E: DeError>(
    fields: &HashMap<String, Vec<String>>,
    key: &'static str,
) -> Result<String, E> {
    fields
        .get(key)
        .and_then(|values| values.first())
        .cloned()
        .ok_or_else(|| DeError::missing_field(key))
}

fn form_vec(fields: &HashMap<String, Vec<String>>, key: &str) -> Vec<String> {
    fields.get(key).cloned().unwrap_or_default()
}

struct OnboardingForm {
    cat_name: String,
    age_value: String,
    age_unit: String,
    pet_indoor_outdoor: String,
    last_vet_date: String,
    conditions: String,
    medications: String,
    vaccine_names: Vec<String>,
    vaccine_dates: Vec<String>,
}

impl<'de> Deserialize<'de> for OnboardingForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(String, String)>::deserialize(deserializer)?;
        let fields = group_form_fields(pairs);

        Ok(OnboardingForm {
            cat_name: form_scalar(&fields, "cat_name")?,
            age_value: form_scalar(&fields, "age_value")?,
            age_unit: form_scalar(&fields, "age_unit")?,
            pet_indoor_outdoor: form_scalar(&fields, "pet_indoor_outdoor")?,
            last_vet_date: form_scalar(&fields, "last_vet_date")?,
            conditions: form_scalar(&fields, "conditions")?,
            medications: form_scalar(&fields, "medications")?,
            vaccine_names: form_vec(&fields, "vaccine_names"),
            vaccine_dates: form_vec(&fields, "vaccine_dates"),
        })
    }
}

#[derive(Deserialize)]
struct PawPointsBuyForm {
    package: String,
    card_name: String,
    card_number: String,
    card_expiry: String,
    card_cvv: String,
}

#[derive(Deserialize)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ContactSubmission {
    name: String,
    email: String,
    subject: String,
    message: String,
    submitted_at: u64,
}

#[derive(Deserialize)]
struct FeedbackForm {
    name: String,
    email: String,
    category: String,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct FeedbackSubmission {
    name: String,
    email: String,
    category: String,
    message: String,
    submitted_at: u64,
}

#[derive(Deserialize, Default)]
struct ContactQuery {
    status: Option<String>,
}

fn admin_email() -> String {
    env::var("ADMIN_EMAIL").unwrap_or_else(|_| "rhibee003@gmail.com".to_string())
}

fn admin_password() -> String {
    env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "WhiskerAdmin2026!".to_string())
}

fn listen_address() -> String {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    format!("0.0.0.0:{port}")
}

fn encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html(value)
}

fn timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn format_timestamp(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Unknown".to_string();
    }

    let seconds_per_day = 86_400;
    let days = timestamp / seconds_per_day;
    let hours = (timestamp % seconds_per_day) / 3_600;
    let minutes = (timestamp % 3_600) / 60;
    format!("day {days} {hours:02}:{minutes:02} UTC")
}

fn is_admin_credentials(email: &str, password: &str) -> bool {
    email.eq_ignore_ascii_case(&admin_email()) && password == admin_password()
}

fn admin_session_valid(state: &AppState, jar: &CookieJar) -> bool {
    let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) else {
        return false;
    };

    state
        .admin_sessions
        .lock()
        .expect("admin session lock")
        .contains(cookie.value())
}

fn create_admin_session(state: &AppState, jar: CookieJar) -> CookieJar {
    let session_id = Uuid::new_v4().to_string();
    state
        .admin_sessions
        .lock()
        .expect("admin session lock")
        .insert(session_id.clone());

    let mut cookie = Cookie::new(ADMIN_SESSION_COOKIE, session_id);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    jar.add(cookie)
}

fn clear_admin_session(state: &AppState, jar: CookieJar) -> CookieJar {
    if let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) {
        state
            .admin_sessions
            .lock()
            .expect("admin session lock")
            .remove(cookie.value());
    }

    jar.remove(Cookie::from(ADMIN_SESSION_COOKIE))
}

fn user_session_email(state: &AppState, jar: &CookieJar) -> Option<String> {
    let cookie = jar.get(USER_SESSION_COOKIE)?;
    state
        .user_sessions
        .lock()
        .expect("user session lock")
        .get(cookie.value())
        .cloned()
}

fn user_name_for_email(state: &AppState, email: &str) -> Option<String> {
    state
        .storage
        .find_user_by_email(email)
        .ok()
        .flatten()
        .map(|user| user.name)
}

async fn form_prefill(state: &AppState, jar: &CookieJar) -> (String, String) {
    let Some(email) = user_session_email(state, jar) else {
        return (String::new(), String::new());
    };

    let form_email = escape_html_attr(&email);
    let form_name = user_name_for_email(state, &email)
        .map(|name| escape_html_attr(&name))
        .unwrap_or_default();
    (form_name, form_email)
}

fn clear_user_session(state: &AppState, jar: CookieJar) -> CookieJar {
    if let Some(cookie) = jar.get(USER_SESSION_COOKIE) {
        state
            .user_sessions
            .lock()
            .expect("user session lock")
            .remove(cookie.value());
    }

    jar.remove(Cookie::from(USER_SESSION_COOKIE))
}

fn user_redirect_if_missing(state: &AppState, jar: &CookieJar) -> Result<String, Redirect> {
    user_session_email(state, jar).ok_or_else(|| Redirect::to("/login"))
}

fn default_starter_tasks() -> Vec<UserTask> {
    let today = Local::now().date_naive();
    let yesterday = today.pred_opt().unwrap_or(today);
    let month = today.month();
    let year = today.year() as u32;

    vec![
        UserTask {
            id: "feed_breakfast".to_string(),
            title: "Morning feeding".to_string(),
            completed: false,
            due_label: "Today · 8:00 AM".to_string(),
            due_day: Some(today.day()),
            due_month: Some(month),
            due_year: Some(year),
            reward: 15,
        },
        UserTask {
            id: "play_session".to_string(),
            title: "15-minute play session".to_string(),
            completed: false,
            due_label: "Today · 5:30 PM".to_string(),
            due_day: Some(today.day()),
            due_month: Some(month),
            due_year: Some(year),
            reward: 20,
        },
        UserTask {
            id: "litter_check".to_string(),
            title: "Refresh litter box".to_string(),
            completed: false,
            due_label: "Yesterday".to_string(),
            due_day: Some(yesterday.day()),
            due_month: Some(yesterday.month()),
            due_year: Some(yesterday.year() as u32),
            reward: 10,
        },
        UserTask {
            id: "water_bowl".to_string(),
            title: "Refill water bowl".to_string(),
            completed: false,
            due_label: "Today · anytime".to_string(),
            due_day: Some(today.day()),
            due_month: Some(month),
            due_year: Some(year),
            reward: 12,
        },
    ]
}

fn task_schedule_date(task: &UserTask) -> Option<NaiveDate> {
    if let Some(day) = task.due_day {
        let today = Local::now().date_naive();
        let month = task.due_month.unwrap_or_else(|| today.month());
        let year = task.due_year.unwrap_or_else(|| today.year() as u32);
        return NaiveDate::from_ymd_opt(year as i32, month, day);
    }

    let label = task.due_label.to_lowercase();
    let today = Local::now().date_naive();
    if label.starts_with("today") {
        return Some(today);
    }
    if label.starts_with("yesterday") {
        return today.pred_opt();
    }

    None
}

fn default_profile(email: &str) -> UserProfile {
    UserProfile {
        email: email.to_string(),
        paw_points: 0,
        parent_level: 1,
        parent_xp: 0,
        pet_name: "Your cat".to_string(),
        pet_breed: "Add your cat's details".to_string(),
        pet_mood: "Waiting to meet you".to_string(),
        pet_emoji: "🐱".to_string(),
        equipped_outfit: "Classic Collar".to_string(),
        owned_outfits: vec!["classic_collar".to_string()],
        onboarding_completed: false,
        pet_age_weeks: None,
        pet_age_years: None,
        last_vet_date: None,
        pet_conditions: String::new(),
        pet_medications: String::new(),
        pet_indoor_outdoor: None,
        vaccine_history: vec![],
        tasks: default_starter_tasks(),
        calendar_events: vec![],
        activity: vec![],
    }
}

async fn save_profile(state: &AppState, profile: &UserProfile) -> Result<(), storage::StorageError> {
    state.storage.save_profile(profile)
}

async fn get_or_create_profile(state: &AppState, email: &str) -> UserProfile {
    if let Ok(Some(profile)) = state.storage.load_profile(email) {
        return profile;
    }

    let profile = default_profile(email);
    let _ = save_profile(state, &profile).await;
    profile
}

fn push_activity(profile: &mut UserProfile, message: &str) {
    profile.activity.push(ProfileActivity {
        message: message.to_string(),
        timestamp: timestamp_now(),
    });
    if profile.activity.len() > 8 {
        let overflow = profile.activity.len() - 8;
        profile.activity.drain(0..overflow);
    }
}

fn level_progress(profile: &UserProfile) -> (u32, String) {
    let xp_per_level = 100;
    let progress = (profile.parent_xp * 100) / xp_per_level;
    let remaining = xp_per_level.saturating_sub(profile.parent_xp);
    let text = if remaining == 0 {
        "Ready to level up! Complete more tasks.".to_string()
    } else {
        format!("{remaining} XP to reach level {}.", profile.parent_level + 1)
    };
    (progress.min(100), text)
}

fn outfit_by_id(id: &str) -> Option<&'static OutfitCatalogItem> {
    OUTFIT_CATALOG.iter().find(|item| item.id == id)
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn parse_age(age_value: &str, age_unit: &str) -> Result<(Option<u32>, Option<u32>), ()> {
    let value: u32 = age_value.trim().parse().map_err(|_| ())?;
    if value == 0 {
        return Err(());
    }

    match age_unit.trim().to_lowercase().as_str() {
        "weeks" | "week" => Ok((Some(value), None)),
        "years" | "year" => Ok((None, Some(value))),
        _ => Err(()),
    }
}

fn age_display(profile: &UserProfile) -> String {
    if let Some(weeks) = profile.pet_age_weeks {
        return format!("{weeks} weeks old");
    }
    if let Some(years) = profile.pet_age_years {
        return format!("{years} years old");
    }
    "Age not set".to_string()
}

fn vet_reminder_interval(profile: &UserProfile) -> Duration {
    if profile.pet_age_weeks.is_some_and(|weeks| weeks < 16) {
        return Duration::weeks(4);
    }

    if profile.pet_age_years.is_some_and(|years| years >= 10) {
        return Duration::days(182);
    }

    Duration::days(365)
}

fn format_event_time_label(date: NaiveDate) -> String {
    let month = MONTH_NAMES
        .get(date.month0() as usize)
        .unwrap_or(&"???");
    format!("{month} {} · 10:00 AM", date.day())
}

fn calendar_event_from_date(date: NaiveDate, title: &str) -> CalendarEvent {
    CalendarEvent {
        day: date.day(),
        month: date.month(),
        year: date.year() as u32,
        title: title.to_string(),
        time_label: format_event_time_label(date),
    }
}

fn parse_vet_date(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VaccineKind {
    Fvrcp,
    Rabies,
    Felv,
}

fn normalize_vaccine_kind(name: &str) -> Option<VaccineKind> {
    let key = name
        .trim()
        .to_lowercase()
        .replace([' ', '-', '_'], "");

    if key.contains("fvrcp") || key.contains("felineviral") || key == "distemper" {
        return Some(VaccineKind::Fvrcp);
    }
    if key.contains("rabies") {
        return Some(VaccineKind::Rabies);
    }
    if key.contains("felv") || key.contains("leukemia") {
        return Some(VaccineKind::Felv);
    }

    None
}

fn parse_vaccine_history(names: &[String], dates: &[String]) -> Vec<VaccineRecord> {
    let count = names.len().min(dates.len());
    let mut history = Vec::new();

    for index in 0..count {
        let name = names[index].trim();
        let date = dates[index].trim();
        if name.is_empty() || date.is_empty() || parse_vet_date(date).is_none() {
            continue;
        }
        history.push(VaccineRecord {
            vaccine_name: name.to_string(),
            date: date.to_string(),
        });
    }

    history
}

fn pet_birth_date(profile: &UserProfile, reference: NaiveDate) -> Option<NaiveDate> {
    if let Some(weeks) = profile.pet_age_weeks {
        return reference.checked_sub_signed(Duration::weeks(weeks as i64));
    }
    if let Some(years) = profile.pet_age_years {
        return reference.checked_sub_signed(Duration::days(i64::from(years) * 365));
    }
    None
}

fn history_dates_for_kind(history: &[VaccineRecord], kind: VaccineKind) -> Vec<NaiveDate> {
    history
        .iter()
        .filter_map(|record| {
            normalize_vaccine_kind(&record.vaccine_name)
                .filter(|record_kind| *record_kind == kind)
                .and_then(|_| parse_vet_date(&record.date))
        })
        .collect()
}

fn latest_history_date(history: &[VaccineRecord], kind: VaccineKind) -> Option<NaiveDate> {
    history_dates_for_kind(history, kind).into_iter().max()
}

fn is_dose_satisfied(kind: VaccineKind, target: NaiveDate, history: &[VaccineRecord]) -> bool {
    history_dates_for_kind(history, kind)
        .into_iter()
        .any(|given| {
            let delta = (given - target).num_days();
            delta >= -21 && delta <= 42
        })
}

fn indoor_outdoor_display(value: Option<&str>) -> String {
    match value.map(str::trim).map(str::to_lowercase).as_deref() {
        Some("outdoor") => "Outdoor cat".to_string(),
        Some("indoor") => "Indoor cat".to_string(),
        _ => "Not set".to_string(),
    }
}

fn is_outdoor_cat(profile: &UserProfile) -> bool {
    profile
        .pet_indoor_outdoor
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("outdoor"))
}

fn week_from_birth(birth: NaiveDate, weeks: u32) -> NaiveDate {
    birth + Duration::weeks(i64::from(weeks))
}

fn push_vaccine_reminder(
    events: &mut Vec<CalendarEvent>,
    date: NaiveDate,
    label: &str,
    pet_name: &str,
    today: NaiveDate,
    horizon: NaiveDate,
) {
    if date < today || date > horizon {
        return;
    }
    events.push(calendar_event_from_date(
        date,
        &format!("{label} — {pet_name}"),
    ));
}

fn schedule_kitten_vaccines(
    events: &mut Vec<CalendarEvent>,
    profile: &UserProfile,
    birth: NaiveDate,
    today: NaiveDate,
    horizon: NaiveDate,
    pet_name: &str,
) {
    let history = &profile.vaccine_history;

    for week in [6u32, 10, 14, 18] {
        let target = week_from_birth(birth, week);
        if !is_dose_satisfied(VaccineKind::Fvrcp, target, history) {
            push_vaccine_reminder(events, target, "FVRCP vaccine", pet_name, today, horizon);
        }
    }

    if let Some(last_fvrcp) = latest_history_date(history, VaccineKind::Fvrcp) {
        let mut next = last_fvrcp + Duration::weeks(4);
        while next <= horizon {
            let age_weeks = next.signed_duration_since(birth).num_weeks();
            if age_weeks > 20 {
                break;
            }
            if !is_dose_satisfied(VaccineKind::Fvrcp, next, history) {
                push_vaccine_reminder(events, next, "FVRCP vaccine", pet_name, today, horizon);
            }
            next += Duration::weeks(4);
        }
    }

    let rabies_at = week_from_birth(birth, 15);
    if !is_dose_satisfied(VaccineKind::Rabies, rabies_at, history) {
        push_vaccine_reminder(events, rabies_at, "Rabies vaccine", pet_name, today, horizon);
    }

    let felv_at = week_from_birth(birth, 8);
    if !is_dose_satisfied(VaccineKind::Felv, felv_at, history) {
        push_vaccine_reminder(events, felv_at, "FeLV vaccine", pet_name, today, horizon);
    }

    let felv_booster_at = latest_history_date(history, VaccineKind::Felv)
        .map(|first| first + Duration::weeks(4))
        .unwrap_or_else(|| week_from_birth(birth, 12));
    if !is_dose_satisfied(VaccineKind::Felv, felv_booster_at, history) {
        push_vaccine_reminder(
            events,
            felv_booster_at,
            "FeLV booster",
            pet_name,
            today,
            horizon,
        );
    }
}

fn schedule_adult_vaccines(
    events: &mut Vec<CalendarEvent>,
    profile: &UserProfile,
    birth: NaiveDate,
    today: NaiveDate,
    horizon: NaiveDate,
    pet_name: &str,
) {
    let history = &profile.vaccine_history;
    let one_year = birth + Duration::weeks(52);

    let mut fvrcp_next = latest_history_date(history, VaccineKind::Fvrcp)
        .map(|last| last + Duration::days(365 * 3))
        .unwrap_or(one_year);
    while fvrcp_next < today {
        fvrcp_next += Duration::days(365 * 3);
    }
    while fvrcp_next <= horizon {
        if !is_dose_satisfied(VaccineKind::Fvrcp, fvrcp_next, history) {
            let label = if fvrcp_next == one_year {
                "FVRCP booster (1 year)"
            } else {
                "FVRCP booster"
            };
            push_vaccine_reminder(events, fvrcp_next, label, pet_name, today, horizon);
        }
        fvrcp_next += Duration::days(365 * 3);
    }

    let mut rabies_next = latest_history_date(history, VaccineKind::Rabies)
        .map(|last| last + Duration::days(365 * 3))
        .unwrap_or(one_year);
    while rabies_next < today {
        rabies_next += Duration::days(365 * 3);
    }
    while rabies_next <= horizon {
        if !is_dose_satisfied(VaccineKind::Rabies, rabies_next, history) {
            let label = if rabies_next == one_year {
                "Rabies booster (1 year)"
            } else {
                "Rabies booster"
            };
            push_vaccine_reminder(events, rabies_next, label, pet_name, today, horizon);
        }
        rabies_next += Duration::days(365 * 3);
    }

    let felv_interval = if is_outdoor_cat(profile) {
        Duration::days(365)
    } else {
        Duration::days(365 * 3)
    };

    let mut felv_next = latest_history_date(history, VaccineKind::Felv)
        .map(|last| last + felv_interval)
        .unwrap_or(one_year);
    while felv_next < today {
        felv_next += felv_interval;
    }
    while felv_next <= horizon {
        if !is_dose_satisfied(VaccineKind::Felv, felv_next, history) {
            let label = if felv_next == one_year {
                "FeLV vaccine (1 year)"
            } else if is_outdoor_cat(profile) {
                "FeLV vaccine (yearly)"
            } else {
                "FeLV vaccine (3-year)"
            };
            push_vaccine_reminder(events, felv_next, label, pet_name, today, horizon);
        }
        felv_next += felv_interval;
    }
}

fn generate_vaccine_calendar_events(
    profile: &UserProfile,
    reference_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let Some(birth) = pet_birth_date(profile, reference_date) else {
        return Vec::new();
    };

    let pet_name = if profile.pet_name.is_empty() {
        "Your cat".to_string()
    } else {
        profile.pet_name.clone()
    };

    let today = reference_date;
    let horizon = reference_date + Duration::days(730);
    let mut events = Vec::new();

    if let Some(weeks) = profile.pet_age_weeks {
        if weeks <= 20 {
            schedule_kitten_vaccines(&mut events, profile, birth, today, horizon, &pet_name);
        }
        if weeks > 20 {
            schedule_adult_vaccines(&mut events, profile, birth, today, horizon, &pet_name);
        }
    } else if profile.pet_age_years.is_some_and(|years| (1..=10).contains(&years)) {
        schedule_adult_vaccines(&mut events, profile, birth, today, horizon, &pet_name);
    }

    events.sort_by_key(|event| (event.year, event.month, event.day));
    events
}

fn merge_calendar_events(profile: &UserProfile, signup_date: NaiveDate) -> Vec<CalendarEvent> {
    let mut events = generate_vet_calendar_events(profile, signup_date);
    events.extend(generate_vaccine_calendar_events(profile, signup_date));
    events.sort_by_key(|event| (event.year, event.month, event.day));
    events
}

fn generate_vet_calendar_events(profile: &UserProfile, signup_date: NaiveDate) -> Vec<CalendarEvent> {
    let anchor = profile
        .last_vet_date
        .as_deref()
        .and_then(parse_vet_date)
        .unwrap_or(signup_date);

    let pet_name = if profile.pet_name.is_empty() {
        "Your cat".to_string()
    } else {
        profile.pet_name.clone()
    };

    let mut events = Vec::new();

    if profile.last_vet_date.is_some() {
        events.push(calendar_event_from_date(
            anchor,
            &format!("Last vet visit — {pet_name}"),
        ));
    }

    let interval = vet_reminder_interval(profile);
    let reminder_title = format!("Vet checkup reminder — {pet_name}");
    let horizon = signup_date + Duration::days(730);
    let mut next = anchor + interval;

    while next <= horizon {
        if profile.last_vet_date.is_none() || next > anchor {
            events.push(calendar_event_from_date(next, &reminder_title));
        }
        next += interval;
    }

    events.sort_by_key(|event| (event.year, event.month, event.day));
    events
}

fn render_pet_health_info(profile: &UserProfile) -> String {
    if !profile.onboarding_completed {
        return String::new();
    }

    let last_vet = profile
        .last_vet_date
        .as_deref()
        .map(|date| escape_html(date))
        .unwrap_or_else(|| "Not recorded".to_string());

    let conditions = if profile.pet_conditions.trim().is_empty() {
        "None noted".to_string()
    } else {
        escape_html(&profile.pet_conditions)
    };

    let medications = if profile.pet_medications.trim().is_empty() {
        "None noted".to_string()
    } else {
        escape_html(&profile.pet_medications)
    };

    let lifestyle = escape_html(&indoor_outdoor_display(
        profile.pet_indoor_outdoor.as_deref(),
    ));

    let vaccine_list = if profile.vaccine_history.is_empty() {
        "None recorded".to_string()
    } else {
        let items: String = profile
            .vaccine_history
            .iter()
            .map(|record| {
                format!(
                    "<li><strong>{}</strong> — {}</li>",
                    escape_html(&record.vaccine_name),
                    escape_html(&record.date)
                )
            })
            .collect();
        format!(r#"<ul class="vaccine-history-list">{items}</ul>"#)
    };

    format!(
        r#"<dl class="pet-health-dl"><dt>Age</dt><dd>{age}</dd><dt>Lifestyle</dt><dd>{lifestyle}</dd><dt>Last vet appointment</dt><dd>{last_vet}</dd><dt>Conditions</dt><dd>{conditions}</dd><dt>Medications</dt><dd>{medications}</dd><dt>Vaccine history</dt><dd>{vaccine_list}</dd></dl>"#,
        age = escape_html(&age_display(profile)),
        lifestyle = lifestyle,
        last_vet = last_vet,
        conditions = conditions,
        medications = medications,
        vaccine_list = vaccine_list,
    )
}

fn render_onboarding_modal(profile: &UserProfile) -> String {
    if profile.onboarding_completed {
        return String::new();
    }

    r#"<div class="onboarding-backdrop" id="onboarding-modal" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
  <div class="onboarding-modal">
    <h2 id="onboarding-title">Tell us about your cat 🐾</h2>
    <p class="onboarding-intro">We will personalize your pet tab and schedule vet and vaccine reminders on your calendar.</p>
    <form class="onboarding-form login-form" action="/home/onboarding" method="post">
      <label for="cat_name">Cat's name</label>
      <input id="cat_name" name="cat_name" type="text" placeholder="Mochi" required />

      <div class="age-row">
        <div>
          <label for="age_value">Age</label>
          <input id="age_value" name="age_value" type="number" min="1" placeholder="12" required />
        </div>
        <div>
          <label for="age_unit">Unit</label>
          <select id="age_unit" name="age_unit" required>
            <option value="weeks">Weeks (under 1 year)</option>
            <option value="years" selected>Years</option>
          </select>
        </div>
      </div>
      <p class="field-hint">Use weeks for kittens (6–20 weeks) so we can schedule FVRCP, rabies, and FeLV doses. Cats 1–10 years get booster schedules; 10+ years get vet reminders every 6 months.</p>

      <fieldset class="indoor-outdoor-fieldset">
        <legend>Indoor or outdoor cat?</legend>
        <label class="radio-pill"><input type="radio" name="pet_indoor_outdoor" value="indoor" required /> Indoor</label>
        <label class="radio-pill"><input type="radio" name="pet_indoor_outdoor" value="outdoor" required /> Outdoor</label>
      </fieldset>
      <p class="field-hint">Outdoor cats need FeLV vaccines yearly; indoor cats every 3 years after the first year.</p>

      <label for="last_vet_date">Last vet appointment</label>
      <input id="last_vet_date" name="last_vet_date" type="date" />
      <p class="field-hint">Optional — leave blank if this is their first visit. We will start reminders from today.</p>

      <fieldset class="vaccine-history-fieldset">
        <legend>Vaccine history</legend>
        <p class="field-hint">Record vaccines your cat already received so we do not duplicate reminders.</p>
        <div id="vaccine-rows" class="vaccine-rows">
          <div class="vaccine-row">
            <select name="vaccine_names" aria-label="Vaccine name">
              <option value="">Select vaccine</option>
              <option value="FVRCP">FVRCP</option>
              <option value="Rabies">Rabies</option>
              <option value="FeLV">FeLV</option>
              <option value="Other">Other</option>
            </select>
            <input name="vaccine_dates" type="date" aria-label="Vaccine date" />
            <button type="button" class="vaccine-remove-btn" hidden aria-label="Remove vaccine row">×</button>
          </div>
        </div>
        <button type="button" class="download-btn vaccine-add-btn" id="add-vaccine-row">+ Add vaccine</button>
      </fieldset>

      <label for="conditions">Health conditions</label>
      <textarea id="conditions" name="conditions" rows="2" placeholder="e.g. asthma, arthritis"></textarea>

      <label for="medications">Medications</label>
      <textarea id="medications" name="medications" rows="2" placeholder="e.g. flea prevention monthly"></textarea>

      <button type="submit" class="download-btn login-submit">Save &amp; continue</button>
    </form>
  </div>
</div>"#
        .to_string()
}

fn current_calendar_month() -> u32 {
    Local::now().month()
}

fn current_calendar_year() -> u32 {
    Local::now().year() as u32
}

fn calendar_month_label(month: u32, year: u32) -> String {
    let name = MONTH_NAMES.get(month.saturating_sub(1) as usize).unwrap_or(&"???");
    format!("{name} {year} — your cat care schedule")
}

fn create_user_session(state: &AppState, jar: CookieJar, email: &str) -> CookieJar {
    let session_id = Uuid::new_v4().to_string();
    state
        .user_sessions
        .lock()
        .expect("user session lock")
        .insert(session_id.clone(), email.to_string());

    let mut cookie = Cookie::new(USER_SESSION_COOKIE, session_id);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    jar.add(cookie)
}

fn signed_in_redirect(state: &AppState, jar: CookieJar, email: &str) -> Response {
    let jar = create_user_session(state, jar, email);
    (jar, Redirect::to("/home")).into_response()
}

async fn index_page(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() {
        return Redirect::to("/home").into_response();
    }

    match fs::read_to_string("static/index.html").await {
        Ok(contents) => {
            let html = contents.replace(
                "{{AUTH_NAV_LINK}}",
                r#"<a href="/login">LOG IN</a>"#,
            );
            Html(html).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load homepage".to_string(),
        )
            .into_response(),
    }
}

fn dashboard_status_block(status: Option<&str>) -> String {
    match status {
        Some("outfit_bought") => {
            r#"<p class="auth-success" role="status">Outfit purchased and equipped! Your pet looks adorable.</p>"#
        }
        Some("outfit_equipped") => {
            r#"<p class="auth-success" role="status">Outfit equipped for your pet.</p>"#
        }
        Some("outfit_owned") => {
            r#"<p class="auth-error" role="alert">You already own that outfit.</p>"#
        }
        Some("outfit_points") => {
            r#"<p class="auth-error" role="alert">Not enough paw points for that outfit.</p>"#
        }
        Some("outfit_invalid") => {
            r#"<p class="auth-error" role="alert">That outfit is not available.</p>"#
        }
        Some("points_bought") => {
            r#"<p class="auth-success" role="status">Paw points added! Thanks for your purchase.</p>"#
        }
        Some("points_invalid") => {
            r#"<p class="auth-error" role="alert">Please fill out all card fields to purchase points.</p>"#
        }
        Some("task_done") => {
            r#"<p class="auth-success" role="status">Task completed! Paw points and XP added.</p>"#
        }
        Some("task_reopened") => {
            r#"<p class="auth-success" role="status">Task marked as incomplete.</p>"#
        }
        Some("task_invalid") => {
            r#"<p class="auth-error" role="alert">That task could not be updated.</p>"#
        }
        Some("onboarding_done") => {
            r#"<p class="auth-success" role="status">Welcome! Your cat profile is saved with vet and vaccine reminders on your calendar.</p>"#
        }
        Some("onboarding_invalid") => {
            r#"<p class="auth-error" role="alert">Please enter your cat's name, a valid age, and whether they are indoor or outdoor.</p>"#
        }
        _ => "",
    }
    .to_string()
}

fn render_activity_list(profile: &UserProfile) -> String {
    if profile.activity.is_empty() {
        return "<li>No activity yet — complete a task to get started!</li>".to_string();
    }

    profile
        .activity
        .iter()
        .rev()
        .take(5)
        .map(|item| format!("<li>{}</li>", escape_html(&item.message)))
        .collect()
}

fn render_outfit_cards(profile: &UserProfile) -> String {
    OUTFIT_CATALOG
        .iter()
        .map(|outfit| {
            let owned = profile.owned_outfits.iter().any(|id| id == outfit.id);
            let equipped = profile.equipped_outfit == outfit.name;
            let mut classes = vec!["outfit-card"];
            if owned {
                classes.push("owned");
            }
            if equipped {
                classes.push("equipped");
            }

            let action = if equipped {
                r#"<span class="outfit-badge">Currently equipped</span>"#.to_string()
            } else if owned {
                format!(
                    r#"<form action="/home/outfits/equip" method="post"><input type="hidden" name="outfit_id" value="{}" /><button type="submit" class="download-btn outfit-btn">Equip</button></form>"#,
                    escape_html_attr(outfit.id)
                )
            } else {
                format!(
                    r#"<form action="/home/outfits/buy" method="post"><input type="hidden" name="outfit_id" value="{}" /><button type="submit" class="download-btn outfit-btn">Buy for {} pts</button></form>"#,
                    escape_html_attr(outfit.id),
                    outfit.price
                )
            };

            format!(
                r#"<article class="{}"><div class="outfit-emoji">{}</div><h3>{}</h3><p class="outfit-price">{} paw points</p><div class="outfit-actions">{}</div></article>"#,
                classes.join(" "),
                outfit.emoji,
                escape_html(outfit.name),
                outfit.price,
                action
            )
        })
        .collect()
}

fn render_task_list(profile: &UserProfile) -> String {
    profile
        .tasks
        .iter()
        .map(|task| {
            let completed_class = if task.completed { " completed" } else { "" };
            let button_label = if task.completed {
                "Mark incomplete"
            } else {
                "Complete"
            };
            format!(
                r#"<li class="task-item{completed_class}"><div><p class="task-title">{title}</p><p class="task-due">{due} · +{reward} pts</p></div><form action="/home/tasks/toggle" method="post"><input type="hidden" name="task_id" value="{id}" /><button type="submit" class="download-btn task-toggle-btn">{button_label}</button></form></li>"#,
                completed_class = completed_class,
                title = escape_html(&task.title),
                due = escape_html(&task.due_label),
                reward = task.reward,
                id = escape_html_attr(&task.id),
                button_label = button_label,
            )
        })
        .collect()
}

fn render_calendar_grid(profile: &UserProfile, month: u32, year: u32) -> String {
    let weekday_labels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut html = String::new();

    for label in weekday_labels {
        html.push_str(&format!(r#"<span class="calendar-head">{label}</span>"#));
    }

    let first_of_month = NaiveDate::from_ymd_opt(year as i32, month, 1).unwrap_or_else(|| {
        NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid fallback date")
    });
    let first_weekday = first_of_month.weekday().num_days_from_sunday();
    let days_in_month = if month == 12 {
        NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year as i32, month + 1, 1)
    }
    .and_then(|next| next.pred_opt())
    .map(|d| d.day())
    .unwrap_or(31);

    let today = Local::now().date_naive();
    let today_in_view = today.year() as u32 == year && today.month() == month;

    for _ in 0..first_weekday {
        html.push_str(r#"<span class="calendar-day empty"></span>"#);
    }

    let event_days: HashSet<u32> = profile
        .calendar_events
        .iter()
        .filter(|e| e.month == month && e.year == year)
        .map(|e| e.day)
        .collect();

    let task_days: HashSet<u32> = profile
        .tasks
        .iter()
        .filter_map(task_schedule_date)
        .filter(|date| date.month() == month && date.year() as u32 == year)
        .map(|date| date.day())
        .collect();

    for day in 1..=days_in_month {
        let mut classes = vec!["calendar-day"];
        if today_in_view && day == today.day() {
            classes.push("today");
        }
        if event_days.contains(&day) {
            classes.push("has-event");
        }
        if task_days.contains(&day) {
            classes.push("has-task");
        }
        let month_name = MONTH_NAMES.get(month.saturating_sub(1) as usize).unwrap_or(&"???");
        html.push_str(&format!(
            r#"<button type="button" class="{}" data-day="{day}" data-month="{month}" data-year="{year}" aria-label="{month_name} {day}, {year}" aria-pressed="false">{day}</button>"#,
            classes.join(" ")
        ));
    }

    html
}

fn render_event_list(profile: &UserProfile) -> String {
    if profile.calendar_events.is_empty() {
        return "<li>No upcoming events yet.</li>".to_string();
    }

    profile
        .calendar_events
        .iter()
        .map(|event| {
            format!(
                r#"<li data-day="{day}" data-month="{month}" data-year="{year}"><strong>{time}</strong> — {title}</li>"#,
                day = event.day,
                month = event.month,
                year = event.year,
                time = escape_html(&event.time_label),
                title = escape_html(&event.title),
            )
        })
        .collect()
}

fn render_calendar_data_json(profile: &UserProfile, month: u32, year: u32) -> String {
    let today = Local::now().date_naive();
    let events: Vec<_> = profile
        .calendar_events
        .iter()
        .map(|event| {
            serde_json::json!({
                "day": event.day,
                "month": event.month,
                "year": event.year,
                "title": event.title,
                "time_label": event.time_label,
            })
        })
        .collect();

    let tasks: Vec<_> = profile
        .tasks
        .iter()
        .filter_map(|task| {
            task_schedule_date(task).map(|date| {
                serde_json::json!({
                    "day": date.day(),
                    "month": date.month(),
                    "year": date.year(),
                    "id": task.id,
                    "title": task.title,
                    "due_label": task.due_label,
                    "reward": task.reward,
                    "completed": task.completed,
                })
            })
        })
        .collect();

    let payload = serde_json::json!({
        "viewMonth": month,
        "viewYear": year,
        "todayDay": if today.year() as u32 == year && today.month() == month {
            today.day()
        } else {
            0
        },
        "events": events,
        "tasks": tasks,
    });

    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

async fn member_since_label(state: &AppState, email: &str) -> String {
    state
        .storage
        .find_user_by_email(email)
        .ok()
        .flatten()
        .map(|user| format_timestamp(user.created_at))
        .unwrap_or_else(|| "Recently joined".to_string())
}

async fn dashboard_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<DashboardQuery>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let profile = get_or_create_profile(&state, &email).await;
    let user_name = user_name_for_email(&state, &email).unwrap_or_else(|| "Parent".to_string());
    let (level_progress_pct, level_progress_text) = level_progress(&profile);
    let calendar_month = current_calendar_month();
    let calendar_year = current_calendar_year();

    let template = match fs::read_to_string("templates/dashboard.html").await {
        Ok(contents) => contents,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load dashboard".to_string(),
            )
                .into_response()
        }
    };

    let body = template
        .replace("{{USER_NAME}}", &escape_html(&user_name))
        .replace("{{USER_EMAIL}}", &escape_html(&email))
        .replace("{{MEMBER_SINCE}}", &escape_html(&member_since_label(&state, &email).await))
        .replace("{{PAW_POINTS}}", &profile.paw_points.to_string())
        .replace("{{PARENT_LEVEL}}", &profile.parent_level.to_string())
        .replace("{{LEVEL_PROGRESS}}", &level_progress_pct.to_string())
        .replace("{{LEVEL_PROGRESS_TEXT}}", &escape_html(&level_progress_text))
        .replace("{{PET_NAME}}", &escape_html(&profile.pet_name))
        .replace("{{PET_BREED}}", &escape_html(&profile.pet_breed))
        .replace("{{PET_MOOD}}", &escape_html(&profile.pet_mood))
        .replace("{{PET_EMOJI}}", &profile.pet_emoji)
        .replace("{{PET_HEALTH_INFO}}", &render_pet_health_info(&profile))
        .replace("{{ONBOARDING_MODAL}}", &render_onboarding_modal(&profile))
        .replace("{{EQUIPPED_OUTFIT}}", &escape_html(&profile.equipped_outfit))
        .replace("{{STATUS_BLOCK}}", &dashboard_status_block(query.status.as_deref()))
        .replace("{{ACTIVITY_LIST}}", &render_activity_list(&profile))
        .replace("{{OUTFIT_CARDS}}", &render_outfit_cards(&profile))
        .replace("{{TASK_LIST}}", &render_task_list(&profile))
        .replace(
            "{{CALENDAR_GRID}}",
            &render_calendar_grid(&profile, calendar_month, calendar_year),
        )
        .replace("{{EVENT_LIST}}", &render_event_list(&profile))
        .replace(
            "{{CALENDAR_DATA_JSON}}",
            &render_calendar_data_json(&profile, calendar_month, calendar_year),
        )
        .replace(
            "{{CALENDAR_MONTH_LABEL}}",
            &calendar_month_label(calendar_month, calendar_year),
        );

    Html(body).into_response()
}

async fn onboarding_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<OnboardingForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let cat_name = form.cat_name.trim();
    if cat_name.is_empty() {
        return Redirect::to("/home?status=onboarding_invalid");
    }

    let (pet_age_weeks, pet_age_years) = match parse_age(&form.age_value, &form.age_unit) {
        Ok(age) => age,
        Err(()) => return Redirect::to("/home?status=onboarding_invalid"),
    };

    let indoor_outdoor = form.pet_indoor_outdoor.trim().to_lowercase();
    if indoor_outdoor != "indoor" && indoor_outdoor != "outdoor" {
        return Redirect::to("/home?status=onboarding_invalid");
    }

    let vaccine_history = parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates);

    let last_vet_date = {
        let trimmed = form.last_vet_date.trim();
        if trimmed.is_empty() {
            None
        } else if parse_vet_date(trimmed).is_some() {
            Some(trimmed.to_string())
        } else {
            return Redirect::to("/home?status=onboarding_invalid");
        }
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    let signup_date = Local::now().date_naive();

    profile.pet_name = cat_name.to_string();
    profile.pet_breed = "Your companion".to_string();
    profile.pet_mood = "Happy".to_string();
    profile.pet_age_weeks = pet_age_weeks;
    profile.pet_age_years = pet_age_years;
    profile.last_vet_date = last_vet_date;
    profile.pet_conditions = form.conditions.trim().to_string();
    profile.pet_medications = form.medications.trim().to_string();
    profile.pet_indoor_outdoor = Some(indoor_outdoor);
    profile.vaccine_history = vaccine_history;
    profile.onboarding_completed = true;
    profile.calendar_events = merge_calendar_events(&profile, signup_date);

    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Set up {pet_name}'s profile, vet visits, and vaccine schedule."),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?status=onboarding_done"),
        Err(_) => Redirect::to("/home?status=onboarding_invalid"),
    }
}

async fn outfit_buy(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<OutfitBuyForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let Some(outfit) = outfit_by_id(form.outfit_id.trim()) else {
        return Redirect::to("/home?tab=outfits&status=outfit_invalid");
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if profile.owned_outfits.iter().any(|id| id == outfit.id) {
        return Redirect::to("/home?tab=outfits&status=outfit_owned");
    }

    if profile.paw_points < outfit.price {
        return Redirect::to("/home?tab=outfits&status=outfit_points");
    }

    profile.paw_points -= outfit.price;
    profile.owned_outfits.push(outfit.id.to_string());
    profile.equipped_outfit = outfit.name.to_string();
    push_activity(
        &mut profile,
        &format!("Purchased {} for {} paw points.", outfit.name, outfit.price),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=outfits&status=outfit_bought"),
        Err(_) => Redirect::to("/home?tab=outfits&status=outfit_invalid"),
    }
}

async fn outfit_equip(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<OutfitEquipForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let Some(outfit) = outfit_by_id(form.outfit_id.trim()) else {
        return Redirect::to("/home?tab=outfits&status=outfit_invalid");
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile.owned_outfits.iter().any(|id| id == outfit.id) {
        return Redirect::to("/home?tab=outfits&status=outfit_invalid");
    }

    profile.equipped_outfit = outfit.name.to_string();
    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Equipped {} on {}.", outfit.name, pet_name),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=outfits&status=outfit_equipped"),
        Err(_) => Redirect::to("/home?tab=outfits&status=outfit_invalid"),
    }
}

async fn task_toggle(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<TaskToggleForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    let task_id = form.task_id.trim();

    let Some(index) = profile.tasks.iter().position(|task| task.id == task_id) else {
        return Redirect::to("/home?tab=tasks&status=task_invalid");
    };

    if profile.tasks[index].completed {
        let title = profile.tasks[index].title.clone();
        profile.tasks[index].completed = false;
        push_activity(&mut profile, &format!("Reopened task: {title}."));
        return match save_profile(&state, &profile).await {
            Ok(()) => Redirect::to("/home?tab=tasks&status=task_reopened"),
            Err(_) => Redirect::to("/home?tab=tasks&status=task_invalid"),
        };
    }

    let reward = profile.tasks[index].reward;
    let title = profile.tasks[index].title.clone();
    profile.tasks[index].completed = true;
    profile.paw_points += reward;
    profile.parent_xp += reward / 2;
    if profile.parent_xp >= 100 {
        profile.parent_xp -= 100;
        profile.parent_level += 1;
        let new_level = profile.parent_level;
        push_activity(
            &mut profile,
            &format!("Leveled up to Parent Level {new_level}!"),
        );
    }
    push_activity(
        &mut profile,
        &format!("Completed \"{title}\" and earned {reward} paw points."),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=tasks&status=task_done"),
        Err(_) => Redirect::to("/home?tab=tasks&status=task_invalid"),
    }
}

async fn paw_points_buy(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<PawPointsBuyForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let card_name = form.card_name.trim();
    let card_number = form.card_number.trim();
    let card_expiry = form.card_expiry.trim();
    let card_cvv = form.card_cvv.trim();

    if card_name.is_empty() || card_number.is_empty() || card_expiry.is_empty() || card_cvv.is_empty()
    {
        return Redirect::to("/home?tab=account&status=points_invalid");
    }

    let points: u32 = match form.package.trim() {
        "100" => 100,
        "250" => 250,
        "500" => 500,
        "1000" => 1000,
        "5000" => 5000,
        _ => return Redirect::to("/home?tab=account&status=points_invalid"),
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    profile.paw_points += points;
    push_activity(
        &mut profile,
        &format!("Purchased {points} paw points with card ending {}.", &card_number[card_number.len().saturating_sub(4)..]),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account&status=points_bought"),
        Err(_) => Redirect::to("/home?tab=account&status=points_invalid"),
    }
}

async fn user_logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = clear_user_session(&state, jar);
    (jar, Redirect::to("/")).into_response()
}

async fn login_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() {
        return Redirect::to("/home").into_response();
    }

    match fs::read_to_string("templates/login.html").await {
        Ok(contents) => {
            let login_error_block = match query.error.as_deref() {
                Some("invalid") => {
                    r#"<p class="auth-error" role="alert">Incorrect password. Please try again.</p>"#
                }
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please enter both email and password.</p>"#
                }
                _ => "",
            };
            let signup_success_block = match query.signup.as_deref() {
                Some("created") => r#"<p class="auth-success" role="status">Account created! You can log in with your new email and password.</p>"#,
                _ => "",
            };
            let body = contents
                .replace("{{LOGIN_ERROR_BLOCK}}", login_error_block)
                .replace("{{SIGNUP_SUCCESS_BLOCK}}", signup_success_block);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load login page".to_string(),
        )
            .into_response(),
    }
}

async fn signup_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SignupQuery>,
) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() {
        return Redirect::to("/home").into_response();
    }

    match fs::read_to_string("templates/signup.html").await {
        Ok(contents) => {
            let signup_error_block = match query.error.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please fill out all sign up fields.</p>"#
                }
                Some("exists") => {
                    r#"<p class="auth-error" role="alert">An account with that email already exists.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not create your account. Please try again.</p>"#
                }
                _ => "",
            };
            let signup_info_block = match query.reason.as_deref() {
                Some("notfound") => {
                    r#"<p class="auth-success" role="status">No account found with that email. Create one below.</p>"#
                }
                _ => "",
            };
            let signup_email = escape_html_attr(query.email.as_deref().unwrap_or(""));
            let body = contents
                .replace("{{SIGNUP_INFO_BLOCK}}", signup_info_block)
                .replace("{{SIGNUP_ERROR_BLOCK}}", signup_error_block)
                .replace("{{SIGNUP_EMAIL}}", &signup_email);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load sign up page".to_string(),
        )
            .into_response(),
    }
}

async fn contact_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<ContactQuery>,
) -> impl IntoResponse {
    match fs::read_to_string("templates/contact.html").await {
        Ok(contents) => {
            let (form_name, form_email) = form_prefill(&state, &jar).await;
            let contact_success_block = match query.status.as_deref() {
                Some("sent") => {
                    r#"<p class="auth-success" role="status">Thanks! Your message was received. We will get back to you soon.</p>"#
                }
                _ => "",
            };
            let contact_error_block = match query.status.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please fill out all fields before sending your message.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not save your message. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let contact_email = escape_html(&admin_email());
            let body = contents
                .replace("{{CONTACT_SUCCESS_BLOCK}}", contact_success_block)
                .replace("{{CONTACT_ERROR_BLOCK}}", contact_error_block)
                .replace("{{CONTACT_EMAIL}}", &contact_email)
                .replace("{{FORM_NAME}}", &form_name)
                .replace("{{FORM_EMAIL}}", &form_email);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load contact page".to_string(),
        )
            .into_response(),
    }
}

async fn feedback_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<FeedbackQuery>,
) -> impl IntoResponse {
    match fs::read_to_string("templates/feedback.html").await {
        Ok(contents) => {
            let (form_name, form_email) = form_prefill(&state, &jar).await;
            let feedback_success_block = match query.status.as_deref() {
                Some("sent") => {
                    r#"<p class="auth-success" role="status">Thanks! Your feedback was sent to the WhiskerWatch team.</p>"#
                }
                _ => "",
            };
            let feedback_error_block = match query.status.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please fill out all feedback fields.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not save your feedback. Please try again.</p>"#
                }
                _ => "",
            };
            let body = contents
                .replace("{{FEEDBACK_SUCCESS_BLOCK}}", feedback_success_block)
                .replace("{{FEEDBACK_ERROR_BLOCK}}", feedback_error_block)
                .replace("{{FORM_NAME}}", &form_name)
                .replace("{{FORM_EMAIL}}", &form_email);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load feedback page".to_string(),
        )
            .into_response(),
    }
}

async fn login_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    let email = form.email.trim();
    let password = form.password.trim();

    if email.is_empty() || password.is_empty() {
        return Redirect::to("/login?error=missing").into_response();
    }

    if is_admin_credentials(email, password) {
        let jar = create_admin_session(&state, jar);
        return (jar, Redirect::to("/admin")).into_response();
    }

    if email.eq_ignore_ascii_case("demo@whiskerwatch.app") && password == "meow123" {
        return signed_in_redirect(&state, jar, email);
    }

    if user_login_valid(&state, email, password) {
        return signed_in_redirect(&state, jar, email);
    }

    if !email_exists(&state, email) {
        let encoded_email = encode_component(email);
        return Redirect::to(&format!("/signup?reason=notfound&email={encoded_email}")).into_response();
    }

    Redirect::to("/login?error=invalid").into_response()
}

fn user_login_valid(state: &AppState, email: &str, password: &str) -> bool {
    state
        .storage
        .validate_login(email, password)
        .unwrap_or(false)
}

fn email_exists(state: &AppState, email: &str) -> bool {
    if email.eq_ignore_ascii_case("demo@whiskerwatch.app")
        || email.eq_ignore_ascii_case(&admin_email())
    {
        return true;
    }

    state.storage.user_exists(email).unwrap_or(false)
}

fn save_user(state: &AppState, form: &SignupForm) -> Result<(), storage::StorageError> {
    let user = User {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        password: form.password.trim().to_string(),
        created_at: timestamp_now(),
    };

    state.storage.save_user(&user)
}

async fn signup_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<SignupForm>,
) -> Response {
    let name = form.name.trim();
    let email = form.email.trim();
    let password = form.password.trim();

    if name.is_empty() || email.is_empty() || password.is_empty() {
        return Redirect::to("/signup?error=missing").into_response();
    }

    if email_exists(&state, email) {
        return Redirect::to("/signup?error=exists").into_response();
    }

    match save_user(&state, &form) {
        Ok(()) => signed_in_redirect(&state, jar, email),
        Err(_) => Redirect::to("/signup?error=failed").into_response(),
    }
}

fn save_contact_submission(
    state: &AppState,
    form: &ContactForm,
) -> Result<(), storage::StorageError> {
    let submission = ContactSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        subject: form.subject.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
    };

    state.storage.save_contact(&submission)
}

fn save_feedback_submission(
    state: &AppState,
    form: &FeedbackForm,
) -> Result<(), storage::StorageError> {
    let submission = FeedbackSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        category: form.category.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
    };

    state.storage.save_feedback(&submission)
}

async fn contact_submit(
    State(state): State<AppState>,
    Form(form): Form<ContactForm>,
) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let subject = form.subject.trim();
    let message = form.message.trim();

    if name.is_empty() || email.is_empty() || subject.is_empty() || message.is_empty() {
        return Redirect::to("/contact?status=missing");
    }

    match save_contact_submission(&state, &form) {
        Ok(()) => Redirect::to("/contact?status=sent"),
        Err(_) => Redirect::to("/contact?status=failed"),
    }
}

async fn feedback_submit(
    State(state): State<AppState>,
    Form(form): Form<FeedbackForm>,
) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let category = form.category.trim();
    let message = form.message.trim();

    if name.is_empty() || email.is_empty() || category.is_empty() || message.is_empty() {
        return Redirect::to("/feedback?status=missing");
    }

    if !matches!(category, "fix" | "idea" | "bug") {
        return Redirect::to("/feedback?status=missing");
    }

    match save_feedback_submission(&state, &form) {
        Ok(()) => Redirect::to("/feedback?status=sent"),
        Err(_) => Redirect::to("/feedback?status=failed"),
    }
}

fn render_submission_rows(
    rows: &[(&str, &str, &str, &str, u64)],
    empty_message: &str,
) -> String {
    if rows.is_empty() {
        return format!(r#"<tr><td colspan="5">{empty_message}</td></tr>"#);
    }

    rows.iter()
        .rev()
        .map(|(kind, name, email, message, submitted_at)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(kind),
                escape_html(name),
                escape_html(email),
                escape_html(message),
                escape_html(&format_timestamp(*submitted_at)),
            )
        })
        .collect()
}

async fn admin_page(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if !admin_session_valid(&state, &jar) {
        return Redirect::to("/login").into_response();
    }

    let feedback = state.storage.load_feedback().unwrap_or_default();
    let contacts = state.storage.load_contacts().unwrap_or_default();

    let feedback_rows: Vec<(&str, &str, &str, &str, u64)> = feedback
        .iter()
        .map(|item| {
            (
                item.category.as_str(),
                item.name.as_str(),
                item.email.as_str(),
                item.message.as_str(),
                item.submitted_at,
            )
        })
        .collect();

    let contact_rows: Vec<(&str, &str, &str, &str, u64)> = contacts
        .iter()
        .map(|item| {
            (
                item.subject.as_str(),
                item.name.as_str(),
                item.email.as_str(),
                item.message.as_str(),
                item.submitted_at,
            )
        })
        .collect();

    let body = format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>WhiskerWatch Admin</title>
    <link rel="stylesheet" href="/styles.css" />
  </head>
  <body>
    <header class="topbar">
      <div class="brand" aria-label="WhiskerWatch">
        <img class="brand-logo" src="/images/logo.png" alt="WhiskerWatch" />
      </div>
      <nav>
        <a href="/">HOME</a>
        <a href="/feedback">FEEDBACK</a>
        <form class="admin-logout-form" action="/admin/logout" method="post">
          <button type="submit" class="admin-logout-btn">LOG OUT</button>
        </form>
      </nav>
    </header>
    <main class="section admin-page">
      <h1>Admin Dashboard</h1>
      <p>Review feedback, bug reports, and contact messages from testers.</p>

      <section class="admin-panel">
        <h2>Feedback and Ideas ({feedback_count})</h2>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Type</th>
              <th>Name</th>
              <th>Email</th>
              <th>Message</th>
              <th>Submitted</th>
            </tr>
          </thead>
          <tbody>
            {feedback_rows}
          </tbody>
        </table>
      </section>

      <section class="admin-panel">
        <h2>Contact Messages ({contact_count})</h2>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Subject</th>
              <th>Name</th>
              <th>Email</th>
              <th>Message</th>
              <th>Submitted</th>
            </tr>
          </thead>
          <tbody>
            {contact_rows}
          </tbody>
        </table>
      </section>
    </main>
  </body>
</html>"#,
        feedback_count = feedback.len(),
        contact_count = contacts.len(),
        feedback_rows = render_submission_rows(
            &feedback_rows,
            "No feedback submissions yet."
        ),
        contact_rows = render_submission_rows(
            &contact_rows,
            "No contact messages yet."
        ),
    );

    Html(body).into_response()
}

async fn admin_logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = clear_admin_session(&state, jar);
    (jar, Redirect::to("/login")).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile_weeks(weeks: u32, indoor: &str) -> UserProfile {
        UserProfile {
            email: "test@example.com".to_string(),
            paw_points: 0,
            parent_level: 1,
            parent_xp: 0,
            pet_name: "Mochi".to_string(),
            pet_breed: String::new(),
            pet_mood: String::new(),
            pet_emoji: "🐱".to_string(),
            equipped_outfit: String::new(),
            owned_outfits: vec![],
            onboarding_completed: true,
            pet_age_weeks: Some(weeks),
            pet_age_years: None,
            last_vet_date: None,
            pet_conditions: String::new(),
            pet_medications: String::new(),
            pet_indoor_outdoor: Some(indoor.to_string()),
            vaccine_history: vec![],
            tasks: vec![],
            calendar_events: vec![],
            activity: vec![],
        }
    }

    #[test]
    fn kitten_gets_fvrcp_and_rabies_slots() {
        let profile = test_profile_weeks(10, "indoor");
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        let events = generate_vaccine_calendar_events(&profile, today);
        let titles: Vec<String> = events.iter().map(|e| e.title.clone()).collect();
        assert!(titles.iter().any(|t| t.contains("FVRCP")));
        assert!(titles.iter().any(|t| t.contains("Rabies")));
    }

    #[test]
    fn recorded_fvrcp_skips_nearby_reminder() {
        let mut profile = test_profile_weeks(10, "indoor");
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        let birth = pet_birth_date(&profile, today).expect("birth");
        let week_10 = week_from_birth(birth, 10);
        profile.vaccine_history.push(VaccineRecord {
            vaccine_name: "FVRCP".to_string(),
            date: week_10.format("%Y-%m-%d").to_string(),
        });
        let events = generate_vaccine_calendar_events(&profile, today);
        assert!(
            !events
                .iter()
                .any(|event| event.title.contains("FVRCP") && event.day == week_10.day())
        );
    }

    #[test]
    fn outdoor_cat_gets_yearly_felv_after_year_one() {
        let profile = test_profile_weeks(52, "outdoor");
        let mut profile = profile;
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        let events = generate_vaccine_calendar_events(&profile, today);
        assert!(events.iter().any(|event| event.title.contains("FeLV")));
    }

    fn onboarding_form_body(extra: &str) -> String {
        format!(
            "cat_name=Mochi&age_value=2&age_unit=years&pet_indoor_outdoor=indoor&last_vet_date=&conditions=&medications={extra}"
        )
    }

    #[test]
    fn onboarding_form_deserializes_zero_vaccines() {
        let form: OnboardingForm =
            serde_urlencoded::from_str(&onboarding_form_body("&vaccine_names=&vaccine_dates="))
                .expect("form");
        assert!(parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates).is_empty());
    }

    #[test]
    fn onboarding_form_deserializes_single_vaccine() {
        let form: OnboardingForm = serde_urlencoded::from_str(&onboarding_form_body(
            "&vaccine_names=FVRCP&vaccine_dates=2024-01-15",
        ))
        .expect("form");
        assert_eq!(form.vaccine_names, vec!["FVRCP"]);
        assert_eq!(form.vaccine_dates, vec!["2024-01-15"]);
        let history = parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].vaccine_name, "FVRCP");
    }

    #[test]
    fn onboarding_form_deserializes_multiple_vaccines() {
        let form: OnboardingForm = serde_urlencoded::from_str(&onboarding_form_body(
            "&vaccine_names=FVRCP&vaccine_names=Rabies&vaccine_dates=2024-01-15&vaccine_dates=2024-02-20",
        ))
        .expect("form");
        assert_eq!(form.vaccine_names, vec!["FVRCP", "Rabies"]);
        assert_eq!(form.vaccine_dates, vec!["2024-01-15", "2024-02-20"]);
        let history = parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates);
        assert_eq!(history.len(), 2);
    }
}

#[tokio::main]
async fn main() {
    let storage = Storage::open().unwrap_or_else(|error| {
        panic!("failed to open storage: {error:?}");
    });
    eprintln!(
        "Using data directory: {} (database: whiskerwatch.db)",
        storage.data_dir().display()
    );

    let state = AppState {
        storage,
        admin_sessions: Arc::new(Mutex::new(HashSet::new())),
        user_sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(index_page))
        .route("/home", get(dashboard_page))
        .route("/home/onboarding", post(onboarding_submit))
        .route("/home/outfits/buy", post(outfit_buy))
        .route("/home/outfits/equip", post(outfit_equip))
        .route("/home/tasks/toggle", post(task_toggle))
        .route("/home/paw-points/buy", post(paw_points_buy))
        .route("/logout", post(user_logout))
        .route("/login", get(login_page).post(login_submit))
        .route("/signup", get(signup_page).post(signup_submit))
        .route("/contact", get(contact_page).post(contact_submit))
        .route("/feedback", get(feedback_page).post(feedback_submit))
        .route("/admin", get(admin_page))
        .route("/admin/logout", post(admin_logout))
        .route("/login.html", get(|| async { Redirect::permanent("/login") }))
        .route("/signup.html", get(|| async { Redirect::permanent("/signup") }))
        .route("/contact.html", get(|| async { Redirect::permanent("/contact") }))
        .route("/feedback.html", get(|| async { Redirect::permanent("/feedback") }))
        .nest_service("/images", ServeDir::new("static/images"))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let address = listen_address();
    let listener = TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind to {address}: {error}"));

    println!("WhiskerWatch running at http://{address}");
    println!("Admin login: {} / (see ADMIN_PASSWORD env var)", admin_email());
    axum::serve(listener, app)
        .await
        .expect("server failed unexpectedly");
}
