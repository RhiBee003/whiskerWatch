use axum::{
    Form, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{fs, io::AsyncWriteExt, net::TcpListener};
use tower_http::services::ServeDir;
use uuid::Uuid;

const ADMIN_SESSION_COOKIE: &str = "ww_admin_session";
const USER_SESSION_COOKIE: &str = "ww_user_session";

#[derive(Clone)]
struct AppState {
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
    reward: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct CalendarEvent {
    day: u32,
    title: String,
    time_label: String,
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

async fn user_name_for_email(email: &str) -> Option<String> {
    load_users()
        .await
        .into_iter()
        .find(|user| user.email.eq_ignore_ascii_case(email))
        .map(|user| user.name)
}

async fn form_prefill(state: &AppState, jar: &CookieJar) -> (String, String) {
    let Some(email) = user_session_email(state, jar) else {
        return (String::new(), String::new());
    };

    let form_email = escape_html_attr(&email);
    let form_name = user_name_for_email(&email)
        .await
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

fn default_profile(email: &str) -> UserProfile {
    UserProfile {
        email: email.to_string(),
        paw_points: 150,
        parent_level: 2,
        parent_xp: 40,
        pet_name: "Mochi".to_string(),
        pet_breed: "Tabby companion".to_string(),
        pet_mood: "Playful".to_string(),
        pet_emoji: "🐱".to_string(),
        equipped_outfit: "Classic Collar".to_string(),
        owned_outfits: vec!["classic_collar".to_string()],
        tasks: vec![
            UserTask {
                id: "feed_breakfast".to_string(),
                title: "Morning feeding".to_string(),
                completed: false,
                due_label: "Today · 8:00 AM".to_string(),
                reward: 15,
            },
            UserTask {
                id: "play_session".to_string(),
                title: "15-minute play session".to_string(),
                completed: false,
                due_label: "Today · 5:30 PM".to_string(),
                reward: 20,
            },
            UserTask {
                id: "litter_check".to_string(),
                title: "Refresh litter box".to_string(),
                completed: true,
                due_label: "Yesterday".to_string(),
                reward: 10,
            },
            UserTask {
                id: "water_bowl".to_string(),
                title: "Refill water bowl".to_string(),
                completed: false,
                due_label: "Today · anytime".to_string(),
                reward: 12,
            },
        ],
        calendar_events: vec![
            CalendarEvent {
                day: 29,
                title: "Vet checkup reminder".to_string(),
                time_label: "May 29 · 2:00 PM".to_string(),
            },
            CalendarEvent {
                day: 31,
                title: "Grooming day".to_string(),
                time_label: "May 31 · 10:00 AM".to_string(),
            },
            CalendarEvent {
                day: 3,
                title: "New treats delivery".to_string(),
                time_label: "Jun 3 · afternoon".to_string(),
            },
        ],
        activity: vec![
            ProfileActivity {
                message: "Welcome to your WhiskerWatch home!".to_string(),
                timestamp: timestamp_now(),
            },
            ProfileActivity {
                message: "Earned 10 paw points for litter box care.".to_string(),
                timestamp: timestamp_now().saturating_sub(86_400),
            },
        ],
    }
}

async fn load_profiles() -> Vec<UserProfile> {
    load_json_lines::<UserProfile>("data/user_profiles.jsonl").await
}

async fn save_profile(profile: &UserProfile) -> Result<(), std::io::Error> {
    let mut profiles = load_profiles().await;
    if let Some(existing) = profiles
        .iter_mut()
        .find(|item| item.email.eq_ignore_ascii_case(&profile.email))
    {
        *existing = profile.clone();
    } else {
        profiles.push(profile.clone());
    }

    fs::create_dir_all("data").await?;
    let mut lines = String::new();
    for item in profiles {
        lines.push_str(&serde_json::to_string(&item).expect("profile should serialize"));
        lines.push('\n');
    }
    fs::write("data/user_profiles.jsonl", lines).await
}

async fn get_or_create_profile(email: &str) -> UserProfile {
    if let Some(profile) = load_profiles()
        .await
        .into_iter()
        .find(|item| item.email.eq_ignore_ascii_case(email))
    {
        return profile;
    }

    let profile = default_profile(email);
    let _ = save_profile(&profile).await;
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
        Ok(contents) => Html(contents).into_response(),
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

fn render_calendar_grid(profile: &UserProfile) -> String {
    let weekday_labels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut html = String::new();

    for label in weekday_labels {
        html.push_str(&format!(r#"<span class="calendar-head">{label}</span>"#));
    }

    let first_weekday = 5_u32;
    let days_in_month = 31_u32;
    let today = 29_u32;

    for _ in 0..first_weekday {
        html.push_str(r#"<span class="calendar-day empty"></span>"#);
    }

    let event_days: HashSet<u32> = profile.calendar_events.iter().map(|e| e.day).collect();

    for day in 1..=days_in_month {
        let mut classes = vec!["calendar-day"];
        if day == today {
            classes.push("today");
        }
        if event_days.contains(&day) {
            classes.push("has-event");
        }
        html.push_str(&format!(
            r#"<span class="{}" aria-label="May {}">{day}</span>"#,
            classes.join(" "),
            day
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
                "<li><strong>{}</strong> — {}</li>",
                escape_html(&event.time_label),
                escape_html(&event.title)
            )
        })
        .collect()
}

async fn member_since_label(email: &str) -> String {
    load_users()
        .await
        .into_iter()
        .find(|user| user.email.eq_ignore_ascii_case(email))
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

    let profile = get_or_create_profile(&email).await;
    let user_name = user_name_for_email(&email)
        .await
        .unwrap_or_else(|| "Parent".to_string());
    let (level_progress_pct, level_progress_text) = level_progress(&profile);

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
        .replace("{{MEMBER_SINCE}}", &escape_html(&member_since_label(&email).await))
        .replace("{{PAW_POINTS}}", &profile.paw_points.to_string())
        .replace("{{PARENT_LEVEL}}", &profile.parent_level.to_string())
        .replace("{{LEVEL_PROGRESS}}", &level_progress_pct.to_string())
        .replace("{{LEVEL_PROGRESS_TEXT}}", &escape_html(&level_progress_text))
        .replace("{{PET_NAME}}", &escape_html(&profile.pet_name))
        .replace("{{PET_BREED}}", &escape_html(&profile.pet_breed))
        .replace("{{PET_MOOD}}", &escape_html(&profile.pet_mood))
        .replace("{{PET_EMOJI}}", &profile.pet_emoji)
        .replace("{{EQUIPPED_OUTFIT}}", &escape_html(&profile.equipped_outfit))
        .replace("{{STATUS_BLOCK}}", &dashboard_status_block(query.status.as_deref()))
        .replace("{{ACTIVITY_LIST}}", &render_activity_list(&profile))
        .replace("{{OUTFIT_CARDS}}", &render_outfit_cards(&profile))
        .replace("{{TASK_LIST}}", &render_task_list(&profile))
        .replace("{{CALENDAR_GRID}}", &render_calendar_grid(&profile))
        .replace("{{EVENT_LIST}}", &render_event_list(&profile))
        .replace("{{CALENDAR_MONTH_LABEL}}", "May 2026 — your cat care schedule");

    Html(body).into_response()
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

    let mut profile = get_or_create_profile(&email).await;

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

    match save_profile(&profile).await {
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

    let mut profile = get_or_create_profile(&email).await;

    if !profile.owned_outfits.iter().any(|id| id == outfit.id) {
        return Redirect::to("/home?tab=outfits&status=outfit_invalid");
    }

    profile.equipped_outfit = outfit.name.to_string();
    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Equipped {} on {}.", outfit.name, pet_name),
    );

    match save_profile(&profile).await {
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

    let mut profile = get_or_create_profile(&email).await;
    let task_id = form.task_id.trim();

    let Some(index) = profile.tasks.iter().position(|task| task.id == task_id) else {
        return Redirect::to("/home?tab=tasks&status=task_invalid");
    };

    if profile.tasks[index].completed {
        let title = profile.tasks[index].title.clone();
        profile.tasks[index].completed = false;
        push_activity(&mut profile, &format!("Reopened task: {title}."));
        return match save_profile(&profile).await {
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

    match save_profile(&profile).await {
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
        _ => return Redirect::to("/home?tab=account&status=points_invalid"),
    };

    let mut profile = get_or_create_profile(&email).await;
    profile.paw_points += points;
    push_activity(
        &mut profile,
        &format!("Purchased {points} paw points with card ending {}.", &card_number[card_number.len().saturating_sub(4)..]),
    );

    match save_profile(&profile).await {
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

    if user_login_valid(email, password).await {
        return signed_in_redirect(&state, jar, email);
    }

    if !email_exists(email).await {
        let encoded_email = encode_component(email);
        return Redirect::to(&format!("/signup?reason=notfound&email={encoded_email}")).into_response();
    }

    Redirect::to("/login?error=invalid").into_response()
}

async fn load_users() -> Vec<User> {
    let contents = match fs::read_to_string("data/users.jsonl").await {
        Ok(contents) => contents,
        Err(_) => return Vec::new(),
    };

    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<User>(line).ok())
        .collect()
}

async fn user_login_valid(email: &str, password: &str) -> bool {
    load_users()
        .await
        .into_iter()
        .any(|user| user.email.eq_ignore_ascii_case(email) && user.password == password)
}

async fn email_exists(email: &str) -> bool {
    if email.eq_ignore_ascii_case("demo@whiskerwatch.app")
        || email.eq_ignore_ascii_case(&admin_email())
    {
        return true;
    }

    load_users()
        .await
        .into_iter()
        .any(|user| user.email.eq_ignore_ascii_case(email))
}

async fn save_user(form: &SignupForm) -> Result<(), std::io::Error> {
    fs::create_dir_all("data").await?;

    let user = User {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        password: form.password.trim().to_string(),
        created_at: timestamp_now(),
    };

    append_json_line("data/users.jsonl", &user).await
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

    if email_exists(email).await {
        return Redirect::to("/signup?error=exists").into_response();
    }

    match save_user(&form).await {
        Ok(()) => signed_in_redirect(&state, jar, email),
        Err(_) => Redirect::to("/signup?error=failed").into_response(),
    }
}

async fn append_json_line<T: Serialize>(path: &str, value: &T) -> Result<(), std::io::Error> {
    fs::create_dir_all("data").await?;
    let line = serde_json::to_string(value).expect("value should serialize");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(format!("{line}\n").as_bytes()).await?;
    Ok(())
}

async fn save_contact_submission(form: &ContactForm) -> Result<(), std::io::Error> {
    let submission = ContactSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        subject: form.subject.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
    };

    append_json_line("data/contact_messages.jsonl", &submission).await
}

async fn save_feedback_submission(form: &FeedbackForm) -> Result<(), std::io::Error> {
    let submission = FeedbackSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        category: form.category.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
    };

    append_json_line("data/feedback.jsonl", &submission).await
}

async fn contact_submit(Form(form): Form<ContactForm>) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let subject = form.subject.trim();
    let message = form.message.trim();

    if name.is_empty() || email.is_empty() || subject.is_empty() || message.is_empty() {
        return Redirect::to("/contact?status=missing");
    }

    match save_contact_submission(&form).await {
        Ok(()) => Redirect::to("/contact?status=sent"),
        Err(_) => Redirect::to("/contact?status=failed"),
    }
}

async fn feedback_submit(Form(form): Form<FeedbackForm>) -> impl IntoResponse {
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

    match save_feedback_submission(&form).await {
        Ok(()) => Redirect::to("/feedback?status=sent"),
        Err(_) => Redirect::to("/feedback?status=failed"),
    }
}

async fn load_json_lines<T: for<'de> Deserialize<'de>>(path: &str) -> Vec<T> {
    let contents = match fs::read_to_string(path).await {
        Ok(contents) => contents,
        Err(_) => return Vec::new(),
    };

    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<T>(line).ok())
        .collect()
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

    let feedback = load_json_lines::<FeedbackSubmission>("data/feedback.jsonl").await;
    let contacts = load_json_lines::<ContactSubmission>("data/contact_messages.jsonl").await;

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

#[tokio::main]
async fn main() {
    let state = AppState {
        admin_sessions: Arc::new(Mutex::new(HashSet::new())),
        user_sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(index_page))
        .route("/home", get(dashboard_page))
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
