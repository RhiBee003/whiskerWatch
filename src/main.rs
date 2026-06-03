use axum::{
    Form, Router,
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
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
use sha2::{Digest, Sha256};
use time::Duration as CookieDuration;
use uuid::Uuid;

mod storage;
mod stripe_payments;
use storage::Storage;
use stripe_payments::CheckoutError;

const ADMIN_SESSION_COOKIE: &str = "ww_admin_session";
const USER_SESSION_COOKIE: &str = "ww_user_session";
const LOGIN_PREFILL_COOKIE: &str = "ww_login_prefill";
const LOGIN_PREFILL_MAX_AGE_SECS: i64 = 120;

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
    reset: Option<String>,
    exists: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct LoginPrefillPayload {
    email: String,
    password: String,
}

#[derive(Deserialize, Default)]
struct ForgotPasswordQuery {
    error: Option<String>,
    sent: Option<String>,
}

#[derive(Deserialize, Default)]
struct ResetPasswordQuery {
    token: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ForgotPasswordForm {
    email: String,
}

#[derive(Deserialize)]
struct ResetPasswordForm {
    token: String,
    password: String,
    confirm_password: String,
}

#[derive(Deserialize, Default)]
struct SignupQuery {
    error: Option<String>,
    email: Option<String>,
    username: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
}

#[derive(Deserialize, Default)]
struct FeedbackQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct SignupForm {
    username: String,
    first_name: String,
    last_name: String,
    email: String,
    password: String,
    confirm_password: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    username: String,
    first_name: String,
    last_name: String,
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
struct VeterinaryNote {
    date: String,
    note: String,
}

const VET_APPOINTMENT_TASK_ID: &str = "vet_appointment_asap";
const MAX_PET_PHOTO_BYTES: usize = 5 * 1024 * 1024;

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
    #[serde(default)]
    pet_color: String,
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
    never_been_to_vet: bool,
    #[serde(default)]
    veterinary_notes: Vec<VeterinaryNote>,
    #[serde(default)]
    vet_notes: Option<String>,
    #[serde(default)]
    vet_followup_pending: bool,
    #[serde(default)]
    pet_conditions: String,
    #[serde(default)]
    pet_medications: String,
    #[serde(default)]
    pet_indoor_outdoor: Option<String>,
    #[serde(default)]
    vaccine_history: Vec<VaccineRecord>,
    #[serde(default)]
    pet_vaccines_unknown: bool,
    tasks: Vec<UserTask>,
    calendar_events: Vec<CalendarEvent>,
    activity: Vec<ProfileActivity>,
    /// Stripe Customer id (`cus_...`) only—never PAN/CVV. Card data stays at Stripe.
    #[serde(default)]
    stripe_customer_id: Option<String>,
    #[serde(default)]
    pet_photo_url: Option<String>,
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
    session_id: Option<String>,
    vet_followup: Option<String>,
    thread: Option<String>,
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

fn form_optional_scalar(fields: &HashMap<String, Vec<String>>, key: &str) -> String {
    fields
        .get(key)
        .and_then(|values| values.first())
        .cloned()
        .unwrap_or_default()
}

fn form_checkbox(fields: &HashMap<String, Vec<String>>, key: &str) -> bool {
    fields
        .get(key)
        .is_some_and(|values| values.iter().any(|value| matches!(value.as_str(), "on" | "true" | "1")))
}

struct OnboardingForm {
    cat_name: String,
    pet_breed: String,
    pet_color: String,
    age_value: String,
    age_unit: String,
    pet_indoor_outdoor: String,
    last_vet_date: String,
    never_been_to_vet: bool,
    conditions: String,
    medications: String,
    vaccine_names: Vec<String>,
    vaccine_dates: Vec<String>,
    pet_vaccines_unknown: bool,
    skip_photo: bool,
}

impl OnboardingForm {
    fn from_fields<E: DeError>(fields: &HashMap<String, Vec<String>>) -> Result<Self, E> {
        Ok(OnboardingForm {
            cat_name: form_scalar(fields, "cat_name")?,
            pet_breed: form_scalar(fields, "pet_breed")?,
            pet_color: form_optional_scalar(fields, "pet_color"),
            age_value: form_scalar(fields, "age_value")?,
            age_unit: form_scalar(fields, "age_unit")?,
            pet_indoor_outdoor: form_scalar(fields, "pet_indoor_outdoor")?,
            last_vet_date: form_optional_scalar(fields, "last_vet_date"),
            never_been_to_vet: form_checkbox(fields, "never_been_to_vet"),
            conditions: form_scalar(fields, "conditions")?,
            medications: form_scalar(fields, "medications")?,
            vaccine_names: form_vec(fields, "vaccine_names"),
            vaccine_dates: form_vec(fields, "vaccine_dates"),
            pet_vaccines_unknown: form_checkbox(fields, "pet_vaccines_unknown"),
            skip_photo: form_checkbox(fields, "skip_photo"),
        })
    }
}

impl<'de> Deserialize<'de> for OnboardingForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(String, String)>::deserialize(deserializer)?;
        let fields = group_form_fields(pairs);
        Self::from_fields(&fields)
    }
}

struct VetVisitForm {
    last_vet_date: String,
    vet_note: String,
    vaccine_names: Vec<String>,
    vaccine_dates: Vec<String>,
}

impl<'de> Deserialize<'de> for VetVisitForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(String, String)>::deserialize(deserializer)?;
        let fields = group_form_fields(pairs);

        Ok(VetVisitForm {
            last_vet_date: form_optional_scalar(&fields, "last_vet_date"),
            vet_note: form_optional_scalar(&fields, "vet_note"),
            vaccine_names: form_vec(&fields, "vaccine_names"),
            vaccine_dates: form_vec(&fields, "vaccine_dates"),
        })
    }
}

#[derive(Deserialize)]
struct VetNotesForm {
    vet_notes: String,
}

#[derive(Deserialize)]
struct PawPointsCheckoutForm {
    package: String,
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
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ForumPost {
    id: i64,
    user_id: String,
    author_username: String,
    title: String,
    body: String,
    created_at: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct ForumReply {
    id: i64,
    post_id: i64,
    user_id: String,
    author_username: String,
    body: String,
    created_at: u64,
}

#[derive(Deserialize)]
struct ForumPostForm {
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct ForumReplyForm {
    post_id: String,
    body: String,
}

#[derive(Deserialize, Default)]
struct ContactQuery {
    status: Option<String>,
}

fn env_or_default(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn admin_email() -> String {
    env_or_default("ADMIN_EMAIL", "rhibee003@gmail.com")
}

fn admin_password() -> String {
    env_or_default("ADMIN_PASSWORD", "WhiskerAdmin2026!")
}

fn is_admin_account(email: &str) -> bool {
    email.eq_ignore_ascii_case(&admin_email())
}

fn listen_address() -> String {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    format!("0.0.0.0:{port}")
}

fn smtp_configured() -> bool {
    env::var("SMTP_HOST")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn show_dev_reset_links() -> bool {
    !smtp_configured()
        || env::var("SHOW_RESET_LINKS")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

fn public_base_url() -> String {
    env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| {
        let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        format!("http://localhost:{port}")
    })
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

fn encode_login_prefill_cookie_value(email: &str, password: &str) -> String {
    let payload = LoginPrefillPayload {
        email: email.to_string(),
        password: password.to_string(),
    };
    urlencoding::encode(
        &serde_json::to_string(&payload).expect("login prefill json should serialize"),
    )
    .into_owned()
}

fn decode_login_prefill_cookie_value(value: &str) -> Option<LoginPrefillPayload> {
    let decoded = urlencoding::decode(value).ok()?.into_owned();
    serde_json::from_str(&decoded).ok()
}

fn set_login_prefill_cookie(jar: CookieJar, email: &str, password: &str) -> CookieJar {
    let mut cookie = Cookie::new(
        LOGIN_PREFILL_COOKIE,
        encode_login_prefill_cookie_value(email, password),
    );
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(Some(CookieDuration::seconds(
        LOGIN_PREFILL_MAX_AGE_SECS,
    )));
    jar.add(cookie)
}

fn take_login_prefill(jar: CookieJar) -> (CookieJar, Option<LoginPrefillPayload>) {
    let Some(cookie) = jar.get(LOGIN_PREFILL_COOKIE) else {
        return (jar, None);
    };
    let payload = decode_login_prefill_cookie_value(cookie.value());
    let jar = jar.remove(Cookie::from(LOGIN_PREFILL_COOKIE));
    (jar, payload)
}

fn redirect_to_login_existing_account(jar: CookieJar, email: &str, password: &str) -> Response {
    let jar = set_login_prefill_cookie(jar, email, password);
    (jar, Redirect::to("/login?exists=1")).into_response()
}

fn signup_redirect_with_fields(
    error: &str,
    username: &str,
    first_name: &str,
    last_name: &str,
    email: &str,
) -> Redirect {
    Redirect::to(&format!(
        "/signup?error={}&username={}&first_name={}&last_name={}&email={}",
        encode_component(error),
        encode_component(username),
        encode_component(first_name),
        encode_component(last_name),
        encode_component(email),
    ))
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

fn ensure_admin_user_account(state: &AppState) -> Result<(), storage::StorageError> {
    let email = admin_email();
    if !state.storage.user_exists(&email)? {
        let user = User {
            username: "Admin".to_string(),
            first_name: "WhiskerWatch".to_string(),
            last_name: "Admin".to_string(),
            email: email.clone(),
            password: admin_password(),
            created_at: timestamp_now(),
        };
        state.storage.save_user(&user)?;
    }

    match state.storage.load_profile(&email)? {
        Some(mut profile) => {
            let mut changed = false;
            if !profile.onboarding_completed {
                profile.onboarding_completed = true;
                changed = true;
            }
            let before = profile.tasks.len();
            profile
                .tasks
                .retain(|task| task.id != VET_APPOINTMENT_TASK_ID);
            if profile.tasks.len() != before {
                changed = true;
            }
            if changed {
                state.storage.save_profile(&profile)?;
            }
        }
        None => {
            state.storage.save_profile(&admin_profile(&email))?;
        }
    }

    Ok(())
}

fn ensure_dashboard_session(state: &AppState, jar: CookieJar) -> Result<(CookieJar, String), Redirect> {
    if let Some(email) = user_session_email(state, &jar) {
        return Ok((jar, email));
    }

    if admin_session_valid(state, &jar) {
        let email = admin_email();
        if let Err(error) = ensure_admin_user_account(state) {
            eprintln!("admin user bootstrap failed: {error}");
        }
        let jar = create_user_session(state, jar, &email);
        return Ok((jar, email));
    }

    Err(Redirect::to("/login"))
}

fn admin_dashboard_nav_link(state: &AppState, jar: &CookieJar) -> &'static str {
    if admin_session_valid(state, jar) {
        r#"<a href="/admin">ADMIN</a>"#
    } else {
        ""
    }
}

fn replace_admin_nav_link(template: &str, state: &AppState, jar: &CookieJar) -> String {
    let link = admin_dashboard_nav_link(state, jar);
    template
        .replace("{{ADMIN_NAV_LINK}}", link)
        .replace("{{admin_nav_link}}", link)
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

fn auth_nav_link_html(state: &AppState, jar: &CookieJar) -> &'static str {
    if user_session_email(state, jar).is_some() {
        r#"<a href="/home?tab=account">ACCOUNT</a>"#
    } else {
        r#"<a href="/login">LOG IN</a>"#
    }
}

fn user_for_email(state: &AppState, email: &str) -> Option<User> {
    state
        .storage
        .find_user_by_email(email)
        .ok()
        .flatten()
}

fn contact_name_for_email(state: &AppState, email: &str) -> Option<String> {
    user_for_email(state, email).map(|user| {
        let full = format!("{} {}", user.first_name.trim(), user.last_name.trim()).trim().to_string();
        if full.is_empty() {
            user.username
        } else {
            full
        }
    })
}

async fn form_prefill(state: &AppState, jar: &CookieJar) -> (String, String) {
    let Some(email) = user_session_email(state, jar) else {
        return (String::new(), String::new());
    };

    let form_email = escape_html_attr(&email);
    let form_name = contact_name_for_email(state, &email)
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
            id: "water_bowl_morning".to_string(),
            title: "Fill water bowl".to_string(),
            completed: false,
            due_label: "Daily · 8:00 AM".to_string(),
            due_day: Some(today.day()),
            due_month: Some(month),
            due_year: Some(year),
            reward: 12,
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
            due_label: "Daily · anytime".to_string(),
            due_day: Some(today.day()),
            due_month: Some(month),
            due_year: Some(year),
            reward: 10,
        },
        UserTask {
            id: "replace_litter".to_string(),
            title: "Replace litter".to_string(),
            completed: false,
            due_label: "Weekly · anytime".to_string(),
            due_day: Some(today.day()),
            due_month: Some(month),
            due_year: Some(year),
            reward: 25,
        },
        UserTask {
            id: "water_bowl_night".to_string(),
            title: "Fill water bowl".to_string(),
            completed: false,
            due_label: "Daily · 9:00 PM".to_string(),
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
        pet_color: String::new(),
        pet_mood: "Waiting to meet you".to_string(),
        pet_emoji: "🐱".to_string(),
        equipped_outfit: "Classic Collar".to_string(),
        owned_outfits: vec!["classic_collar".to_string()],
        onboarding_completed: false,
        pet_age_weeks: None,
        pet_age_years: None,
        last_vet_date: None,
        never_been_to_vet: false,
        veterinary_notes: vec![],
        vet_notes: None,
        vet_followup_pending: false,
        pet_conditions: String::new(),
        pet_medications: String::new(),
        pet_indoor_outdoor: None,
        vaccine_history: vec![],
        pet_vaccines_unknown: false,
        tasks: default_starter_tasks(),
        calendar_events: vec![],
        activity: vec![],
        stripe_customer_id: None,
        pet_photo_url: None,
    }
}

fn admin_profile(email: &str) -> UserProfile {
    let mut profile = default_profile(email);
    profile.onboarding_completed = true;
    profile.tasks = vec![];
    profile.pet_name = "No pet yet".to_string();
    profile.pet_breed = String::new();
    profile.pet_mood = "Admin dashboard".to_string();
    profile
}

fn email_upload_basename(email: &str) -> String {
    let hash = Sha256::digest(email.trim().to_lowercase().as_bytes());
    hex::encode(hash)
}

fn pet_uploads_dir(state: &AppState) -> std::path::PathBuf {
    state.storage.data_dir().join("uploads")
}

fn detect_image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 3 && bytes[0..3] == [0xFF, 0xD8, 0xFF] {
        return Some("jpg");
    }
    if bytes.len() >= 8 && bytes[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some("png");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    None
}

fn allowed_pet_photo_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp"
    )
}

fn validate_pet_photo(content_type: Option<&str>, bytes: &[u8]) -> Result<&'static str, ()> {
    if bytes.is_empty() {
        return Err(());
    }
    if bytes.len() > MAX_PET_PHOTO_BYTES {
        return Err(());
    }
    let ext = detect_image_ext(bytes).ok_or(())?;
    if let Some(content_type) = content_type {
        if !allowed_pet_photo_content_type(content_type) {
            return Err(());
        }
    }
    Ok(ext)
}

async fn save_pet_photo(
    state: &AppState,
    email: &str,
    bytes: &[u8],
    ext: &str,
) -> Result<String, storage::StorageError> {
    let uploads_dir = pet_uploads_dir(state);
    fs::create_dir_all(&uploads_dir).await?;

    let basename = email_upload_basename(email);
    let filename = format!("{basename}.{ext}");
    let disk_path = uploads_dir.join(&filename);
    fs::write(&disk_path, bytes).await?;

    Ok(format!("/uploads/{filename}"))
}

fn render_pet_user_photo_optional(profile: &UserProfile) -> String {
    let Some(url) = profile.pet_photo_url.as_deref().filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let name = escape_html(&profile.pet_name);
    format!(
        r#"<div class="pet-user-photo-optional" hidden>
      <img src="{url}" alt="Photo of {name}" width="96" height="96" />
    </div>"#,
        url = escape_html_attr(url),
        name = name,
    )
}

fn render_pet_avatar(profile: &UserProfile) -> String {
    let pet_name = escape_html(&profile.pet_name);
    let display_name = if profile.pet_name.trim().is_empty() {
        "Cinder".to_string()
    } else {
        pet_name.clone()
    };
    let photo_toggle = if profile
        .pet_photo_url
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        r#"<button type="button" class="cinder-photo-toggle" aria-pressed="false">Show my cat photo</button>"#
    } else {
        ""
    };
    let user_photo = render_pet_user_photo_optional(profile);
    format!(
        r#"<div class="pet-cinder-stage" id="cinder-pet-stage" data-pet-name="{display_name}">
      <p class="cinder-pet-label">{display_name}</p>
      <div class="cinder-pet-image-wrap">
        <img class="cinder-pet-image" src="/cinderanimate.png" alt="{display_name} virtual pet" />
      </div>
      {user_photo}
      {photo_toggle}
    </div>"#,
        display_name = display_name,
        user_photo = user_photo,
        photo_toggle = photo_toggle,
    )
}

async fn save_profile(state: &AppState, profile: &UserProfile) -> Result<(), storage::StorageError> {
    state.storage.save_profile(profile)
}

async fn get_or_create_profile(state: &AppState, email: &str) -> UserProfile {
    let mut profile = if let Ok(Some(profile)) = state.storage.load_profile(email) {
        profile
    } else if is_admin_account(email) {
        admin_profile(email)
    } else {
        default_profile(email)
    };

    if is_admin_account(email) && !profile.onboarding_completed {
        profile.onboarding_completed = true;
        let _ = save_profile(state, &profile).await;
    }

    if refresh_profile_tasks(&mut profile) {
        let _ = save_profile(state, &profile).await;
    }

    profile
}

fn is_daily_task(task: &UserTask) -> bool {
    task.due_label.to_lowercase().contains("daily") || task.id == VET_APPOINTMENT_TASK_ID
}

fn vet_appointment_task(today: NaiveDate) -> UserTask {
    UserTask {
        id: VET_APPOINTMENT_TASK_ID.to_string(),
        title: "Make vet appointment ASAP".to_string(),
        completed: false,
        due_label: "Daily · urgent".to_string(),
        due_day: Some(today.day()),
        due_month: Some(today.month()),
        due_year: Some(today.year() as u32),
        reward: 30,
    }
}

fn vaccines_due_or_overdue(profile: &UserProfile, today: NaiveDate) -> bool {
    let Some(birth) = pet_birth_date(profile, today) else {
        return false;
    };

    let history = &profile.vaccine_history;

    if let Some(weeks) = profile.pet_age_weeks {
        if weeks <= 20 {
            for week in [6u32, 10, 14, 18] {
                let target = week_from_birth(birth, week);
                if target <= today && !is_dose_satisfied(VaccineKind::Fvrcp, target, history) {
                    return true;
                }
            }

            let rabies_at = week_from_birth(birth, 15);
            if rabies_at <= today && !is_dose_satisfied(VaccineKind::Rabies, rabies_at, history) {
                return true;
            }

            let felv_at = week_from_birth(birth, 8);
            if felv_at <= today && !is_dose_satisfied(VaccineKind::Felv, felv_at, history) {
                return true;
            }

            let felv_booster = latest_history_date(history, VaccineKind::Felv)
                .map(|first| first + Duration::weeks(4))
                .unwrap_or_else(|| week_from_birth(birth, 12));
            if felv_booster <= today && !is_dose_satisfied(VaccineKind::Felv, felv_booster, history) {
                return true;
            }
        }
    }

    let one_year = birth + Duration::weeks(52);

    for kind in [VaccineKind::Fvrcp, VaccineKind::Rabies] {
        let interval = Duration::days(365 * 3);
        let mut next = latest_history_date(history, kind)
            .map(|last| last + interval)
            .unwrap_or(one_year);
        while next <= today {
            if !is_dose_satisfied(kind, next, history) {
                return true;
            }
            next += interval;
        }
    }

    let felv_interval = if is_outdoor_cat(profile) {
        Duration::days(365)
    } else {
        Duration::days(365 * 3)
    };
    let mut felv_next = latest_history_date(history, VaccineKind::Felv)
        .map(|last| last + felv_interval)
        .unwrap_or(one_year);
    while felv_next <= today {
        if !is_dose_satisfied(VaccineKind::Felv, felv_next, history) {
            return true;
        }
        felv_next += felv_interval;
    }

    generate_vaccine_calendar_events(profile, today)
        .iter()
        .any(|event| {
            NaiveDate::from_ymd_opt(event.year as i32, event.month, event.day)
                .is_some_and(|date| date <= today)
        })
}

fn needs_vet_appointment_asap(profile: &UserProfile, today: NaiveDate) -> bool {
    if !profile.onboarding_completed || is_admin_account(&profile.email) {
        return false;
    }

    if profile.never_been_to_vet {
        return true;
    }

    if profile.pet_vaccines_unknown {
        return true;
    }

    if profile.last_vet_date.is_none() {
        return true;
    }

    vaccines_due_or_overdue(profile, today)
}

fn refresh_profile_tasks(profile: &mut UserProfile) -> bool {
    let today = Local::now().date_naive();
    let month = today.month();
    let year = today.year() as u32;
    let day = today.day();
    let mut changed = false;

    for task in &mut profile.tasks {
        if is_daily_task(task) {
            let stale = task.due_day != Some(day)
                || task.due_month != Some(month)
                || task.due_year != Some(year);
            if stale {
                task.completed = false;
                task.due_day = Some(day);
                task.due_month = Some(month);
                task.due_year = Some(year);
                if task.id == VET_APPOINTMENT_TASK_ID {
                    task.due_label = "Daily · urgent".to_string();
                }
                changed = true;
            }
        }
    }

    let needs_vet = needs_vet_appointment_asap(profile, today);
    let has_vet_task = profile
        .tasks
        .iter()
        .any(|task| task.id == VET_APPOINTMENT_TASK_ID);

    if needs_vet && !has_vet_task {
        profile.tasks.insert(0, vet_appointment_task(today));
        changed = true;
    } else if !needs_vet && has_vet_task {
        profile.tasks.retain(|task| task.id != VET_APPOINTMENT_TASK_ID);
        changed = true;
    }

    changed
}

pub(crate) fn push_activity(profile: &mut UserProfile, message: &str) {
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

fn pet_trait_display(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Not specified".to_string()
    } else {
        escape_html(trimmed)
    }
}

fn render_pet_meta(profile: &UserProfile) -> String {
    let breed = pet_trait_display(&profile.pet_breed);
    let color = profile.pet_color.trim();
    let color_part = if color.is_empty() {
        String::new()
    } else {
        format!(" · {}", escape_html(color))
    };
    format!(
        "{breed}{color_part} · Mood: {}",
        escape_html(&profile.pet_mood)
    )
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
        .unwrap_or_else(|| "Never".to_string());

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

    let vaccine_list = if profile.pet_vaccines_unknown {
        "Unknown — we recommend a vet visit soon to get vaccines up to date".to_string()
    } else if profile.vaccine_history.is_empty() {
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
        r#"<dl class="pet-health-dl"><dt>Breed</dt><dd>{breed}</dd><dt>Color</dt><dd>{color}</dd><dt>Age</dt><dd>{age}</dd><dt>Lifestyle</dt><dd>{lifestyle}</dd><dt>Last vet appointment</dt><dd>{last_vet}</dd><dt>Conditions</dt><dd>{conditions}</dd><dt>Medications</dt><dd>{medications}</dd><dt>Vaccine history</dt><dd>{vaccine_list}</dd></dl><p class="field-hint pet-health-tab-hint">See the <strong>Health</strong> tab for full veterinary notes and records.</p>"#,
        breed = pet_trait_display(&profile.pet_breed),
        color = pet_trait_display(&profile.pet_color),
        age = escape_html(&age_display(profile)),
        lifestyle = lifestyle,
        last_vet = last_vet,
        conditions = conditions,
        medications = medications,
        vaccine_list = vaccine_list,
    )
}

fn render_vaccine_row_html(name: &str, date: &str) -> String {
    let options = ["FVRCP", "Rabies", "FeLV", "Other"];
    let select_options: String = options
        .iter()
        .map(|option| {
            let selected = if name.eq_ignore_ascii_case(option) {
                " selected"
            } else {
                ""
            };
            format!(r#"<option value="{option}"{selected}>{option}</option>"#)
        })
        .collect();

    format!(
        r#"<div class="vaccine-row"><select name="vaccine_names" aria-label="Vaccine name"><option value="">Select vaccine</option>{select_options}</select><input name="vaccine_dates" type="date" value="{date}" aria-label="Vaccine date" /><button type="button" class="vaccine-remove-btn" aria-label="Remove vaccine row">×</button></div>"#,
        date = escape_html_attr(date),
    )
}

fn render_vet_followup_modal(profile: &UserProfile, show: bool) -> String {
    if !show || !profile.onboarding_completed {
        return String::new();
    }

    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let last_vet_value = profile
        .last_vet_date
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&today);

    let vaccine_rows = if profile.vaccine_history.is_empty() {
        render_vaccine_row_html("", "")
    } else {
        profile
            .vaccine_history
            .iter()
            .map(|record| render_vaccine_row_html(&record.vaccine_name, &record.date))
            .collect::<String>()
    };

    format!(
        r#"<div class="onboarding-backdrop" id="vet-followup-modal" role="dialog" aria-modal="true" aria-labelledby="vet-followup-title">
  <div class="onboarding-modal">
    <h2 id="vet-followup-title">Record vet visit 🏥</h2>
    <p class="onboarding-intro">Update vaccines and add notes from your appointment so your Health tab stays current.</p>
    <form class="onboarding-form login-form" action="/home/vet-visit" method="post">
      <label for="vet_last_vet_date">Last vet appointment</label>
      <input id="vet_last_vet_date" name="last_vet_date" type="date" value="{last_vet}" />

      <fieldset class="vaccine-history-fieldset">
        <legend>Vaccines given</legend>
        <p class="field-hint">Add or edit vaccines from this visit.</p>
        <div id="vet-vaccine-rows" class="vaccine-rows">
          {vaccine_rows}
        </div>
        <button type="button" class="download-btn vaccine-add-btn" id="vet-add-vaccine-row">+ Add vaccine</button>
      </fieldset>

      <label for="vet_note">Veterinary notes</label>
      <textarea id="vet_note" name="vet_note" rows="4" placeholder="Exam findings, recommendations, follow-up instructions…"></textarea>

      <button type="submit" class="download-btn login-submit">Save vet visit</button>
    </form>
  </div>
</div>"#,
        last_vet = escape_html_attr(last_vet_value),
        vaccine_rows = vaccine_rows,
    )
}

fn render_health_tab(profile: &UserProfile) -> String {
    if !profile.onboarding_completed {
        return r#"<p class="panel-intro">Complete onboarding on your first visit to see health records here.</p>"#.to_string();
    }

    let last_vet = profile
        .last_vet_date
        .as_deref()
        .map(|date| escape_html(date))
        .unwrap_or_else(|| "Never".to_string());

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

    let vaccine_list = if profile.pet_vaccines_unknown {
        "<li>Vaccine history unknown — we recommend taking your cat to the vet soon to get their vaccines up to date.</li>".to_string()
    } else if profile.vaccine_history.is_empty() {
        "<li>No vaccines recorded yet.</li>".to_string()
    } else {
        profile
            .vaccine_history
            .iter()
            .map(|record| {
                format!(
                    "<li><strong>{}</strong> — {}</li>",
                    escape_html(&record.vaccine_name),
                    escape_html(&record.date)
                )
            })
            .collect()
    };

    let notes_list = if profile.veterinary_notes.is_empty() {
        String::new()
    } else {
        profile
            .veterinary_notes
            .iter()
            .rev()
            .map(|entry| {
                format!(
                    "<li><strong>{}</strong><p>{}</p></li>",
                    escape_html(&entry.date),
                    escape_html(&entry.note)
                )
            })
            .collect()
    };

    let visit_notes_section = if notes_list.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="vet-visit-notes">
    <h3>Visit notes</h3>
    <ul class="health-notes-list">{notes_list}</ul>
  </div>"#
        )
    };

    let vet_notes_value = profile
        .vet_notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (vet_notes_display, vet_notes_label, vet_notes_placeholder, submit_label) =
        if let Some(notes) = vet_notes_value {
            (
                format!(
                    r#"<div class="vet-notes-display"><p>{}</p></div>"#,
                    escape_html(notes)
                ),
                "Edit vet notes",
                "Update allergies, medications, or instructions from your vet…",
                "Save vet notes",
            )
        } else {
            (
                r#"<p class="vet-notes-empty">No vet notes yet. Add reminders, allergies, or instructions from your vet.</p>"#
                    .to_string(),
                "Add vet notes",
                "Allergies, special care instructions, follow-up reminders…",
                "Add vet notes",
            )
        };

    let textarea_value = vet_notes_value.unwrap_or("");

    format!(
        r#"<p class="panel-intro">Health records for {pet_name} — vaccines, vet visits, and notes.</p>
<div class="health-grid">
  <article class="dashboard-card">
    <h2>Overview</h2>
    <dl class="pet-health-dl">
      <dt>Breed</dt><dd>{breed}</dd>
      <dt>Color</dt><dd>{color}</dd>
      <dt>Age</dt><dd>{age}</dd>
      <dt>Lifestyle</dt><dd>{lifestyle}</dd>
      <dt>Last vet appointment</dt><dd>{last_vet}</dd>
      <dt>Conditions</dt><dd>{conditions}</dd>
      <dt>Medications</dt><dd>{medications}</dd>
    </dl>
  </article>
  <article class="dashboard-card">
    <h2>Vaccine history</h2>
    <ul class="vaccine-history-list health-record-list">{vaccine_list}</ul>
  </article>
  <article class="dashboard-card health-notes-card">
    <h2>Vet notes</h2>
    {vet_notes_display}
    <form class="login-form vet-notes-form" action="/home/vet-notes" method="post">
      <label for="vet_notes">{vet_notes_label}</label>
      <textarea id="vet_notes" name="vet_notes" rows="5" placeholder="{vet_notes_placeholder}">{textarea_value}</textarea>
      <button type="submit" class="download-btn login-submit">{submit_label}</button>
    </form>
    {visit_notes_section}
  </article>
</div>"#,
        pet_name = escape_html(&profile.pet_name),
        breed = pet_trait_display(&profile.pet_breed),
        color = pet_trait_display(&profile.pet_color),
        age = escape_html(&age_display(profile)),
        lifestyle = lifestyle,
        last_vet = last_vet,
        conditions = conditions,
        medications = medications,
        vaccine_list = vaccine_list,
        vet_notes_display = vet_notes_display,
        vet_notes_label = vet_notes_label,
        vet_notes_placeholder = escape_html_attr(vet_notes_placeholder),
        textarea_value = escape_html(textarea_value),
        submit_label = submit_label,
        visit_notes_section = visit_notes_section,
    )
}

fn render_onboarding_modal(profile: &UserProfile) -> String {
    if profile.onboarding_completed || is_admin_account(&profile.email) {
        return String::new();
    }

    r#"<div class="onboarding-backdrop" id="onboarding-modal" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
  <div class="onboarding-modal">
    <h2 id="onboarding-title">Tell us about your cat 🐾</h2>
    <p class="onboarding-intro">We will personalize your pet tab and schedule vet and vaccine reminders on your calendar.</p>
    <form class="onboarding-form login-form" action="/home/onboarding" method="post" enctype="multipart/form-data">
      <label for="cat_name">Cat's name</label>
      <input id="cat_name" name="cat_name" type="text" placeholder="Mochi" required />

      <label for="pet_breed">Cat breed</label>
      <input id="pet_breed" name="pet_breed" type="text" list="cat-breeds" placeholder="e.g. Domestic Shorthair" required />
      <datalist id="cat-breeds">
        <option value="Domestic Shorthair" />
        <option value="Domestic Longhair" />
        <option value="Siamese" />
        <option value="Maine Coon" />
        <option value="Persian" />
        <option value="Ragdoll" />
        <option value="Bengal" />
        <option value="Mixed" />
      </datalist>

      <label for="pet_color">Cat color / markings</label>
      <input id="pet_color" name="pet_color" type="text" list="cat-colors" placeholder="e.g. tabby, black and white" />
      <datalist id="cat-colors">
        <option value="Black" />
        <option value="White" />
        <option value="Gray" />
        <option value="Orange" />
        <option value="Tabby" />
        <option value="Calico" />
        <option value="Tortoiseshell" />
        <option value="Black and white" />
      </datalist>

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

      <fieldset class="last-vet-fieldset">
        <label for="last_vet_date">Last vet appointment</label>
        <input id="last_vet_date" name="last_vet_date" type="date" value="" />
        <label class="checkbox-pill never-vet-option">
          <input type="checkbox" id="never_been_to_vet" name="never_been_to_vet" value="on" />
          Never been to the vet
        </label>
      </fieldset>
      <p class="field-hint">Pick a date if you remember their last visit, or check the box if they have never been. Future vet reminders start from today.</p>

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
            <button type="button" class="vaccine-remove-btn" aria-label="Remove vaccine row">×</button>
          </div>
        </div>
        <button type="button" class="download-btn vaccine-add-btn" id="add-vaccine-row">+ Add vaccine</button>
        <label class="checkbox-pill vaccine-unknown-option">
          <input type="checkbox" id="pet_vaccines_unknown" name="pet_vaccines_unknown" value="on" />
          I don't know my cat's vaccine history
        </label>
        <p id="vaccine-unknown-alert" class="vaccine-unknown-alert" role="alert" hidden>
          We recommend taking your cat to the vet soon to get their vaccines up to date.
        </p>
      </fieldset>

      <label for="conditions">Health conditions</label>
      <textarea id="conditions" name="conditions" rows="2" placeholder="e.g. asthma, arthritis"></textarea>

      <label for="medications">Medications</label>
      <textarea id="medications" name="medications" rows="2" placeholder="e.g. flea prevention monthly"></textarea>

      <fieldset class="pet-photo-fieldset">
        <legend>Cat profile photo</legend>
        <p class="field-hint">Add a photo of your cat for the My Pet tab. JPEG, PNG, or WebP up to 5MB.</p>
        <div class="pet-photo-upload">
          <input id="pet_photo" name="pet_photo" type="file" class="pet-photo-input" accept="image/jpeg,image/png,image/webp,.jpg,.jpeg,.png,.webp" />
          <label for="pet_photo" class="pet-photo-paw-btn" aria-label="Choose cat profile photo">
            <span class="pet-photo-paw-icon" aria-hidden="true">🐾</span>
          </label>
        </div>
        <div id="pet-photo-preview" class="pet-photo-preview" hidden aria-live="polite"></div>
        <label class="checkbox-pill skip-photo-option">
          <input type="checkbox" id="skip_photo" name="skip_photo" value="on" />
          Skip photo for now
        </label>
      </fieldset>

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

fn complete_sign_in(state: &AppState, jar: CookieJar, email: &str) -> CookieJar {
    let email = if is_admin_account(email) {
        admin_email()
    } else {
        email.to_string()
    };

    if is_admin_account(&email) {
        if let Err(error) = ensure_admin_user_account(state) {
            eprintln!("admin user bootstrap failed: {error}");
        }
        let jar = create_admin_session(state, jar);
        create_user_session(state, jar, &email)
    } else {
        create_user_session(state, jar, &email)
    }
}

fn signed_in_redirect(state: &AppState, jar: CookieJar, email: &str) -> Response {
    let jar = complete_sign_in(state, jar, email);
    (jar, Redirect::to("/home")).into_response()
}

async fn index_page(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() || admin_session_valid(&state, &jar) {
        return Redirect::to("/home").into_response();
    }

    match fs::read_to_string("static/index.html").await {
        Ok(contents) => {
            let html = contents.replace("{{AUTH_NAV_LINK}}", auth_nav_link_html(&state, &jar));
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
            r#"<p class="auth-success" role="status">Payment received! Paw points have been added to your account.</p>"#
        }
        Some("points_cancelled") => {
            r#"<p class="auth-error" role="alert">Checkout was cancelled. No charge was made.</p>"#
        }
        Some("points_checkout_failed") => {
            r#"<p class="auth-error" role="alert">Could not start checkout. Try again or contact support.</p>"#
        }
        Some("points_invalid") => {
            r#"<p class="auth-error" role="alert">That point package is not available.</p>"#
        }
        Some("payments_unconfigured") => {
            r#"<p class="auth-error" role="alert">Payments are not configured on this server yet.</p>"#
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
            r#"<p class="auth-error" role="alert">Please enter your cat's name, breed, a valid age, and whether they are indoor or outdoor.</p>"#
        }
        Some("onboarding_photo_invalid") => {
            r#"<p class="auth-error" role="alert">That photo could not be saved. Use a JPEG, PNG, or WebP image under 5MB, or skip the photo.</p>"#
        }
        Some("vet_visit_done") => {
            r#"<p class="auth-success" role="status">Vet visit saved! Vaccines and health notes updated.</p>"#
        }
        Some("vet_visit_invalid") => {
            r#"<p class="auth-error" role="alert">Could not save vet visit. Check vaccine dates and try again.</p>"#
        }
        Some("vet_notes_done") => {
            r#"<p class="auth-success" role="status">Vet notes saved.</p>"#
        }
        Some("vet_notes_invalid") => {
            r#"<p class="auth-error" role="alert">Could not save vet notes. Please try again.</p>"#
        }
        Some("feedback_sent") => {
            r#"<p class="auth-success" role="status">Thanks! Your feedback was sent to the WhiskerWatch team.</p>"#
        }
        Some("feedback_missing") => {
            r#"<p class="auth-error" role="alert">Please fill out all feedback fields.</p>"#
        }
        Some("feedback_failed") => {
            r#"<p class="auth-error" role="alert">We could not save your feedback. Please try again.</p>"#
        }
        Some("forum_post_sent") => {
            r#"<p class="auth-success" role="status">Your question was posted to the forum.</p>"#
        }
        Some("forum_reply_sent") => {
            r#"<p class="auth-success" role="status">Your reply was posted.</p>"#
        }
        Some("forum_missing") => {
            r#"<p class="auth-error" role="alert">Please enter a title and question details.</p>"#
        }
        Some("forum_reply_missing") => {
            r#"<p class="auth-error" role="alert">Please enter a reply.</p>"#
        }
        Some("forum_invalid") => {
            r#"<p class="auth-error" role="alert">That forum thread could not be found.</p>"#
        }
        Some("forum_failed") => {
            r#"<p class="auth-error" role="alert">We could not save your forum post. Please try again.</p>"#
        }
        _ => "",
    }
    .to_string()
}

fn vet_urgency_alert_message(profile: &UserProfile) -> &'static str {
    if profile.pet_vaccines_unknown {
        "We don't know your cat's vaccine history — make a vet appointment ASAP to get vaccines up to date."
    } else if profile.never_been_to_vet {
        "Make a vet appointment ASAP — we don't have a vet visit on record yet."
    } else if profile.last_vet_date.is_none() {
        "Make a vet appointment ASAP to keep your cat's health records current."
    } else {
        "Make a vet appointment ASAP — vaccines or checkups may be due."
    }
}

fn render_vet_urgency_alert(profile: &UserProfile, extra_class: &str) -> String {
    let today = Local::now().date_naive();
    if !needs_vet_appointment_asap(profile, today) {
        return String::new();
    }

    let class_suffix = if extra_class.is_empty() {
        String::new()
    } else {
        format!(" {extra_class}")
    };

    format!(
        r#"<p class="vaccine-unknown-alert{class_suffix}" role="alert">{message}</p>"#,
        class_suffix = class_suffix,
        message = vet_urgency_alert_message(profile),
    )
}

fn render_dashboard_status_area(profile: &UserProfile, status: Option<&str>) -> String {
    let mut html = dashboard_status_block(status);
    html.push_str(&render_vet_urgency_alert(profile, "dashboard-vaccine-alert"));
    html
}

fn render_dashboard_feedback_tab(form_name: &str, form_email: &str) -> String {
    format!(
        r#"<h1>Share Feedback</h1>
        <p class="panel-intro">Tell us what to fix, what to improve, or share a new idea for WhiskerWatch.</p>
        <article class="dashboard-card">
          <form class="login-form contact-form" action="/feedback" method="post">
            <label for="feedback-name">Name</label>
            <input id="feedback-name" name="name" type="text" placeholder="Your name" value="{form_name}" required />

            <label for="feedback-email">Email</label>
            <input id="feedback-email" name="email" type="email" placeholder="you@example.com" value="{form_email}" required />

            <label for="feedback-category">Type</label>
            <select id="feedback-category" name="category" required>
              <option value="">Choose one...</option>
              <option value="fix">Something to fix</option>
              <option value="idea">New idea</option>
              <option value="bug">Bug report</option>
            </select>

            <label for="feedback-message">Details</label>
            <textarea
              id="feedback-message"
              name="message"
              rows="5"
              placeholder="What should we change or add?"
              required
            ></textarea>

            <button type="submit" class="download-btn login-submit">Send Feedback</button>
          </form>
        </article>"#,
        form_name = form_name,
        form_email = form_email,
    )
}

fn render_forum_reply(reply: &ForumReply) -> String {
    format!(
        r#"<li class="forum-reply">
          <p class="forum-reply-meta"><strong>{author}</strong> · {when}</p>
          <p class="forum-reply-body">{body}</p>
        </li>"#,
        author = escape_html(&reply.author_username),
        when = escape_html(&format_timestamp(reply.created_at)),
        body = escape_html(&reply.body),
    )
}

fn render_forum_thread(
    post: &ForumPost,
    replies: &[ForumReply],
    reply_count: u32,
    open: bool,
) -> String {
    let open_attr = if open { " open" } else { "" };
    let answer_label = if reply_count == 1 {
        "1 answer".to_string()
    } else {
        format!("{reply_count} answers")
    };
    let replies_html: String = replies.iter().map(render_forum_reply).collect();
    let replies_block = if replies.is_empty() {
        r#"<p class="forum-no-replies">No answers yet — be the first to help!</p>"#.to_string()
    } else {
        format!(r#"<ul class="forum-replies">{replies_html}</ul>"#, replies_html = replies_html)
    };

    format!(
        r#"<details class="forum-thread"{open_attr} data-post-id="{id}">
          <summary class="forum-thread-summary">
            <span class="forum-thread-title">{title}</span>
            <span class="forum-thread-meta">by {author} · {when} · {answers}</span>
          </summary>
          <div class="forum-thread-body">
            <p>{body}</p>
            {replies_block}
            <form class="login-form forum-reply-form" action="/home/forum/reply" method="post">
              <input type="hidden" name="post_id" value="{id}" />
              <label for="forum-reply-{id}">Your answer</label>
              <textarea id="forum-reply-{id}" name="body" rows="3" placeholder="Share advice or your experience..." required></textarea>
              <button type="submit" class="download-btn login-submit">Post reply</button>
            </form>
          </div>
        </details>"#,
        open_attr = open_attr,
        id = post.id,
        title = escape_html(&post.title),
        author = escape_html(&post.author_username),
        when = escape_html(&format_timestamp(post.created_at)),
        answers = escape_html(&answer_label),
        body = escape_html(&post.body),
        replies_block = replies_block,
    )
}

fn render_dashboard_forum_tab(state: &AppState, open_thread: Option<i64>) -> String {
    let posts = state.storage.list_forum_posts().unwrap_or_default();
    let mut threads = String::new();

    if posts.is_empty() {
        threads.push_str(
            r#"<p class="forum-empty">No questions yet. Ask the community about your pet's care!</p>"#,
        );
    } else {
        for post in &posts {
            let replies = state
                .storage
                .list_forum_replies(post.id)
                .unwrap_or_default();
            let reply_count = state
                .storage
                .count_forum_replies(post.id)
                .unwrap_or(replies.len() as u32);
            let open = open_thread.is_some_and(|id| id == post.id);
            threads.push_str(&render_forum_thread(post, &replies, reply_count, open));
        }
    }

    format!(
        r#"<h1>Pet Q&amp;A Forum</h1>
        <p class="panel-intro">Ask questions and share answers about cat care with other WhiskerWatch parents.</p>
        <article class="dashboard-card forum-ask-card">
          <h2>Ask a question</h2>
          <form class="login-form forum-ask-form" action="/home/forum/post" method="post">
            <label for="forum-title">Question title</label>
            <input id="forum-title" name="title" type="text" placeholder="e.g. How often should I brush my cat?" required maxlength="200" />

            <label for="forum-body">Details</label>
            <textarea id="forum-body" name="body" rows="4" placeholder="Tell us more about your pet and what you need help with..." required maxlength="4000"></textarea>

            <button type="submit" class="download-btn login-submit">Post question</button>
          </form>
        </article>
        <div class="forum-list">{threads}</div>"#,
        threads = threads,
    )
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
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    if let Some(session_id) = query.session_id.as_deref() {
        if !session_id.is_empty() {
            let _ = stripe_payments::fulfill_checkout_session(&state, session_id).await;
        }
    }

    let profile = get_or_create_profile(&state, &email).await;
    let show_vet_followup = profile.vet_followup_pending
        || query.vet_followup.as_deref().is_some_and(|value| value == "1");
    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let first_name = user
        .as_ref()
        .map(|u| u.first_name.clone())
        .unwrap_or_default();
    let last_name = user
        .as_ref()
        .map(|u| u.last_name.clone())
        .unwrap_or_default();
    let (level_progress_pct, level_progress_text) = level_progress(&profile);
    let calendar_month = current_calendar_month();
    let calendar_year = current_calendar_year();
    let (form_name, form_email) = form_prefill(&state, &jar).await;

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
        .replace("{{USER_NAME}}", &escape_html(&username))
        .replace("{{USER_FIRST_NAME}}", &escape_html(&first_name))
        .replace("{{USER_LAST_NAME}}", &escape_html(&last_name))
        .replace("{{USER_USERNAME}}", &escape_html(&username))
        .replace("{{USER_EMAIL}}", &escape_html(&email))
        .replace("{{MEMBER_SINCE}}", &escape_html(&member_since_label(&state, &email).await))
        .replace("{{PAW_POINTS}}", &profile.paw_points.to_string())
        .replace("{{PARENT_LEVEL}}", &profile.parent_level.to_string())
        .replace("{{LEVEL_PROGRESS}}", &level_progress_pct.to_string())
        .replace("{{LEVEL_PROGRESS_TEXT}}", &escape_html(&level_progress_text))
        .replace("{{PET_NAME}}", &escape_html(&profile.pet_name))
        .replace("{{PET_META}}", &render_pet_meta(&profile))
        .replace("{{PET_AVATAR}}", &render_pet_avatar(&profile))
        .replace("{{PET_HEALTH_INFO}}", &render_pet_health_info(&profile))
        .replace(
            "{{PET_VET_ALERT}}",
            &render_vet_urgency_alert(&profile, "pet-tab-vet-alert"),
        )
        .replace(
            "{{CALENDAR_VET_ALERT}}",
            &render_vet_urgency_alert(&profile, "calendar-tab-vet-alert"),
        )
        .replace("{{ONBOARDING_MODAL}}", &render_onboarding_modal(&profile))
        .replace(
            "{{VET_FOLLOWUP_MODAL}}",
            &render_vet_followup_modal(&profile, show_vet_followup),
        )
        .replace("{{HEALTH_TAB_CONTENT}}", &render_health_tab(&profile))
        .replace("{{EQUIPPED_OUTFIT}}", &escape_html(&profile.equipped_outfit))
        .replace("{{STATUS_BLOCK}}", &render_dashboard_status_area(&profile, query.status.as_deref()))
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
        )
        .replace("{{BUY_POINTS_SECTION}}", &stripe_payments::render_buy_points_section())
        .replace(
            "{{SAVED_PAYMENT_METHODS}}",
            &stripe_payments::render_saved_payment_methods(&state, &profile).await,
        )
        .replace(
            "{{FEEDBACK_TAB_CONTENT}}",
            &render_dashboard_feedback_tab(&form_name, &form_email),
        );
    let open_thread = query
        .thread
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok());
    let body = body.replace(
        "{{FORUM_TAB_CONTENT}}",
        &render_dashboard_forum_tab(&state, open_thread),
    );
    let body = replace_admin_nav_link(&body, &state, &jar);

    (jar, Html(body)).into_response()
}

async fn onboarding_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut fields: HashMap<String, Vec<String>> = HashMap::new();
    let mut photo_bytes: Option<Vec<u8>> = None;
    let mut photo_content_type: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "pet_photo" {
            photo_content_type = field.content_type().map(str::to_string);
            match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => photo_bytes = Some(bytes.to_vec()),
                Ok(_) => {}
                Err(_) => return Redirect::to("/home?status=onboarding_photo_invalid"),
            }
            continue;
        }

        match field.text().await {
            Ok(text) => fields.entry(name).or_default().push(text),
            Err(_) => return Redirect::to("/home?status=onboarding_invalid"),
        }
    }

    let form = match OnboardingForm::from_fields::<serde::de::value::Error>(&fields) {
        Ok(form) => form,
        Err(_) => return Redirect::to("/home?status=onboarding_invalid"),
    };

    let cat_name = form.cat_name.trim();
    if cat_name.is_empty() {
        return Redirect::to("/home?status=onboarding_invalid");
    }

    let pet_breed = form.pet_breed.trim();
    if pet_breed.is_empty() {
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

    let vaccine_history = if form.pet_vaccines_unknown {
        vec![]
    } else {
        parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates)
    };

    let last_vet_date = if form.never_been_to_vet {
        None
    } else {
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
    profile.pet_breed = pet_breed.to_string();
    profile.pet_color = form.pet_color.trim().to_string();
    profile.pet_mood = "Happy".to_string();
    profile.pet_age_weeks = pet_age_weeks;
    profile.pet_age_years = pet_age_years;
    profile.never_been_to_vet = form.never_been_to_vet;
    profile.last_vet_date = last_vet_date;
    profile.pet_conditions = form.conditions.trim().to_string();
    profile.pet_medications = form.medications.trim().to_string();
    profile.pet_indoor_outdoor = Some(indoor_outdoor);
    profile.vaccine_history = vaccine_history;
    profile.pet_vaccines_unknown = form.pet_vaccines_unknown;
    profile.onboarding_completed = true;
    profile.calendar_events = merge_calendar_events(&profile, signup_date);
    let _ = refresh_profile_tasks(&mut profile);

    if !form.skip_photo {
        if let Some(bytes) = photo_bytes {
            let ext = match validate_pet_photo(photo_content_type.as_deref(), &bytes) {
                Ok(ext) => ext,
                Err(()) => return Redirect::to("/home?status=onboarding_photo_invalid"),
            };
            match save_pet_photo(&state, &email, &bytes, ext).await {
                Ok(url) => profile.pet_photo_url = Some(url),
                Err(_) => return Redirect::to("/home?status=onboarding_photo_invalid"),
            }
        }
    }

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

    let is_vet_task = task_id == VET_APPOINTMENT_TASK_ID;
    if is_vet_task {
        profile.vet_followup_pending = true;
    }

    match save_profile(&state, &profile).await {
        Ok(()) if is_vet_task => {
            Redirect::to("/home?tab=tasks&vet_followup=1&status=task_done")
        }
        Ok(()) => Redirect::to("/home?tab=tasks&status=task_done"),
        Err(_) => Redirect::to("/home?tab=tasks&status=task_invalid"),
    }
}

async fn vet_visit_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<VetVisitForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    let vaccine_history = parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates);

    let last_vet_date = {
        let trimmed = form.last_vet_date.trim();
        if trimmed.is_empty() {
            None
        } else if parse_vet_date(trimmed).is_some() {
            Some(trimmed.to_string())
        } else {
            return Redirect::to("/home?tab=health&status=vet_visit_invalid");
        }
    };

    let note_text = form.vet_note.trim();
    if !note_text.is_empty() {
        let note_date = last_vet_date
            .clone()
            .unwrap_or_else(|| Local::now().date_naive().format("%Y-%m-%d").to_string());
        profile.veterinary_notes.push(VeterinaryNote {
            date: note_date,
            note: note_text.to_string(),
        });
    }

    profile.vaccine_history = vaccine_history;
    if !profile.vaccine_history.is_empty() {
        profile.pet_vaccines_unknown = false;
    }
    if last_vet_date.is_some() {
        profile.last_vet_date = last_vet_date;
        profile.never_been_to_vet = false;
    }
    profile.vet_followup_pending = false;

    let today = Local::now().date_naive();
    profile.calendar_events = merge_calendar_events(&profile, today);
    let _ = refresh_profile_tasks(&mut profile);

    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Recorded a vet visit for {pet_name} and updated health records."),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=health&status=vet_visit_done"),
        Err(_) => Redirect::to("/home?tab=health&status=vet_visit_invalid"),
    }
}

async fn vet_notes_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<VetNotesForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if !profile.onboarding_completed {
        return Redirect::to("/home?tab=health&status=vet_notes_invalid");
    }

    let trimmed = form.vet_notes.trim();
    profile.vet_notes = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };

    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Updated vet notes for {pet_name}."),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=health&status=vet_notes_done"),
        Err(_) => Redirect::to("/home?tab=health&status=vet_notes_invalid"),
    }
}

async fn paw_points_checkout(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<PawPointsCheckoutForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    if !stripe_payments::stripe_checkout_enabled() {
        return Redirect::to("/home?tab=account&status=payments_unconfigured");
    }

    let package = match stripe_payments::package_by_id(form.package.trim()) {
        Some(package) => package,
        None => return Redirect::to("/home?tab=account&status=points_invalid"),
    };

    match stripe_payments::create_checkout_session(&state, &email, package).await {
        Ok(url) => Redirect::temporary(&url),
        Err(CheckoutError::NotConfigured) => {
            Redirect::to("/home?tab=account&status=payments_unconfigured")
        }
        Err(_) => Redirect::to("/home?tab=account&status=points_checkout_failed"),
    }
}

async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let secret = match stripe_payments::stripe_webhook_secret() {
        Some(secret) => secret,
        None => return StatusCode::SERVICE_UNAVAILABLE,
    };

    let signature = match headers.get("stripe-signature").and_then(|v| v.to_str().ok()) {
        Some(sig) => sig,
        None => return StatusCode::BAD_REQUEST,
    };

    if !stripe_payments::verify_webhook_signature(&body, signature, &secret) {
        return StatusCode::BAD_REQUEST;
    }

    match stripe_payments::handle_webhook_payload(&state, &body).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn user_logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = clear_user_session(&state, jar);
    let jar = if admin_session_valid(&state, &jar) {
        clear_admin_session(&state, jar)
    } else {
        jar
    };
    (jar, Redirect::to("/")).into_response()
}

async fn login_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() || admin_session_valid(&state, &jar) {
        return Redirect::to("/home").into_response();
    }

    match fs::read_to_string("templates/login.html").await {
        Ok(contents) => {
            let login_error_block = match query.error.as_deref() {
                Some("admin_invalid") => {
                    r#"<p class="auth-error" role="alert">Incorrect password for the admin account. Use the <code>ADMIN_PASSWORD</code> from your server environment (Render → Environment tab in production). Locally, the default is <code>WhiskerAdmin2026!</code> unless you set <code>ADMIN_PASSWORD</code>.</p>"#
                }
                Some("invalid") => {
                    r#"<p class="auth-error" role="alert">Incorrect password. Please try again.</p>"#
                }
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please enter both email and password.</p>"#
                }
                Some("storage") => {
                    r#"<p class="auth-error" role="alert">We could not verify your account right now. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let signup_success_block = match query.signup.as_deref() {
                Some("created") => r#"<p class="auth-success" role="status">Account created! You can log in with your new email and password.</p>"#,
                _ => "",
            };
            let reset_success_block = match query.reset.as_deref() {
                Some("success") => {
                    r#"<p class="auth-success" role="status">Your password was updated. You can log in with your new password.</p>"#
                }
                _ => "",
            };
            let (jar, prefill) = take_login_prefill(jar);
            let (prefill_email, prefill_password) = prefill
                .map(|payload| (payload.email, payload.password))
                .unwrap_or_default();
            let account_exists_block = if query.exists.as_deref() == Some("1")
                || !prefill_email.is_empty()
            {
                r#"<p class="auth-success" role="status">An account with this email already exists. Log in below.</p>"#
            } else {
                ""
            };
            let body = contents
                .replace("{{LOGIN_ERROR_BLOCK}}", login_error_block)
                .replace("{{SIGNUP_SUCCESS_BLOCK}}", signup_success_block)
                .replace("{{RESET_SUCCESS_BLOCK}}", reset_success_block)
                .replace("{{ACCOUNT_EXISTS_BLOCK}}", account_exists_block)
                .replace("{{LOGIN_EMAIL}}", &escape_html_attr(&prefill_email))
                .replace("{{LOGIN_PASSWORD}}", &escape_html_attr(&prefill_password));
            (jar, Html(body)).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load login page".to_string(),
        )
            .into_response(),
    }
}

async fn forgot_password_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<ForgotPasswordQuery>,
) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() {
        return Redirect::to("/home").into_response();
    }

    match fs::read_to_string("templates/forgot-password.html").await {
        Ok(contents) => {
            let forgot_error_block = match query.error.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please enter your email address.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not process your request right now. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let forgot_success_block = match query.sent.as_deref() {
                Some("1") => r#"<p class="auth-success" role="status">If an account exists for that email, password reset instructions have been sent.</p>"#,
                _ => "",
            };
            let body = contents
                .replace("{{FORGOT_ERROR_BLOCK}}", forgot_error_block)
                .replace("{{FORGOT_SUCCESS_BLOCK}}", forgot_success_block)
                .replace("{{DEV_RESET_LINK_BLOCK}}", "");
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load forgot password page".to_string(),
        )
            .into_response(),
    }
}

fn render_forgot_password_sent(dev_reset_link_block: &str) -> Response {
    match std::fs::read_to_string("templates/forgot-password.html") {
        Ok(contents) => {
            let body = contents
                .replace("{{FORGOT_ERROR_BLOCK}}", "")
                .replace(
                    "{{FORGOT_SUCCESS_BLOCK}}",
                    r#"<p class="auth-success" role="status">If an account exists for that email, password reset instructions have been sent.</p>"#,
                )
                .replace("{{DEV_RESET_LINK_BLOCK}}", dev_reset_link_block);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load forgot password page".to_string(),
        )
            .into_response(),
    }
}

async fn forgot_password_submit(
    State(state): State<AppState>,
    Form(form): Form<ForgotPasswordForm>,
) -> Response {
    let email = form.email.trim();
    if email.is_empty() {
        return Redirect::to("/forgot-password?error=missing").into_response();
    }

    let mut dev_reset_link_block = String::new();

    if email_exists(&state, email) {
        match state.storage.create_password_reset_token(email) {
            Ok(token) => {
                let reset_path = format!("/reset-password?token={}", encode_component(&token));
                let reset_url = format!("{}{}", public_base_url(), reset_path);
                eprintln!("Password reset link for {email}: {reset_url}");

                if smtp_configured() {
                    // Email delivery would be wired here when SMTP is configured.
                    eprintln!(
                        "SMTP is configured but password reset email delivery is not implemented yet."
                    );
                }

                if show_dev_reset_links() {
                    dev_reset_link_block = format!(
                        r#"<div class="dev-reset-notice" role="note">
  <p><strong>Email not configured.</strong> Use this link to reset your password (valid for 1 hour):</p>
  <p><a href="{path}">{url}</a></p>
</div>"#,
                        path = escape_html_attr(&reset_path),
                        url = escape_html(&reset_url),
                    );
                }
            }
            Err(error) => {
                eprintln!("password reset token creation failed for {email}: {error}");
                return Redirect::to("/forgot-password?error=failed").into_response();
            }
        }
    }

    render_forgot_password_sent(&dev_reset_link_block)
}

async fn reset_password_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<ResetPasswordQuery>,
) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() {
        return Redirect::to("/home").into_response();
    }

    let token = query.token.as_deref().unwrap_or("").trim();
    if token.is_empty() {
        return Redirect::to("/forgot-password").into_response();
    }

    let token_valid = state
        .storage
        .find_valid_reset_token(token)
        .unwrap_or(None)
        .is_some();

    if !token_valid {
        return Redirect::to("/forgot-password?error=failed").into_response();
    }

    match fs::read_to_string("templates/reset-password.html").await {
        Ok(contents) => {
            let reset_error_block = match query.error.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please enter and confirm your new password.</p>"#
                }
                Some("password") => {
                    r#"<p class="auth-error" role="alert">Password must be at least 5 characters and include a number and a special character.</p>"#
                }
                Some("password_mismatch") => {
                    r#"<p class="auth-error" role="alert">Passwords do not match. Please re-enter your password and try again.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">This reset link is invalid or has expired. Please request a new one.</p>"#
                }
                _ => "",
            };
            let escaped_token = escape_html_attr(token);
            let body = contents
                .replace("{{RESET_ERROR_BLOCK}}", reset_error_block)
                .replace("{{RESET_TOKEN}}", &escaped_token);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load reset password page".to_string(),
        )
            .into_response(),
    }
}

async fn reset_password_submit(
    State(state): State<AppState>,
    Form(form): Form<ResetPasswordForm>,
) -> Response {
    let token = form.token.trim();
    let password = form.password.trim();
    let confirm_password = form.confirm_password.trim();

    if token.is_empty() || password.is_empty() || confirm_password.is_empty() {
        return Redirect::to(&format!(
            "/reset-password?token={}&error=missing",
            encode_component(token)
        ))
        .into_response();
    }

    if !signup_passwords_match(password, confirm_password) {
        return Redirect::to(&format!(
            "/reset-password?token={}&error=password_mismatch",
            encode_component(token)
        ))
        .into_response();
    }

    if !password_meets_signup_requirements(password) {
        return Redirect::to(&format!(
            "/reset-password?token={}&error=password",
            encode_component(token)
        ))
        .into_response();
    }

    match state.storage.reset_password_with_token(token, password) {
        Ok(()) => Redirect::to("/login?reset=success").into_response(),
        Err(error) => {
            eprintln!("password reset failed: {error}");
            Redirect::to(&format!(
                "/reset-password?token={}&error=failed",
                encode_component(token)
            ))
            .into_response()
        }
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
                Some("email_exists") | Some("exists") => {
                    r#"<p class="auth-error" role="alert">An account with that email already exists. <a href="/login">Log in</a> instead.</p>"#
                }
                Some("username") => {
                    r#"<p class="auth-error" role="alert">That username is already taken. Please choose another.</p>"#
                }
                Some("password") => {
                    r#"<p class="auth-error" role="alert">Password must be at least 5 characters and include a number and a special character.</p>"#
                }
                Some("password_mismatch") => {
                    r#"<p class="auth-error" role="alert">Passwords do not match. Please re-enter your password and try again.</p>"#
                }
                Some("no_account") => {
                    r#"<p class="auth-error" role="alert">You don't have an account yet. Create one below.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not create your account. Please try again.</p>"#
                }
                _ => "",
            };
            let signup_info_block = "";
            let signup_email = escape_html_attr(query.email.as_deref().unwrap_or(""));
            let signup_username = escape_html_attr(query.username.as_deref().unwrap_or(""));
            let signup_first_name = escape_html_attr(query.first_name.as_deref().unwrap_or(""));
            let signup_last_name = escape_html_attr(query.last_name.as_deref().unwrap_or(""));
            let body = contents
                .replace("{{SIGNUP_INFO_BLOCK}}", signup_info_block)
                .replace("{{SIGNUP_ERROR_BLOCK}}", signup_error_block)
                .replace("{{SIGNUP_EMAIL}}", &signup_email)
                .replace("{{SIGNUP_USERNAME}}", &signup_username)
                .replace("{{SIGNUP_FIRST_NAME}}", &signup_first_name)
                .replace("{{SIGNUP_LAST_NAME}}", &signup_last_name);
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
                .replace("{{AUTH_NAV_LINK}}", auth_nav_link_html(&state, &jar))
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
                .replace("{{AUTH_NAV_LINK}}", auth_nav_link_html(&state, &jar))
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
        if let Err(error) = ensure_admin_user_account(&state) {
            eprintln!("admin user bootstrap failed: {error}");
        }
        if let Err(error) = state
            .storage
            .set_user_password(&admin_email(), &admin_password())
        {
            eprintln!("admin password sync failed: {error}");
        }
        return signed_in_redirect(&state, jar, &admin_email());
    }

    if email.eq_ignore_ascii_case("demo@whiskerwatch.app") && password == "meow123" {
        return signed_in_redirect(&state, jar, email);
    }

    match user_login_valid(&state, email, password) {
        LoginCheck::Valid => return signed_in_redirect(&state, jar, email),
        LoginCheck::StorageError => {
            return Redirect::to("/login?error=storage").into_response();
        }
        LoginCheck::Invalid => {}
    }

    match email_exists_result(&state, email) {
        Ok(false) => {
            let encoded_email = encode_component(email);
            return Redirect::to(&format!("/signup?error=no_account&email={encoded_email}"))
                .into_response();
        }
        Ok(true) => {
            if is_admin_account(email) {
                Redirect::to("/login?error=admin_invalid").into_response()
            } else {
                Redirect::to("/login?error=invalid").into_response()
            }
        }
        Err(()) => Redirect::to("/login?error=storage").into_response(),
    }
}

enum LoginCheck {
    Valid,
    Invalid,
    StorageError,
}

fn user_login_valid(state: &AppState, email: &str, password: &str) -> LoginCheck {
    match state.storage.validate_login(email, password) {
        Ok(true) => LoginCheck::Valid,
        Ok(false) => LoginCheck::Invalid,
        Err(error) => {
            eprintln!("login validation failed for {email}: {error}");
            LoginCheck::StorageError
        }
    }
}

fn email_exists_result(state: &AppState, email: &str) -> Result<bool, ()> {
    if email.eq_ignore_ascii_case("demo@whiskerwatch.app")
        || email.eq_ignore_ascii_case(&admin_email())
    {
        return Ok(true);
    }

    state.storage.user_exists(email).map_err(|error| {
        eprintln!("user_exists check failed for {email}: {error}");
    })
}

fn email_exists(state: &AppState, email: &str) -> bool {
    email_exists_result(state, email).unwrap_or(false)
}

fn password_meets_signup_requirements(password: &str) -> bool {
    if password.len() < 5 {
        return false;
    }
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());
    has_digit && has_special
}

fn signup_passwords_match(password: &str, confirm_password: &str) -> bool {
    password == confirm_password
}

fn save_user(state: &AppState, form: &SignupForm) -> Result<(), storage::StorageError> {
    let user = User {
        username: form.username.trim().to_string(),
        first_name: form.first_name.trim().to_string(),
        last_name: form.last_name.trim().to_string(),
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
    let username = form.username.trim();
    let first_name = form.first_name.trim();
    let last_name = form.last_name.trim();
    let email = form.email.trim();
    let password = form.password.trim();
    let confirm_password = form.confirm_password.trim();

    if username.is_empty()
        || first_name.is_empty()
        || last_name.is_empty()
        || email.is_empty()
        || password.is_empty()
        || confirm_password.is_empty()
    {
        return Redirect::to("/signup?error=missing").into_response();
    }

    if !signup_passwords_match(password, confirm_password) {
        return signup_redirect_with_fields("password_mismatch", username, first_name, last_name, email)
            .into_response();
    }

    if !password_meets_signup_requirements(password) {
        return signup_redirect_with_fields("password", username, first_name, last_name, email)
            .into_response();
    }

    if email_exists(&state, email) {
        return redirect_to_login_existing_account(jar, email, password);
    }

    if state.storage.username_exists(username).unwrap_or(false) {
        if email_exists(&state, email) {
            return redirect_to_login_existing_account(jar, email, password);
        }
        return Redirect::to("/signup?error=username").into_response();
    }

    match save_user(&state, &form) {
        Ok(()) => Redirect::to("/login?signup=created").into_response(),
        Err(storage::StorageError::EmailTaken) => {
            redirect_to_login_existing_account(jar, email, password)
        }
        Err(storage::StorageError::UsernameTaken) => {
            if email_exists(&state, email) {
                redirect_to_login_existing_account(jar, email, password)
            } else {
                Redirect::to("/signup?error=username").into_response()
            }
        }
        Err(error) => {
            eprintln!("signup failed for {email}: {error}");
            Redirect::to("/signup?error=failed").into_response()
        }
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
    user_id: Option<&str>,
) -> Result<(), storage::StorageError> {
    let submission = FeedbackSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        category: form.category.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
        user_id: user_id.map(str::to_string),
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
    jar: CookieJar,
    Form(form): Form<FeedbackForm>,
) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let category = form.category.trim();
    let message = form.message.trim();
    let from_dashboard = user_session_email(&state, &jar).is_some();

    if name.is_empty() || email.is_empty() || category.is_empty() || message.is_empty() {
        return if from_dashboard {
            Redirect::to("/home?tab=feedback&status=feedback_missing")
        } else {
            Redirect::to("/feedback?status=missing")
        };
    }

    if !matches!(category, "fix" | "idea" | "bug") {
        return if from_dashboard {
            Redirect::to("/home?tab=feedback&status=feedback_missing")
        } else {
            Redirect::to("/feedback?status=missing")
        };
    }

    let user_id = user_session_email(&state, &jar);
    match save_feedback_submission(&state, &form, user_id.as_deref()) {
        Ok(()) => {
            if from_dashboard {
                Redirect::to("/home?tab=feedback&status=feedback_sent")
            } else {
                Redirect::to("/feedback?status=sent")
            }
        }
        Err(_) => {
            if from_dashboard {
                Redirect::to("/home?tab=feedback&status=feedback_failed")
            } else {
                Redirect::to("/feedback?status=failed")
            }
        }
    }
}

async fn forum_post_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ForumPostForm>,
) -> Response {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let title = form.title.trim();
    let body = form.body.trim();
    if title.is_empty() || body.is_empty() {
        return Redirect::to("/home?tab=forum&status=forum_missing").into_response();
    }

    let username = user_for_email(&state, &email)
        .map(|user| user.username)
        .unwrap_or_else(|| "Parent".to_string());

    match state.storage.create_forum_post(
        &email,
        &username,
        title,
        body,
        timestamp_now(),
    ) {
        Ok(post_id) => {
            let url = format!("/home?tab=forum&thread={post_id}&status=forum_post_sent");
            Redirect::temporary(&url).into_response()
        }
        Err(error) => {
            eprintln!("forum post failed for {email}: {error}");
            Redirect::to("/home?tab=forum&status=forum_failed").into_response()
        }
    }
}

async fn forum_reply_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ForumReplyForm>,
) -> Response {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let body = form.body.trim();
    let post_id: i64 = match form.post_id.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => return Redirect::to("/home?tab=forum&status=forum_invalid").into_response(),
    };

    if body.is_empty() {
        let url = format!("/home?tab=forum&thread={post_id}&status=forum_reply_missing");
        return Redirect::temporary(&url).into_response();
    }

    if state.storage.get_forum_post(post_id).ok().flatten().is_none() {
        return Redirect::to("/home?tab=forum&status=forum_invalid").into_response();
    }

    let username = user_for_email(&state, &email)
        .map(|user| user.username)
        .unwrap_or_else(|| "Parent".to_string());

    match state.storage.create_forum_reply(
        post_id,
        &email,
        &username,
        body,
        timestamp_now(),
    ) {
        Ok(()) => {
            let url = format!("/home?tab=forum&thread={post_id}&status=forum_reply_sent");
            Redirect::temporary(&url).into_response()
        }
        Err(error) => {
            eprintln!("forum reply failed for {email}: {error}");
            let url = format!("/home?tab=forum&thread={post_id}&status=forum_failed");
            Redirect::temporary(&url).into_response()
        }
    }
}

async fn forum_thread_redirect(Path(post_id): Path<i64>) -> Response {
    let url = format!("/home?tab=forum&thread={post_id}");
    Redirect::temporary(&url).into_response()
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

fn render_feedback_rows(feedback: &[FeedbackSubmission], empty_message: &str) -> String {
    if feedback.is_empty() {
        return format!(r#"<tr><td colspan="6">{empty_message}</td></tr>"#);
    }

    feedback
        .iter()
        .rev()
        .map(|item| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&item.category),
                escape_html(&item.name),
                escape_html(&item.email),
                escape_html(item.user_id.as_deref().unwrap_or("—")),
                escape_html(&item.message),
                escape_html(&format_timestamp(item.submitted_at)),
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
        <a href="/home">HOME</a>
        <a href="/home?tab=feedback">FEEDBACK</a>
        <form class="admin-logout-form" action="/admin/logout" method="post">
          <button type="submit" class="admin-logout-btn">LOG OUT</button>
        </form>
      </nav>
    </header>
    <main class="section admin-page">
      <h1>Admin Dashboard</h1>
      <p>Review feedback, bug reports, and contact messages from testers.</p>

      <section class="admin-panel" id="feedback">
        <h2>Feedback and Ideas ({feedback_count})</h2>
        <table class="admin-table admin-feedback-table">
          <thead>
            <tr>
              <th>Type</th>
              <th>Name</th>
              <th>Email</th>
              <th>User ID</th>
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
        feedback_rows = render_feedback_rows(&feedback, "No feedback submissions yet."),
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
            pet_color: String::new(),
            pet_mood: String::new(),
            pet_emoji: "🐱".to_string(),
            equipped_outfit: String::new(),
            owned_outfits: vec![],
            onboarding_completed: true,
            pet_age_weeks: Some(weeks),
            pet_age_years: None,
            last_vet_date: None,
            never_been_to_vet: false,
            veterinary_notes: vec![],
            vet_notes: None,
            vet_followup_pending: false,
            pet_conditions: String::new(),
            pet_medications: String::new(),
            pet_indoor_outdoor: Some(indoor.to_string()),
            vaccine_history: vec![],
            pet_vaccines_unknown: false,
            tasks: vec![],
            calendar_events: vec![],
            activity: vec![],
            stripe_customer_id: None,
            pet_photo_url: None,
        }
    }

    #[test]
    fn signup_password_requires_minimum_length() {
        assert!(!password_meets_signup_requirements("a1!"));
        assert!(password_meets_signup_requirements("ab12!"));
    }

    #[test]
    fn signup_password_requires_digit() {
        assert!(!password_meets_signup_requirements("abcde!"));
        assert!(password_meets_signup_requirements("abcde1!"));
    }

    #[test]
    fn signup_password_requires_special_character() {
        assert!(!password_meets_signup_requirements("abcde1"));
        assert!(password_meets_signup_requirements("abcde1!"));
    }

    #[test]
    fn signup_passwords_must_match() {
        assert!(signup_passwords_match("abcde1!", "abcde1!"));
        assert!(!signup_passwords_match("abcde1!", "abcde1?"));
    }

    #[test]
    fn login_prefill_cookie_round_trips_special_characters() {
        let encoded = encode_login_prefill_cookie_value(
            "user+tag@example.com",
            "p@ss \"word'&<>",
        );
        let payload = decode_login_prefill_cookie_value(&encoded).expect("decode prefill");
        assert_eq!(payload.email, "user+tag@example.com");
        assert_eq!(payload.password, "p@ss \"word'&<>");
    }

    #[test]
    fn login_prefill_cookie_rejects_invalid_payload() {
        assert!(decode_login_prefill_cookie_value("not-valid").is_none());
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
            "cat_name=Mochi&pet_breed=Domestic+Shorthair&pet_color=Tabby&age_value=2&age_unit=years&pet_indoor_outdoor=indoor&last_vet_date=&conditions=&medications={extra}"
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

    #[test]
    fn onboarding_form_deserializes_never_been_to_vet_without_date() {
        let form: OnboardingForm = serde_urlencoded::from_str(&onboarding_form_body(
            "&never_been_to_vet=on&vaccine_names=&vaccine_dates=",
        ))
        .expect("form");
        assert!(form.never_been_to_vet);
        assert!(form.last_vet_date.is_empty());
    }

    #[test]
    fn validate_pet_photo_accepts_png_magic_bytes() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert_eq!(
            validate_pet_photo(Some("image/png"), &png),
            Ok("png")
        );
    }

    #[test]
    fn validate_pet_photo_rejects_oversized_file() {
        let bytes = vec![0xFF; MAX_PET_PHOTO_BYTES + 1];
        assert!(validate_pet_photo(Some("image/jpeg"), &bytes).is_err());
    }

    #[test]
    fn validate_pet_photo_rejects_bad_content_type() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert!(validate_pet_photo(Some("application/pdf"), &png).is_err());
    }

    #[test]
    fn render_pet_avatar_renders_cinder_stage() {
        let profile = test_profile_weeks(10, "indoor");
        let html = render_pet_avatar(&profile);
        assert!(html.contains("cinder-pet-stage"));
        assert!(html.contains("cinder-pet-image"));
        assert!(html.contains("/cinderanimate.png"));
        assert!(html.contains("Mochi"));
    }

    #[test]
    fn render_pet_user_photo_optional_when_uploaded() {
        let mut profile = test_profile_weeks(10, "indoor");
        profile.pet_photo_url = Some("/uploads/example.jpg".to_string());
        let html = render_pet_avatar(&profile);
        assert!(html.contains("cinder-photo-toggle"));
        assert!(html.contains("/uploads/example.jpg"));
        assert!(html.contains("pet-user-photo-optional"));
    }

    #[test]
    fn vet_calendar_skips_last_visit_when_never_been() {
        let profile = test_profile_weeks(52, "indoor");
        let mut profile = profile;
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.last_vet_date = None;
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        let events = generate_vet_calendar_events(&profile, today);
        assert!(
            !events
                .iter()
                .any(|event| event.title.contains("Last vet visit"))
        );
        assert!(
            events
                .iter()
                .any(|event| event.title.contains("Vet checkup reminder"))
        );
    }

    #[test]
    fn never_been_to_vet_triggers_asap_task() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.never_been_to_vet = true;
        profile.last_vet_date = None;
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        assert!(needs_vet_appointment_asap(&profile, today));
        profile.tasks.clear();
        assert!(refresh_profile_tasks(&mut profile));
        assert!(
            profile
                .tasks
                .iter()
                .any(|task| task.id == VET_APPOINTMENT_TASK_ID)
        );
    }

    #[test]
    fn onboarding_form_deserializes_pet_vaccines_unknown() {
        let form: OnboardingForm = serde_urlencoded::from_str(&onboarding_form_body(
            "&pet_vaccines_unknown=on&vaccine_names=&vaccine_dates=",
        ))
        .expect("form");
        assert!(form.pet_vaccines_unknown);
        assert!(parse_vaccine_history(&form.vaccine_names, &form.vaccine_dates).is_empty());
    }

    #[test]
    fn unknown_vaccines_triggers_asap_task() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.pet_vaccines_unknown = true;
        profile.last_vet_date = Some("2025-01-01".to_string());
        profile.never_been_to_vet = false;
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        assert!(needs_vet_appointment_asap(&profile, today));
        profile.tasks.clear();
        assert!(refresh_profile_tasks(&mut profile));
        assert!(
            profile
                .tasks
                .iter()
                .any(|task| task.id == VET_APPOINTMENT_TASK_ID)
        );
    }

    #[test]
    fn vet_urgency_alert_shows_on_pet_and_calendar_tabs() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.pet_vaccines_unknown = true;
        profile.last_vet_date = Some("2025-01-01".to_string());

        let pet_alert = render_vet_urgency_alert(&profile, "pet-tab-vet-alert");
        assert!(pet_alert.contains("vaccine-unknown-alert"));
        assert!(pet_alert.contains("pet-tab-vet-alert"));
        assert!(pet_alert.contains("make a vet appointment ASAP"));

        let calendar_alert = render_vet_urgency_alert(&profile, "calendar-tab-vet-alert");
        assert!(calendar_alert.contains("calendar-tab-vet-alert"));
        assert!(calendar_alert.contains("vaccine history"));
    }

    #[test]
    fn vet_urgency_alert_hidden_when_not_needed() {
        let profile = admin_profile(&admin_email());
        assert!(render_vet_urgency_alert(&profile, "pet-tab-vet-alert").is_empty());
        assert!(render_vet_urgency_alert(&profile, "calendar-tab-vet-alert").is_empty());
    }

    #[test]
    fn overdue_vaccine_triggers_asap_task() {
        let mut profile = test_profile_weeks(10, "indoor");
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        profile.never_been_to_vet = false;
        profile.last_vet_date = Some("2025-01-01".to_string());
        assert!(vaccines_due_or_overdue(&profile, today));
        assert!(needs_vet_appointment_asap(&profile, today));
    }

    #[test]
    fn admin_credentials_match_configured_email_and_password() {
        assert!(is_admin_credentials(
            &admin_email(),
            &admin_password()
        ));
        assert!(!is_admin_credentials(&admin_email(), "wrong-password"));
        assert!(!is_admin_credentials("other@example.com", &admin_password()));
    }

    #[test]
    fn complete_sign_in_grants_admin_session_for_admin_email() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-admin-login-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        storage
            .save_user(&User {
                username: "AdminUser".to_string(),
                first_name: "WhiskerWatch".to_string(),
                last_name: "Admin".to_string(),
                email: admin_email(),
                password: "CustomSignup1!".to_string(),
                created_at: 1,
            })
            .expect("save admin user");

        let state = AppState {
            storage,
            admin_sessions: Arc::new(Mutex::new(HashSet::new())),
            user_sessions: Arc::new(Mutex::new(HashMap::new())),
        };

        let jar = complete_sign_in(&state, CookieJar::new(), &admin_email());
        assert!(admin_session_valid(&state, &jar));
        assert_eq!(
            user_session_email(&state, &jar).as_deref(),
            Some(admin_email().as_str())
        );
    }

    #[test]
    fn admin_env_password_syncs_database_hash() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-admin-sync-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        storage
            .save_user(&User {
                username: "AdminUser".to_string(),
                first_name: "WhiskerWatch".to_string(),
                last_name: "Admin".to_string(),
                email: admin_email(),
                password: "OldSignup1!".to_string(),
                created_at: 1,
            })
            .expect("save admin user");

        storage
            .set_user_password(&admin_email(), &admin_password())
            .expect("sync admin password");

        assert!(
            stateless_validate_admin_password(&storage),
            "admin env password should match database after sync"
        );
    }

    fn stateless_validate_admin_password(storage: &Storage) -> bool {
        storage
            .validate_login(&admin_email(), &admin_password())
            .unwrap_or(false)
    }

    #[test]
    fn admin_account_skips_onboarding_modal() {
        let profile = admin_profile(&admin_email());
        assert!(render_onboarding_modal(&profile).is_empty());
    }

    #[test]
    fn admin_account_skips_vet_appointment_task() {
        let profile = admin_profile(&admin_email());
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        assert!(!needs_vet_appointment_asap(&profile, today));
    }

    #[test]
    fn vet_notes_round_trips_in_profile_json() {
        let mut profile = test_profile_weeks(10, "indoor");
        profile.vet_notes = Some("Follow up on dental cleaning in 6 months.".to_string());
        let json = serde_json::to_string(&profile).expect("serialize profile");
        let restored: UserProfile = serde_json::from_str(&json).expect("deserialize profile");
        assert_eq!(
            restored.vet_notes.as_deref(),
            Some("Follow up on dental cleaning in 6 months.")
        );
    }

    #[test]
    fn health_tab_shows_vet_notes_form() {
        let mut profile = test_profile_weeks(10, "indoor");
        profile.vet_notes = Some("Annual bloodwork due.".to_string());
        let html = render_health_tab(&profile);
        assert!(html.contains("action=\"/home/vet-notes\""));
        assert!(html.contains("Annual bloodwork due."));
        assert!(html.contains("Save vet notes"));

        profile.vet_notes = None;
        let empty_html = render_health_tab(&profile);
        assert!(empty_html.contains("Add vet notes"));
        assert!(empty_html.contains("No vet notes yet"));
    }

    #[test]
    fn admin_dashboard_nav_link_only_when_admin_session() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-admin-nav-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        let state = AppState {
            storage,
            admin_sessions: Arc::new(Mutex::new(HashSet::new())),
            user_sessions: Arc::new(Mutex::new(HashMap::new())),
        };
        let jar = CookieJar::new();
        assert_eq!(admin_dashboard_nav_link(&state, &jar), "");

        let jar = create_admin_session(&state, jar);
        assert!(admin_dashboard_nav_link(&state, &jar).contains("/admin"));
    }

    #[test]
    fn forum_tab_renders_ask_form_and_threads() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-forum-tab-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        let state = AppState {
            storage,
            admin_sessions: Arc::new(Mutex::new(HashSet::new())),
            user_sessions: Arc::new(Mutex::new(HashMap::new())),
        };

        let post_id = state
            .storage
            .create_forum_post(
                "user@test.local",
                "catmom",
                "Best brush for longhair?",
                "My cat hates brushing.",
                1_700_000_000,
            )
            .expect("create post");

        let html = render_dashboard_forum_tab(&state, Some(post_id));
        assert!(html.contains("Ask a question"));
        assert!(html.contains("Best brush for longhair?"));
        assert!(html.contains(&format!(r#"data-post-id="{post_id}""#)));
        assert!(html.contains("Post reply"));
    }

    #[test]
    fn dashboard_admin_nav_placeholders_are_replaced() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-admin-nav-template-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        let state = AppState {
            storage,
            admin_sessions: Arc::new(Mutex::new(HashSet::new())),
            user_sessions: Arc::new(Mutex::new(HashMap::new())),
        };
        let template = "<nav>{{ADMIN_NAV_LINK}}\n{{admin_nav_link}}</nav>";

        let jar = CookieJar::new();
        let html = replace_admin_nav_link(template, &state, &jar);
        assert!(!html.contains("{{"));
        assert!(!html.contains("ADMIN_NAV_LINK"));
        assert!(!html.contains("admin_nav_link"));

        let jar = create_admin_session(&state, jar);
        let html = replace_admin_nav_link(template, &state, &jar);
        assert!(html.contains(r#"<a href="/admin">ADMIN</a>"#));
        assert_eq!(html.matches(r#"<a href="/admin">ADMIN</a>"#).count(), 2);
    }

    #[test]
    fn admin_feedback_list_renders_submissions_with_user_id() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-admin-feedback-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        storage
            .save_feedback(&FeedbackSubmission {
                name: "Cat Mom".to_string(),
                email: "catmom@example.com".to_string(),
                category: "idea".to_string(),
                message: "Add a treat counter".to_string(),
                submitted_at: 1_700_000_000,
                user_id: Some("catmom@example.com".to_string()),
            })
            .expect("save feedback");

        let feedback = storage.load_feedback().expect("load feedback");
        let html = render_feedback_rows(&feedback, "No feedback submissions yet.");

        assert!(html.contains("Cat Mom"));
        assert!(html.contains("catmom@example.com"));
        assert!(html.contains("Add a treat counter"));
        assert!(html.contains("idea"));
    }

    #[test]
    fn admin_page_requires_valid_session() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-admin-gate-{}",
            Uuid::new_v4()
        )))
        .expect("storage");
        let state = AppState {
            storage,
            admin_sessions: Arc::new(Mutex::new(HashSet::new())),
            user_sessions: Arc::new(Mutex::new(HashMap::new())),
        };
        let jar = CookieJar::new();
        assert!(!admin_session_valid(&state, &jar));

        let jar = create_admin_session(&state, jar);
        assert!(admin_session_valid(&state, &jar));
    }
}

#[tokio::main]
async fn main() {
    let storage = Storage::open().unwrap_or_else(|error| {
        panic!("failed to open storage: {error:?}");
    });
    let uploads_dir = storage.data_dir().join("uploads");
    if let Err(error) = std::fs::create_dir_all(&uploads_dir) {
        eprintln!("warning: could not create uploads directory {}: {error}", uploads_dir.display());
    }
    let db_path = storage.db_path();
    eprintln!(
        "Using data directory: {} (database: {})",
        storage.data_dir().display(),
        db_path.display()
    );
    if !std::env::var("DATA_DIR").map(|v| !v.trim().is_empty()).unwrap_or(false) {
        eprintln!(
            "Tip: set DATA_DIR to a fixed absolute path if accounts seem to disappear between runs."
        );
    }

    let state = AppState {
        storage,
        admin_sessions: Arc::new(Mutex::new(HashSet::new())),
        user_sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(index_page))
        .route("/index.html", get(|| async { Redirect::permanent("/") }))
        .route("/home", get(dashboard_page))
        .route("/home/onboarding", post(onboarding_submit))
        .route("/home/vet-visit", post(vet_visit_submit))
        .route("/home/vet-notes", post(vet_notes_submit))
        .route("/home/outfits/buy", post(outfit_buy))
        .route("/home/outfits/equip", post(outfit_equip))
        .route("/home/tasks/toggle", post(task_toggle))
        .route("/home/forum/post", post(forum_post_submit))
        .route("/home/forum/reply", post(forum_reply_submit))
        .route("/home/forum/{id}", get(forum_thread_redirect))
        .route("/home/paw-points/checkout", post(paw_points_checkout))
        .route("/webhooks/stripe", post(stripe_webhook))
        .route("/logout", post(user_logout))
        .route("/login", get(login_page).post(login_submit))
        .route("/signup", get(signup_page).post(signup_submit))
        .route(
            "/forgot-password",
            get(forgot_password_page).post(forgot_password_submit),
        )
        .route(
            "/reset-password",
            get(reset_password_page).post(reset_password_submit),
        )
        .route("/contact", get(contact_page).post(contact_submit))
        .route("/feedback", get(feedback_page).post(feedback_submit))
        .route("/admin", get(admin_page))
        .route("/admin/logout", post(admin_logout))
        .route("/login.html", get(|| async { Redirect::permanent("/login") }))
        .route("/signup.html", get(|| async { Redirect::permanent("/signup") }))
        .route("/contact.html", get(|| async { Redirect::permanent("/contact") }))
        .route("/feedback.html", get(|| async { Redirect::permanent("/feedback") }))
        .nest_service("/uploads", ServeDir::new(uploads_dir))
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
