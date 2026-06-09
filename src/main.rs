use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, header::ACCEPT, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use serde::{
    de::{Deserializer, Error as DeError},
    Deserialize, Serialize,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time::Duration as CookieDuration;
use tokio::{fs, net::TcpListener};
use tower_http::services::ServeDir;
use uuid::Uuid;

mod breed_guides;
mod breed_seo;
mod breed_health;
mod breeds;
mod community;
mod email_delivery;
mod memorial;
mod entitlements;
mod onboarding_emails;
mod push_notifications;
mod share_cards;
mod sharing;
mod shelter_locator;
mod storage;
mod streak_rewards;
mod stripe_payments;
mod symptom_checker;
mod vet_financial_resources;
use storage::{ForumDeleteOutcome, Storage};
use stripe_payments::CheckoutError;

const ADMIN_SESSION_COOKIE: &str = "ww_admin_session";
const USER_SESSION_COOKIE: &str = "ww_user_session";
const LOGIN_PREFILL_COOKIE: &str = "ww_login_prefill";
const LOGIN_PREFILL_MAX_AGE_SECS: i64 = 120;
const AUTH_SESSION_MAX_AGE_SECS: i64 = 30 * 24 * 3600;

#[derive(Clone)]
struct AppState {
    storage: Storage,
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

#[derive(Deserialize)]
struct ChangePasswordForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}

#[derive(Deserialize)]
struct ChangePetNameForm {
    pet_name: String,
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
    feedback: Option<String>,
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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct CareSchedule {
    feed_time_minutes: u16,
    #[serde(default = "default_feed_lunch_time")]
    feed_lunch_time_minutes: u16,
    #[serde(default = "default_feed_afternoon_time")]
    feed_afternoon_time_minutes: u16,
    #[serde(default = "default_feed_dinner_time")]
    feed_dinner_time_minutes: u16,
    water_morning_time_minutes: u16,
    water_evening_time_minutes: u16,
    litter_time_minutes: u16,
    #[serde(default = "default_play_time_minutes")]
    play_time_minutes: u16,
}

pub(crate) const FEEDING_TASK_IDS: &[&str] = &[
    "feed_breakfast",
    "feed_lunch",
    "feed_afternoon",
    "feed_dinner",
];
const CUSTOM_TASK_ID_PREFIX: &str = "custom_";
const CUSTOM_TASK_REWARD: u32 = 10;
const MAX_CUSTOM_TASKS: usize = 20;
const MAX_CUSTOM_TASK_TITLE_LEN: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeedingPlan {
    FourMeals,
    ThreeMeals,
    TwoMeals,
}

fn default_play_time_minutes() -> u16 {
    17 * 60 + 30
}

fn default_feed_lunch_time() -> u16 {
    13 * 60
}

fn default_feed_afternoon_time() -> u16 {
    17 * 60
}

fn default_feed_dinner_time() -> u16 {
    18 * 60
}

fn default_care_schedule() -> CareSchedule {
    CareSchedule {
        feed_time_minutes: 8 * 60,
        feed_lunch_time_minutes: default_feed_lunch_time(),
        feed_afternoon_time_minutes: default_feed_afternoon_time(),
        feed_dinner_time_minutes: default_feed_dinner_time(),
        water_morning_time_minutes: 8 * 60,
        water_evening_time_minutes: 21 * 60,
        litter_time_minutes: 10 * 60,
        play_time_minutes: default_play_time_minutes(),
    }
}

fn feeding_plan_for_profile(profile: &UserProfile, today: NaiveDate) -> FeedingPlan {
    pet_snapshot(profile, PRIMARY_PET_ID)
        .map(|snapshot| feeding_plan_for_snapshot(&snapshot, today))
        .unwrap_or(FeedingPlan::TwoMeals)
}

fn feeding_specs_for_plan(
    plan: FeedingPlan,
    schedule: &CareSchedule,
) -> Vec<(&'static str, &'static str, u16, u32)> {
    match plan {
        FeedingPlan::FourMeals => vec![
            (
                "feed_breakfast",
                "Morning feeding",
                schedule.feed_time_minutes,
                10,
            ),
            (
                "feed_lunch",
                "Lunch feeding",
                schedule.feed_lunch_time_minutes,
                10,
            ),
            (
                "feed_afternoon",
                "Afternoon feeding",
                schedule.feed_afternoon_time_minutes,
                10,
            ),
            (
                "feed_dinner",
                "Evening feeding",
                schedule.feed_dinner_time_minutes,
                10,
            ),
        ],
        FeedingPlan::ThreeMeals => vec![
            (
                "feed_breakfast",
                "Morning feeding",
                schedule.feed_time_minutes,
                12,
            ),
            (
                "feed_lunch",
                "Lunch feeding",
                schedule.feed_lunch_time_minutes,
                12,
            ),
            (
                "feed_dinner",
                "Evening feeding",
                schedule.feed_dinner_time_minutes,
                12,
            ),
        ],
        FeedingPlan::TwoMeals => vec![
            (
                "feed_breakfast",
                "Morning feeding",
                schedule.feed_time_minutes,
                15,
            ),
            (
                "feed_dinner",
                "Evening feeding",
                schedule.feed_dinner_time_minutes,
                15,
            ),
        ],
    }
}

pub(crate) const PRIMARY_PET_ID: &str = "primary";

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
    #[serde(default = "default_task_time_minutes")]
    time_minutes: u16,
    reward: u32,
    #[serde(default = "default_task_pet_id")]
    pet_id: String,
}

fn default_task_pet_id() -> String {
    PRIMARY_PET_ID.to_string()
}

fn default_task_time_minutes() -> u16 {
    600
}

#[derive(Serialize, Deserialize, Clone)]
struct CalendarEvent {
    #[serde(default)]
    id: Option<String>,
    day: u32,
    #[serde(default = "default_calendar_month")]
    month: u32,
    #[serde(default = "default_calendar_year")]
    year: u32,
    title: String,
    time_label: String,
    #[serde(default = "default_event_time_minutes")]
    time_minutes: u16,
}

fn default_event_time_minutes() -> u16 {
    600
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

pub(crate) const VET_APPOINTMENT_TASK_ID: &str = "vet_appointment_asap";
const MAX_PET_PHOTO_BYTES: usize = 5 * 1024 * 1024;
const MAX_PET_VIDEO_BYTES: usize = 50 * 1024 * 1024;
const PET_VIDEO_CLIP_MIN_SECONDS: f32 = 3.0;
const PET_VIDEO_CLIP_MAX_SECONDS: f32 = 6.0;
const BASE_PET_PET_REWARD: u32 = 1;

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
    pet_birth_date: Option<String>,
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
    #[serde(default = "default_care_schedule")]
    care_schedule: CareSchedule,
    tasks: Vec<UserTask>,
    calendar_events: Vec<CalendarEvent>,
    #[serde(default)]
    user_calendar_events: Vec<CalendarEvent>,
    activity: Vec<ProfileActivity>,
    /// Stripe Customer id (`cus_...`) only—never PAN/CVV. Card data stays at Stripe.
    #[serde(default)]
    stripe_customer_id: Option<String>,
    #[serde(default)]
    pet_photo_url: Option<String>,
    #[serde(default)]
    pet_video_url: Option<String>,
    #[serde(default)]
    pet_video_clip_start: Option<f32>,
    #[serde(default)]
    pet_video_clip_duration: Option<f32>,
    #[serde(default)]
    pet_video_zoom: Option<f32>,
    #[serde(default)]
    pet_video_offset_x: Option<f32>,
    #[serde(default)]
    pet_video_offset_y: Option<f32>,
    #[serde(default)]
    deceased: bool,
    #[serde(default)]
    deceased_at: Option<String>,
    #[serde(default)]
    memorial_videos: Vec<String>,
    #[serde(default)]
    memorial_comfort_seen: bool,
    #[serde(default)]
    pending_purrfect_idea_ids: Vec<i64>,
    #[serde(default = "default_owned_decor")]
    owned_decor: Vec<String>,
    #[serde(default = "default_equipped_decor")]
    equipped_decor: HashMap<String, String>,
    #[serde(default = "default_owned_tap_boosts")]
    owned_tap_boosts: Vec<String>,
    #[serde(default = "default_equipped_tap_boost")]
    equipped_tap_boost: String,
    #[serde(default)]
    petting_bonus_inventory: HashMap<String, u32>,
    #[serde(default)]
    active_petting_bonus: String,
    #[serde(default)]
    petting_bonus_expires_at: u64,
    #[serde(default)]
    owned_breed_guides: Vec<String>,
    #[serde(default)]
    premium_unlocked: bool,
    #[serde(default)]
    additional_pets: Vec<HouseholdPet>,
    #[serde(default = "default_active_pet_id")]
    active_pet_id: String,
    #[serde(default)]
    active_pet_owner_email: Option<String>,
    #[serde(default)]
    care_streak_days: u32,
    #[serde(default)]
    care_streak_last_date: Option<String>,
    #[serde(default)]
    best_care_streak: u32,
    #[serde(default)]
    claimed_streak_rewards: Vec<u32>,
    #[serde(default = "default_community_visible")]
    community_visible: bool,
    #[serde(default)]
    notification_prefs: push_notifications::NotificationPrefs,
    #[serde(default)]
    notification_sent_dates: HashMap<String, String>,
    #[serde(default = "default_onboarding_emails_enabled")]
    onboarding_emails_enabled: bool,
    #[serde(default)]
    onboarding_emails_sent: Vec<String>,
}

fn default_onboarding_emails_enabled() -> bool {
    true
}

fn default_community_visible() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
struct HouseholdPet {
    #[serde(default = "new_household_pet_id")]
    id: String,
    pet_name: String,
    pet_breed: String,
    #[serde(default)]
    pet_color: String,
    #[serde(default = "default_pet_mood")]
    pet_mood: String,
    #[serde(default)]
    pet_age_weeks: Option<u32>,
    #[serde(default)]
    pet_age_years: Option<u32>,
    #[serde(default)]
    pet_birth_date: Option<String>,
    #[serde(default)]
    last_vet_date: Option<String>,
    #[serde(default)]
    never_been_to_vet: bool,
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
    #[serde(default = "default_care_schedule")]
    care_schedule: CareSchedule,
    #[serde(default)]
    pet_photo_url: Option<String>,
    #[serde(default)]
    pet_video_url: Option<String>,
    #[serde(default)]
    pet_video_clip_start: Option<f32>,
    #[serde(default)]
    pet_video_clip_duration: Option<f32>,
    #[serde(default)]
    pet_video_zoom: Option<f32>,
    #[serde(default)]
    pet_video_offset_x: Option<f32>,
    #[serde(default)]
    pet_video_offset_y: Option<f32>,
    #[serde(default)]
    deceased: bool,
    #[serde(default)]
    deceased_at: Option<String>,
    #[serde(default)]
    memorial_videos: Vec<String>,
    #[serde(default)]
    memorial_comfort_seen: bool,
}

fn new_household_pet_id() -> String {
    format!("pet_{}", Uuid::new_v4())
}

fn default_pet_mood() -> String {
    "Happy".to_string()
}

fn default_active_pet_id() -> String {
    PRIMARY_PET_ID.to_string()
}

#[derive(Clone)]
pub(crate) struct PetSnapshot {
    id: String,
    pet_name: String,
    pet_breed: String,
    pet_color: String,
    pet_mood: String,
    pet_age_weeks: Option<u32>,
    pet_age_years: Option<u32>,
    pet_birth_date: Option<String>,
    last_vet_date: Option<String>,
    never_been_to_vet: bool,
    pet_conditions: String,
    pet_medications: String,
    pet_indoor_outdoor: Option<String>,
    vaccine_history: Vec<VaccineRecord>,
    pet_vaccines_unknown: bool,
    care_schedule: CareSchedule,
    pet_photo_url: Option<String>,
    pet_video_url: Option<String>,
    pet_video_clip_start: Option<f32>,
    pet_video_clip_duration: Option<f32>,
    pet_video_zoom: Option<f32>,
    pet_video_offset_x: Option<f32>,
    pet_video_offset_y: Option<f32>,
    pub(crate) deceased: bool,
    pub(crate) deceased_at: Option<String>,
    pub(crate) memorial_videos: Vec<String>,
    pub(crate) memorial_comfort_seen: bool,
}

impl PetSnapshot {
    fn from_primary(profile: &UserProfile) -> Self {
        Self {
            id: PRIMARY_PET_ID.to_string(),
            pet_name: profile.pet_name.clone(),
            pet_breed: profile.pet_breed.clone(),
            pet_color: profile.pet_color.clone(),
            pet_mood: profile.pet_mood.clone(),
            pet_age_weeks: profile.pet_age_weeks,
            pet_age_years: profile.pet_age_years,
            pet_birth_date: profile.pet_birth_date.clone(),
            last_vet_date: profile.last_vet_date.clone(),
            never_been_to_vet: profile.never_been_to_vet,
            pet_conditions: profile.pet_conditions.clone(),
            pet_medications: profile.pet_medications.clone(),
            pet_indoor_outdoor: profile.pet_indoor_outdoor.clone(),
            vaccine_history: profile.vaccine_history.clone(),
            pet_vaccines_unknown: profile.pet_vaccines_unknown,
            care_schedule: profile.care_schedule.clone(),
            pet_photo_url: profile.pet_photo_url.clone(),
            pet_video_url: profile.pet_video_url.clone(),
            pet_video_clip_start: profile.pet_video_clip_start,
            pet_video_clip_duration: profile.pet_video_clip_duration,
            pet_video_zoom: profile.pet_video_zoom,
            pet_video_offset_x: profile.pet_video_offset_x,
            pet_video_offset_y: profile.pet_video_offset_y,
            deceased: profile.deceased,
            deceased_at: profile.deceased_at.clone(),
            memorial_videos: profile.memorial_videos.clone(),
            memorial_comfort_seen: profile.memorial_comfort_seen,
        }
    }

    fn from_household(pet: &HouseholdPet) -> Self {
        Self {
            id: pet.id.clone(),
            pet_name: pet.pet_name.clone(),
            pet_breed: pet.pet_breed.clone(),
            pet_color: pet.pet_color.clone(),
            pet_mood: pet.pet_mood.clone(),
            pet_age_weeks: pet.pet_age_weeks,
            pet_age_years: pet.pet_age_years,
            pet_birth_date: pet.pet_birth_date.clone(),
            last_vet_date: pet.last_vet_date.clone(),
            never_been_to_vet: pet.never_been_to_vet,
            pet_conditions: pet.pet_conditions.clone(),
            pet_medications: pet.pet_medications.clone(),
            pet_indoor_outdoor: pet.pet_indoor_outdoor.clone(),
            vaccine_history: pet.vaccine_history.clone(),
            pet_vaccines_unknown: pet.pet_vaccines_unknown,
            care_schedule: pet.care_schedule.clone(),
            pet_photo_url: pet.pet_photo_url.clone(),
            pet_video_url: pet.pet_video_url.clone(),
            pet_video_clip_start: pet.pet_video_clip_start,
            pet_video_clip_duration: pet.pet_video_clip_duration,
            pet_video_zoom: pet.pet_video_zoom,
            pet_video_offset_x: pet.pet_video_offset_x,
            pet_video_offset_y: pet.pet_video_offset_y,
            deceased: pet.deceased,
            deceased_at: pet.deceased_at.clone(),
            memorial_videos: pet.memorial_videos.clone(),
            memorial_comfort_seen: pet.memorial_comfort_seen,
        }
    }
}

pub(crate) fn household_pet_is_complete(pet: &HouseholdPet) -> bool {
    let name = pet.pet_name.trim();
    let breed = pet.pet_breed.trim();
    let has_name = !name.is_empty()
        && !name.eq_ignore_ascii_case("your cat")
        && !name.eq_ignore_ascii_case("no pet yet");
    let has_breed = !breed.is_empty() && !breed.eq_ignore_ascii_case("add your cat's details");
    let has_age = pet
        .pet_birth_date
        .as_deref()
        .is_some_and(|value| parse_vet_date(value).is_some())
        || pet.pet_age_weeks.is_some()
        || pet.pet_age_years.is_some();
    let has_lifestyle = pet
        .pet_indoor_outdoor
        .as_deref()
        .is_some_and(|value| value == "indoor" || value == "outdoor");
    has_name && has_breed && has_age && has_lifestyle
}

pub(crate) fn pet_snapshot(profile: &UserProfile, pet_id: &str) -> Option<PetSnapshot> {
    if pet_id == PRIMARY_PET_ID {
        if profile_has_pet(profile) {
            return Some(PetSnapshot::from_primary(profile));
        }
        return None;
    }
    profile
        .additional_pets
        .iter()
        .find(|pet| pet.id == pet_id && household_pet_is_complete(pet))
        .map(PetSnapshot::from_household)
}

pub(crate) fn active_pet_snapshot(profile: &UserProfile) -> Option<PetSnapshot> {
    pet_snapshot(profile, &profile.active_pet_id)
        .or_else(|| pet_snapshot(profile, PRIMARY_PET_ID))
}

pub(crate) fn list_pet_summaries(profile: &UserProfile) -> Vec<(String, String)> {
    let mut pets = Vec::new();
    if profile_has_pet(profile) {
        pets.push((PRIMARY_PET_ID.to_string(), profile.pet_name.clone()));
    }
    for pet in &profile.additional_pets {
        if household_pet_is_complete(pet) {
            pets.push((pet.id.clone(), pet.pet_name.clone()));
        }
    }
    pets
}

fn pet_id_exists(profile: &UserProfile, pet_id: &str) -> bool {
    pet_snapshot(profile, pet_id).is_some()
}

fn normalize_profile_pets(profile: &mut UserProfile) -> bool {
    let mut changed = false;
    if profile.active_pet_id.is_empty() {
        profile.active_pet_id = PRIMARY_PET_ID.to_string();
        changed = true;
    }
    for pet in &mut profile.additional_pets {
        if pet.id.is_empty() {
            pet.id = new_household_pet_id();
            changed = true;
        }
    }
    if !pet_id_exists(profile, &profile.active_pet_id) {
        profile.active_pet_id = if profile_has_pet(profile) {
            PRIMARY_PET_ID.to_string()
        } else if let Some(pet) = profile
            .additional_pets
            .iter()
            .find(|pet| household_pet_is_complete(pet))
        {
            pet.id.clone()
        } else {
            PRIMARY_PET_ID.to_string()
        };
        changed = true;
    }
    for task in &mut profile.tasks {
        if task.pet_id.is_empty() {
            task.pet_id = PRIMARY_PET_ID.to_string();
            changed = true;
        }
    }
    if profile
        .active_pet_owner_email
        .as_deref()
        .is_some_and(|owner| owner.eq_ignore_ascii_case(&profile.email))
    {
        profile.active_pet_owner_email = None;
        changed = true;
    }
    changed
}

fn task_pet_owner_hidden_field(profile: &UserProfile) -> String {
    profile
        .active_pet_owner_email
        .as_deref()
        .filter(|owner| !owner.eq_ignore_ascii_case(&profile.email))
        .map(|owner| {
            format!(
                r#"<input type="hidden" name="pet_owner" value="{}" />"#,
                escape_html_attr(owner)
            )
        })
        .unwrap_or_default()
}

fn task_owner_hint<'a>(profile: &'a UserProfile, form_owner: &'a str) -> Option<&'a str> {
    let trimmed = form_owner.trim();
    if !trimmed.is_empty() {
        return Some(trimmed);
    }
    profile.active_pet_owner_email.as_deref()
}

fn set_active_pet(profile: &mut UserProfile, pet_id: &str) -> bool {
    if !pet_id_exists(profile, pet_id) {
        return false;
    }
    if profile.active_pet_id != pet_id {
        profile.active_pet_id = pet_id.to_string();
        return true;
    }
    false
}

fn pet_birth_date_for_snapshot(snapshot: &PetSnapshot, reference: NaiveDate) -> Option<NaiveDate> {
    if let Some(stored) = snapshot
        .pet_birth_date
        .as_deref()
        .and_then(parse_vet_date)
    {
        return Some(stored);
    }
    if let Some(weeks) = snapshot.pet_age_weeks {
        return reference.checked_sub_signed(Duration::weeks(weeks as i64));
    }
    if let Some(years) = snapshot.pet_age_years {
        return reference.checked_sub_signed(Duration::days(i64::from(years) * 365));
    }
    None
}

fn feeding_plan_for_snapshot(snapshot: &PetSnapshot, today: NaiveDate) -> FeedingPlan {
    let Some(birth) = pet_birth_date_for_snapshot(snapshot, today) else {
        return FeedingPlan::TwoMeals;
    };
    let days = (today - birth).num_days().max(0);
    if days < 182 {
        return FeedingPlan::FourMeals;
    }
    if days < 365 {
        return FeedingPlan::ThreeMeals;
    }
    FeedingPlan::TwoMeals
}

fn tasks_for_pet<'a>(profile: &'a UserProfile, pet_id: &str) -> Vec<&'a UserTask> {
    profile
        .tasks
        .iter()
        .filter(|task| task.pet_id == pet_id)
        .collect()
}

fn active_pet_id(profile: &UserProfile) -> &str {
    if pet_id_exists(profile, &profile.active_pet_id) {
        &profile.active_pet_id
    } else {
        PRIMARY_PET_ID
    }
}

fn resolve_task_pet_id(profile: &UserProfile, form_pet_id: &str) -> String {
    let trimmed = form_pet_id.trim();
    if !trimmed.is_empty() && pet_id_exists(profile, trimmed) {
        trimmed.to_string()
    } else {
        active_pet_id(profile).to_string()
    }
}

fn find_task_index(profile: &UserProfile, task_id: &str, pet_id: &str) -> Option<usize> {
    profile.tasks.iter().position(|task| {
        task.id == task_id && task.pet_id == pet_id
    })
}

fn render_pet_switcher(profile: &UserProfile) -> String {
    let pets = list_pet_summaries(profile);
    if pets.len() <= 1 {
        return String::new();
    }

    let active = active_pet_id(profile);
    let active_index = pets
        .iter()
        .position(|(id, _)| id == active)
        .unwrap_or(0);
    let prev_idx = if active_index == 0 {
        pets.len() - 1
    } else {
        active_index - 1
    };
    let next_idx = (active_index + 1) % pets.len();
    let prev_id = pets[prev_idx].0.clone();
    let next_id = pets[next_idx].0.clone();

    let tabs = pets
        .iter()
        .map(|(id, name)| {
            let active_class = if id == active {
                " pet-switcher-tab-active"
            } else {
                ""
            };
            format!(
                r#"<a href="/home?tab=pet&amp;pet={pet_id}" class="pet-switcher-tab{active_class}" aria-current="{current}">{name}</a>"#,
                pet_id = escape_html_attr(id),
                active_class = active_class,
                current = if id == active { "page" } else { "false" },
                name = escape_html(name),
            )
        })
        .collect::<String>();

    format!(
        r#"<nav class="pet-switcher" aria-label="Switch cat">
  <button type="button" class="pet-switcher-nav" data-pet-target="{prev_id}" aria-label="Previous cat">‹</button>
  <div class="pet-switcher-tabs">{tabs}</div>
  <button type="button" class="pet-switcher-nav" data-pet-target="{next_id}" aria-label="Next cat">›</button>
  <p class="field-hint pet-switcher-count">{position} of {total} cats</p>
</nav>"#,
        prev_id = escape_html_attr(&prev_id),
        next_id = escape_html_attr(&next_id),
        tabs = tabs,
        position = active_index + 1,
        total = pets.len(),
    )
}

struct OutfitCatalogItem {
    id: &'static str,
    name: &'static str,
    emoji: &'static str,
    price: u32,
}

struct DecorCatalogItem {
    id: &'static str,
    name: &'static str,
    emoji: &'static str,
    price: u32,
    slot: &'static str,
}

struct TapBoostCatalogItem {
    id: &'static str,
    name: &'static str,
    emoji: &'static str,
    price: u32,
    tap_reward: u32,
}

struct PettingBonusCatalogItem {
    id: &'static str,
    name: &'static str,
    emoji: &'static str,
    price: u32,
    multiplier: u32,
    duration_secs: u32,
}

const DECOR_CATALOG: [DecorCatalogItem; 10] = [
    DecorCatalogItem {
        id: "sunny_nook",
        name: "Sunny Window Nook",
        emoji: "🪟",
        price: 0,
        slot: "room",
    },
    DecorCatalogItem {
        id: "starry_night",
        name: "Starry Night Room",
        emoji: "🌙",
        price: 80,
        slot: "room",
    },
    DecorCatalogItem {
        id: "garden_view",
        name: "Garden View",
        emoji: "🌿",
        price: 100,
        slot: "room",
    },
    DecorCatalogItem {
        id: "soft_mat",
        name: "Soft Mat",
        emoji: "🧶",
        price: 0,
        slot: "rug",
    },
    DecorCatalogItem {
        id: "plush_rug",
        name: "Plush Rug",
        emoji: "🟣",
        price: 45,
        slot: "rug",
    },
    DecorCatalogItem {
        id: "cloud_bed",
        name: "Cloud Bed",
        emoji: "☁️",
        price: 65,
        slot: "bed",
    },
    DecorCatalogItem {
        id: "hammock",
        name: "Cozy Hammock",
        emoji: "🏝️",
        price: 55,
        slot: "bed",
    },
    DecorCatalogItem {
        id: "yarn_ball",
        name: "Yarn Ball",
        emoji: "🧵",
        price: 30,
        slot: "toy",
    },
    DecorCatalogItem {
        id: "cat_tree",
        name: "Mini Cat Tree",
        emoji: "🌳",
        price: 90,
        slot: "toy",
    },
    DecorCatalogItem {
        id: "potted_plant",
        name: "Potted Plant",
        emoji: "🪴",
        price: 35,
        slot: "plant",
    },
];

const TAP_BOOST_CATALOG: [TapBoostCatalogItem; 5] = [
    TapBoostCatalogItem {
        id: "basic_pets",
        name: "Gentle Pets",
        emoji: "🐾",
        price: 0,
        tap_reward: 1,
    },
    TapBoostCatalogItem {
        id: "happy_taps",
        name: "Happy Taps",
        emoji: "😸",
        price: 40,
        tap_reward: 2,
    },
    TapBoostCatalogItem {
        id: "super_scratcher",
        name: "Super Scratcher",
        emoji: "✨",
        price: 75,
        tap_reward: 3,
    },
    TapBoostCatalogItem {
        id: "mega_cuddles",
        name: "Mega Cuddles",
        emoji: "💖",
        price: 120,
        tap_reward: 5,
    },
    TapBoostCatalogItem {
        id: "legendary_love",
        name: "Legendary Love",
        emoji: "🌟",
        price: 200,
        tap_reward: 8,
    },
];

const PETTING_BONUS_CATALOG: [PettingBonusCatalogItem; 3] = [
    PettingBonusCatalogItem {
        id: "purr_rush",
        name: "Purr Rush",
        emoji: "💨",
        price: 30,
        multiplier: 2,
        duration_secs: 60,
    },
    PettingBonusCatalogItem {
        id: "cuddle_frenzy",
        name: "Cuddle Frenzy",
        emoji: "🔥",
        price: 75,
        multiplier: 5,
        duration_secs: 45,
    },
    PettingBonusCatalogItem {
        id: "love_explosion",
        name: "Love Explosion",
        emoji: "💥",
        price: 150,
        multiplier: 10,
        duration_secs: 60,
    },
];

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
    feedback: Option<String>,
    #[allow(dead_code)]
    cal_day: Option<String>,
    cal_month: Option<String>,
    cal_year: Option<String>,
    community: Option<String>,
    breed: Option<String>,
    add_cat: Option<String>,
    pet: Option<String>,
    pet_owner: Option<String>,
}

#[derive(Deserialize, Default)]
struct BreedSelectQuery {
    add_cat: Option<String>,
}

#[derive(Deserialize)]
struct OutfitBuyForm {
    outfit_id: String,
    #[serde(default, rename = "return_to")]
    _return_to: String,
}

#[derive(Deserialize)]
struct OutfitEquipForm {
    outfit_id: String,
    #[serde(default, rename = "return_to")]
    _return_to: String,
}

#[derive(Deserialize)]
struct DecorBuyForm {
    decor_id: String,
}

#[derive(Deserialize)]
struct DecorEquipForm {
    decor_id: String,
}

#[derive(Deserialize)]
struct TapBoostBuyForm {
    boost_id: String,
    #[serde(default, rename = "return_to")]
    _return_to: String,
}

#[derive(Deserialize)]
struct TapBoostEquipForm {
    boost_id: String,
    #[serde(default, rename = "return_to")]
    _return_to: String,
}

#[derive(Deserialize)]
struct PettingBonusBuyForm {
    bonus_id: String,
    #[serde(default, rename = "return_to")]
    _return_to: String,
}

#[derive(Deserialize)]
struct PettingBonusActivateForm {
    bonus_id: String,
    #[serde(default, rename = "return_to")]
    _return_to: String,
}

#[derive(Deserialize, Default)]
struct CatHomeQuery {
    status: Option<String>,
    decor_id: Option<String>,
    outfit_id: Option<String>,
    boost_id: Option<String>,
    petting_bonus_id: Option<String>,
}

#[derive(Deserialize)]
struct BreedGuideCheckoutForm {
    breed_slug: String,
}

#[derive(Deserialize)]
struct AddPetForm {
    pet_name: String,
    pet_breed: String,
    #[serde(default)]
    pet_color: String,
}

#[derive(Deserialize, Default)]
struct BreedGuideQuery {
    status: Option<String>,
    session_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct NeedPawPointsQuery {
    outfit_id: Option<String>,
    decor_id: Option<String>,
    boost_id: Option<String>,
    petting_bonus_id: Option<String>,
    return_to: Option<String>,
}

struct ShopItemQuote {
    name: &'static str,
    price: u32,
}

#[derive(Deserialize)]
struct TaskToggleForm {
    task_id: String,
    #[serde(default)]
    pet_id: String,
    #[serde(default)]
    pet_owner: String,
}

#[derive(Deserialize)]
struct TaskTimeForm {
    task_id: String,
    task_time: String,
    #[serde(default)]
    pet_id: String,
    #[serde(default)]
    pet_owner: String,
}

#[derive(Deserialize)]
struct TaskAddForm {
    task_title: String,
    #[serde(default)]
    pet_id: String,
    #[serde(default)]
    pet_owner: String,
}

#[derive(Deserialize)]
struct TaskDeleteForm {
    task_id: String,
    #[serde(default)]
    pet_id: String,
    #[serde(default)]
    pet_owner: String,
}

#[derive(Deserialize)]
struct CalendarEventAddForm {
    day: String,
    month: String,
    year: String,
    title: String,
    time_minutes: String,
}

#[derive(Deserialize)]
struct CalendarEventFormQuery {
    day: Option<String>,
    month: Option<String>,
    year: Option<String>,
}

#[derive(Serialize)]
struct TaskToggleResponse {
    ok: bool,
    status: &'static str,
    tasks_html: String,
    activity_html: String,
    paw_points: u32,
    paw_from_tasks: u32,
    xp_from_tasks: u32,
    parent_level: u32,
    parent_xp: u32,
    level_progress: u32,
    level_progress_text: String,
    calendar_data: serde_json::Value,
    show_vet_followup: bool,
    care_streak_days: u32,
    share_card: Option<share_cards::ShareCardOffer>,
}

#[derive(Serialize)]
struct CalendarEventAddResponse {
    ok: bool,
    status: &'static str,
    calendar_data: serde_json::Value,
}

#[derive(Serialize)]
struct PawPointsBalanceResponse {
    ok: bool,
    paw_points: u32,
}

#[derive(Serialize)]
struct ShopBuyResponse {
    ok: bool,
    status: &'static str,
    paw_points: u32,
    item_kind: &'static str,
    item_id: String,
    equipped: bool,
}

fn shop_buy_json_response(
    ok: bool,
    status: &'static str,
    paw_points: u32,
    item_kind: &'static str,
    item_id: &str,
    equipped: bool,
) -> Response {
    let code = if ok {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (
        code,
        Json(ShopBuyResponse {
            ok,
            status,
            paw_points,
            item_kind,
            item_id: item_id.to_string(),
            equipped,
        }),
    )
        .into_response()
}

#[derive(Serialize, Deserialize)]
struct PetNameChangeResponse {
    ok: bool,
    status: &'static str,
    pet_name: String,
}

#[derive(Serialize)]
struct PetPetResponse {
    ok: bool,
    paw_points: u32,
    tap_reward: u32,
    tap_boost_base: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tap_multiplier: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    petting_bonus_expires_at: Option<u64>,
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
    fields.get(key).is_some_and(|values| {
        values
            .iter()
            .any(|value| matches!(value.as_str(), "on" | "true" | "1"))
    })
}

struct OnboardingForm {
    cat_name: String,
    pet_breed: String,
    pet_color: String,
    pet_birth_date: String,
    pet_indoor_outdoor: String,
    last_vet_date: String,
    never_been_to_vet: bool,
    conditions: String,
    medications: String,
    vaccine_names: Vec<String>,
    vaccine_dates: Vec<String>,
    pet_vaccines_unknown: bool,
    skip_video: bool,
    pet_video_clip_start: f32,
    pet_video_clip_duration: f32,
    pet_video_zoom: Option<f32>,
    pet_video_offset_x: Option<f32>,
    pet_video_offset_y: Option<f32>,
}

impl OnboardingForm {
    fn from_fields<E: DeError>(fields: &HashMap<String, Vec<String>>) -> Result<Self, E> {
        Ok(OnboardingForm {
            cat_name: form_scalar(fields, "cat_name")?,
            pet_breed: form_scalar(fields, "pet_breed")?,
            pet_color: form_optional_scalar(fields, "pet_color"),
            pet_birth_date: form_scalar(fields, "pet_birth_date")?,
            pet_indoor_outdoor: form_scalar(fields, "pet_indoor_outdoor")?,
            last_vet_date: form_optional_scalar(fields, "last_vet_date"),
            never_been_to_vet: form_checkbox(fields, "never_been_to_vet"),
            conditions: form_optional_scalar(fields, "conditions"),
            medications: form_optional_scalar(fields, "medications"),
            vaccine_names: form_vec(fields, "vaccine_names"),
            vaccine_dates: form_vec(fields, "vaccine_dates"),
            pet_vaccines_unknown: form_checkbox(fields, "pet_vaccines_unknown"),
            skip_video: form_checkbox(fields, "skip_video"),
            pet_video_clip_start: parse_pet_video_clip_start(&form_optional_scalar(
                fields,
                "pet_video_clip_start",
            )),
            pet_video_clip_duration: parse_pet_video_clip_duration(&form_optional_scalar(
                fields,
                "pet_video_clip_duration",
            )),
            pet_video_zoom: parse_optional_video_float(&form_optional_scalar(fields, "pet_video_zoom")),
            pet_video_offset_x: parse_optional_video_float(&form_optional_scalar(
                fields,
                "pet_video_offset_x",
            )),
            pet_video_offset_y: parse_optional_video_float(&form_optional_scalar(
                fields,
                "pet_video_offset_y",
            )),
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

struct SymptomCheckForm {
    symptoms: String,
    quick_symptoms: Vec<String>,
}

struct ShelterSearchForm {
    shelter_zip: String,
    shelter_city: String,
    shelter_state: String,
}

impl<'de> Deserialize<'de> for ShelterSearchForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(String, String)>::deserialize(deserializer)?;
        let fields = group_form_fields(pairs);

        Ok(ShelterSearchForm {
            shelter_zip: form_optional_scalar(&fields, "shelter_zip"),
            shelter_city: form_optional_scalar(&fields, "shelter_city"),
            shelter_state: form_optional_scalar(&fields, "shelter_state"),
        })
    }
}

impl<'de> Deserialize<'de> for SymptomCheckForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(String, String)>::deserialize(deserializer)?;
        let fields = group_form_fields(pairs);

        Ok(SymptomCheckForm {
            symptoms: form_optional_scalar(&fields, "symptoms"),
            quick_symptoms: form_vec(&fields, "quick_symptoms"),
        })
    }
}

#[derive(Serialize)]
struct SymptomCheckResponse {
    ok: bool,
    status: Option<&'static str>,
    #[serde(flatten)]
    analysis: Option<symptom_checker::SymptomAnalysis>,
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

#[derive(Deserialize)]
struct FeedbackVoteForm {
    feedback_id: String,
    vote: String,
}

#[derive(Serialize)]
struct FeedbackVoteResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
    feedback_id: i64,
    upvotes: u32,
    downvotes: u32,
    user_vote: Option<i8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FeedbackSubmission {
    #[serde(default)]
    id: i64,
    name: String,
    email: String,
    category: String,
    message: String,
    submitted_at: u64,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    author_username: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ForumPost {
    id: i64,
    user_id: String,
    author_username: String,
    title: String,
    body: String,
    created_at: u64,
    #[serde(default)]
    breed_slug: String,
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
struct MemorialPetForm {
    pet_id: String,
}

#[derive(Deserialize)]
struct ForumPostForm {
    title: String,
    body: String,
    #[serde(default)]
    breed_slug: String,
}

#[derive(Deserialize)]
struct ForumReplyForm {
    post_id: String,
    body: String,
}

#[derive(Deserialize)]
struct ForumDeletePostForm {
    post_id: String,
}

#[derive(Deserialize)]
struct ForumDeleteReplyForm {
    reply_id: String,
    post_id: String,
}

#[derive(Deserialize)]
struct CommunityVisibilityForm {
    #[serde(default)]
    community_visible: String,
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

pub(crate) fn company_email() -> String {
    env_or_default("COMPANY_EMAIL", "WhiskerWatch.official@gmail.com")
}

fn admin_password() -> String {
    env_or_default("ADMIN_PASSWORD", "WhiskerAdmin2026!")
}

pub(crate) fn is_admin_account(email: &str) -> bool {
    email.eq_ignore_ascii_case(&admin_email())
}

fn listen_address() -> String {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    format!("0.0.0.0:{port}")
}

fn session_cookie_secure() -> bool {
    ["PUBLIC_APP_URL", "RENDER_EXTERNAL_URL", "PUBLIC_BASE_URL"]
        .into_iter()
        .filter_map(|key| env::var(key).ok())
        .any(|url| url.trim().starts_with("https://"))
}

fn apply_session_cookie_settings(cookie: &mut Cookie<'_>) {
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(Some(CookieDuration::seconds(AUTH_SESSION_MAX_AGE_SECS)));
    if session_cookie_secure() {
        cookie.set_secure(true);
    }
}

fn ensure_user_profile(state: &AppState, email: &str) {
    match state.storage.load_profile(email) {
        Ok(None) => {
            let profile = default_profile(email);
            if let Err(error) = state.storage.save_profile(&profile) {
                eprintln!("failed to seed profile for {email}: {error}");
            }
        }
        Ok(Some(_)) => {}
        Err(error) => eprintln!("failed to check profile for {email}: {error}"),
    }
}

fn smtp_configured() -> bool {
    email_delivery::smtp_configured()
}

fn show_dev_reset_links() -> bool {
    !smtp_configured()
        || env::var("SHOW_RESET_LINKS")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

pub(crate) fn public_base_url() -> String {
    for key in ["PUBLIC_APP_URL", "RENDER_EXTERNAL_URL", "PUBLIC_BASE_URL"] {
        if let Ok(url) = env::var(key) {
            let trimmed = url.trim().trim_end_matches('/').to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    format!("http://localhost:{port}")
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
    cookie.set_max_age(Some(CookieDuration::seconds(LOGIN_PREFILL_MAX_AGE_SECS)));
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

fn paw_points_icon_html() -> &'static str {
    r#"<img src="/images/paw-points-icon.png" alt="" class="paw-points-icon" width="40" height="21" decoding="async" aria-hidden="true" />"#
}

fn paw_points_amount_html(amount: u32) -> String {
    format!(
        r#"<span class="paw-points-amount">{amount} {}</span>"#,
        paw_points_icon_html()
    )
}

pub(crate) fn timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn format_timestamp(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Unknown".to_string();
    }

    DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt| dt.with_timezone(&Utc).format("%b %d, %Y").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn format_member_since(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Recently joined".to_string();
    }

    DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt| dt.with_timezone(&Utc).format("%B %d, %Y").to_string())
        .unwrap_or_else(|| "Recently joined".to_string())
}

fn is_admin_credentials(email: &str, password: &str) -> bool {
    email.eq_ignore_ascii_case(&admin_email()) && password == admin_password()
}

fn admin_session_valid(state: &AppState, jar: &CookieJar) -> bool {
    let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) else {
        return false;
    };

    state
        .storage
        .admin_session_valid(cookie.value())
        .unwrap_or(false)
}

fn create_admin_session(state: &AppState, jar: CookieJar) -> CookieJar {
    let session_id = Uuid::new_v4().to_string();
    let now = timestamp_now();
    let expires_at = now.saturating_add(AUTH_SESSION_MAX_AGE_SECS as u64);
    if let Err(error) = state
        .storage
        .save_auth_session(&session_id, "admin", None, now, expires_at)
    {
        eprintln!("failed to persist admin session: {error}");
    }

    let mut cookie = Cookie::new(ADMIN_SESSION_COOKIE, session_id);
    apply_session_cookie_settings(&mut cookie);
    jar.add(cookie)
}

fn clear_admin_session(state: &AppState, jar: CookieJar) -> CookieJar {
    if let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) {
        if let Err(error) = state.storage.delete_auth_session(cookie.value()) {
            eprintln!("failed to delete admin session: {error}");
        }
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
            let has_pet = profile_has_pet(&profile);
            if profile.onboarding_completed != has_pet {
                profile.onboarding_completed = has_pet;
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

fn ensure_dashboard_session(
    state: &AppState,
    jar: CookieJar,
) -> Result<(CookieJar, String), Redirect> {
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

    Err(Redirect::to("/"))
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
        .storage
        .lookup_user_session(cookie.value())
        .ok()
        .flatten()
}

fn voter_session_email(state: &AppState, jar: &CookieJar) -> Option<String> {
    if let Some(email) = user_session_email(state, jar) {
        return Some(email);
    }
    if admin_session_valid(state, jar) {
        return Some(admin_email());
    }
    None
}

fn auth_nav_link_html(state: &AppState, jar: &CookieJar) -> &'static str {
    if user_session_email(state, jar).is_some() {
        r#"<a href="/home?tab=account">ACCOUNT</a>"#
    } else {
        r#"<a href="/login">LOG IN</a>"#
    }
}

fn apply_auth_nav_link(html: &str, state: &AppState, jar: &CookieJar) -> String {
    let link = auth_nav_link_html(state, jar);
    html.replace("{{AUTH_NAV_LINK}}", link)
        .replace(r#"<a href="/login">LOG IN</a>"#, link)
}

pub(crate) fn user_for_email(state: &AppState, email: &str) -> Option<User> {
    state.storage.find_user_by_email(email).ok().flatten()
}

fn contact_name_for_email(state: &AppState, email: &str) -> Option<String> {
    user_for_email(state, email).map(|user| {
        let full = format!("{} {}", user.first_name.trim(), user.last_name.trim())
            .trim()
            .to_string();
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
        if let Err(error) = state.storage.delete_auth_session(cookie.value()) {
            eprintln!("failed to delete user session: {error}");
        }
    }

    jar.remove(Cookie::from(USER_SESSION_COOKIE))
}

fn user_redirect_if_missing(state: &AppState, jar: &CookieJar) -> Result<String, Redirect> {
    user_session_email(state, jar).ok_or_else(|| Redirect::to("/"))
}

fn api_user_email(state: &AppState, jar: &CookieJar) -> Option<String> {
    if let Some(email) = user_session_email(state, jar) {
        return Some(email);
    }

    if admin_session_valid(state, jar) {
        if let Err(error) = ensure_admin_user_account(state) {
            eprintln!("admin user bootstrap failed: {error}");
        }
        return Some(admin_email());
    }

    None
}

fn api_auth_error(wants_json: bool) -> Response {
    if wants_json {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "ok": false, "status": "auth" })),
        )
            .into_response()
    } else {
        Redirect::to("/").into_response()
    }
}

fn minutes_to_time_input_value(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn parse_time_input(value: &str) -> Option<u16> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hours: u16 = parts[0].parse().ok()?;
    let mins: u16 = parts[1].parse().ok()?;
    if hours > 23 || mins > 59 {
        return None;
    }
    Some(hours * 60 + mins)
}

fn daily_due_label(time_minutes: u16) -> String {
    format!("Daily · {}", format_time_from_minutes(time_minutes))
}

fn sort_tasks_by_time(tasks: &mut [UserTask]) {
    tasks.sort_by(|left, right| {
        left.time_minutes
            .cmp(&right.time_minutes)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn is_custom_task(task_id: &str) -> bool {
    task_id.starts_with(CUSTOM_TASK_ID_PREFIX)
}

fn custom_task_count(profile: &UserProfile) -> usize {
    let active_id = active_pet_id(profile);
    profile
        .tasks
        .iter()
        .filter(|task| is_custom_task(&task.id) && task.pet_id == active_id)
        .count()
}

fn sanitize_custom_task_title(raw: &str) -> Option<String> {
    let title = raw.trim();
    if title.is_empty() {
        return None;
    }
    let title: String = title.chars().take(MAX_CUSTOM_TASK_TITLE_LEN).collect();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

fn default_custom_task_time(profile: &UserProfile, pet_id: &str) -> u16 {
    profile
        .tasks
        .iter()
        .filter(|task| task.pet_id == pet_id)
        .map(|task| task.time_minutes)
        .max()
        .map(|latest| latest.saturating_add(30).min(22 * 60))
        .unwrap_or(12 * 60)
}

fn create_custom_task(
    profile: &UserProfile,
    pet_id: &str,
    title: String,
    today: NaiveDate,
) -> UserTask {
    let time_minutes = default_custom_task_time(profile, pet_id);
    scheduled_task(
        &format!("{CUSTOM_TASK_ID_PREFIX}{}", Uuid::new_v4()),
        &title,
        &daily_due_label(time_minutes),
        time_minutes,
        CUSTOM_TASK_REWARD,
        today,
        pet_id,
    )
}

fn remove_custom_task(profile: &mut UserProfile, task_id: &str, pet_id: &str) -> Option<UserTask> {
    if !is_custom_task(task_id) {
        return None;
    }
    let index = find_task_index(profile, task_id, pet_id)?;
    let task = profile.tasks.remove(index);
    if task.completed {
        profile.paw_points = profile.paw_points.saturating_sub(task.reward);
        reverse_parent_xp_for_reward(profile, task.reward);
    }
    Some(task)
}

fn task_has_adjustable_time(task_id: &str) -> bool {
    is_custom_task(task_id)
        || breed_guides::is_breed_guide_task_id(task_id)
        || matches!(
            task_id,
            "feed_breakfast"
                | "feed_lunch"
                | "feed_afternoon"
                | "feed_dinner"
                | "water_bowl_morning"
                | "water_bowl_night"
                | "litter_check"
                | "play_session"
        )
}

fn feed_schedule_time(schedule: &CareSchedule, task_id: &str) -> Option<u16> {
    match task_id {
        "feed_breakfast" => Some(schedule.feed_time_minutes),
        "feed_lunch" => Some(schedule.feed_lunch_time_minutes),
        "feed_afternoon" => Some(schedule.feed_afternoon_time_minutes),
        "feed_dinner" => Some(schedule.feed_dinner_time_minutes),
        _ => None,
    }
}

fn set_feed_schedule_time(schedule: &mut CareSchedule, task_id: &str, time_minutes: u16) -> bool {
    match task_id {
        "feed_breakfast" => {
            let changed = schedule.feed_time_minutes != time_minutes;
            schedule.feed_time_minutes = time_minutes;
            changed
        }
        "feed_lunch" => {
            let changed = schedule.feed_lunch_time_minutes != time_minutes;
            schedule.feed_lunch_time_minutes = time_minutes;
            changed
        }
        "feed_afternoon" => {
            let changed = schedule.feed_afternoon_time_minutes != time_minutes;
            schedule.feed_afternoon_time_minutes = time_minutes;
            changed
        }
        "feed_dinner" => {
            let changed = schedule.feed_dinner_time_minutes != time_minutes;
            schedule.feed_dinner_time_minutes = time_minutes;
            changed
        }
        _ => false,
    }
}

fn task_schedule_prefix(task_id: &str) -> &'static str {
    if task_id == "play_session" {
        "Today"
    } else {
        "Daily"
    }
}

fn task_due_label_for(task_id: &str, time_minutes: u16) -> String {
    if breed_guides::is_breed_guide_task_id(task_id) {
        return format!(
            "Daily · {}",
            format_time_from_minutes(time_minutes)
        );
    }
    format!(
        "{} · {}",
        task_schedule_prefix(task_id),
        format_time_from_minutes(time_minutes)
    )
}

fn apply_task_time_to_profile(profile: &mut UserProfile, task_id: &str, time_minutes: u16) -> bool {
    if !task_has_adjustable_time(task_id) {
        return false;
    }

    let active_id = active_pet_id(profile).to_string();
    let Some(task) = profile.tasks.iter_mut().find(|task| {
        task.id == task_id && task.pet_id == active_id
    }) else {
        return false;
    };

    let due_label = task_due_label_for(task_id, time_minutes);
    if task.time_minutes != time_minutes || task.due_label != due_label {
        task.time_minutes = time_minutes;
        task.due_label = due_label;
    }

    if active_id == PRIMARY_PET_ID {
        let schedule = &mut profile.care_schedule;
        if feed_schedule_time(schedule, task_id).is_some() {
            set_feed_schedule_time(schedule, task_id, time_minutes);
        } else {
            match task_id {
                "water_bowl_morning" => schedule.water_morning_time_minutes = time_minutes,
                "water_bowl_night" => schedule.water_evening_time_minutes = time_minutes,
                "litter_check" => schedule.litter_time_minutes = time_minutes,
                "play_session" => schedule.play_time_minutes = time_minutes,
                _ => {}
            }
        }
    } else if let Some(pet) = profile
        .additional_pets
        .iter_mut()
        .find(|pet| pet.id == active_id)
    {
        let schedule = &mut pet.care_schedule;
        if feed_schedule_time(schedule, task_id).is_some() {
            set_feed_schedule_time(schedule, task_id, time_minutes);
        } else {
            match task_id {
                "water_bowl_morning" => schedule.water_morning_time_minutes = time_minutes,
                "water_bowl_night" => schedule.water_evening_time_minutes = time_minutes,
                "litter_check" => schedule.litter_time_minutes = time_minutes,
                "play_session" => schedule.play_time_minutes = time_minutes,
                _ => {}
            }
        }
    }

    sort_tasks_by_time(&mut profile.tasks);
    true
}

pub(crate) fn scheduled_task(
    id: &str,
    title: &str,
    due_label: &str,
    time_minutes: u16,
    reward: u32,
    today: NaiveDate,
    pet_id: &str,
) -> UserTask {
    UserTask {
        id: id.to_string(),
        title: title.to_string(),
        completed: false,
        due_label: due_label.to_string(),
        due_day: Some(today.day()),
        due_month: Some(today.month()),
        due_year: Some(today.year() as u32),
        time_minutes,
        reward,
        pet_id: pet_id.to_string(),
    }
}

fn default_starter_tasks(snapshot: &PetSnapshot, schedule: &CareSchedule) -> Vec<UserTask> {
    let today = Local::now().date_naive();
    let plan = feeding_plan_for_snapshot(snapshot, today);
    let pet_id = snapshot.id.as_str();
    let mut tasks = feeding_specs_for_plan(plan, schedule)
        .into_iter()
        .map(|(id, title, time_minutes, reward)| {
            scheduled_task(
                id,
                title,
                &daily_due_label(time_minutes),
                time_minutes,
                reward,
                today,
                pet_id,
            )
        })
        .collect::<Vec<_>>();

    tasks.extend([
        scheduled_task(
            "water_bowl_morning",
            "Fill water bowl",
            &daily_due_label(schedule.water_morning_time_minutes),
            schedule.water_morning_time_minutes,
            12,
            today,
            pet_id,
        ),
        scheduled_task(
            "play_session",
            "15-minute play session",
            &task_due_label_for("play_session", schedule.play_time_minutes),
            schedule.play_time_minutes,
            20,
            today,
            pet_id,
        ),
        scheduled_task(
            "litter_check",
            "Refresh litter box",
            &daily_due_label(schedule.litter_time_minutes),
            schedule.litter_time_minutes,
            10,
            today,
            pet_id,
        ),
        scheduled_task(
            "replace_litter",
            "Replace litter",
            "Weekly · anytime",
            default_task_time_minutes(),
            50,
            today,
            pet_id,
        ),
        scheduled_task(
            "water_bowl_night",
            "Fill water bowl (evening)",
            &daily_due_label(schedule.water_evening_time_minutes),
            schedule.water_evening_time_minutes,
            12,
            today,
            pet_id,
        ),
    ]);

    tasks
}

fn apply_care_schedule_to_pet_tasks(profile: &mut UserProfile, snapshot: &PetSnapshot) -> bool {
    if snapshot.deceased {
        return false;
    }
    let schedule = snapshot.care_schedule.clone();
    let today = Local::now().date_naive();
    let plan = feeding_plan_for_snapshot(snapshot, today);
    let feeding_specs: std::collections::HashMap<&str, (&str, u16, u32)> =
        feeding_specs_for_plan(plan, &schedule)
            .into_iter()
            .map(|(id, title, time, reward)| (id, (title, time, reward)))
            .collect();
    let pet_id = snapshot.id.as_str();
    let mut changed = false;

    for task in &mut profile.tasks {
        if task.pet_id != pet_id {
            continue;
        }

        if let Some((title, time_minutes, reward)) = feeding_specs.get(task.id.as_str()) {
            let due_label = daily_due_label(*time_minutes);
            if task.title != *title
                || task.time_minutes != *time_minutes
                || task.due_label != due_label
                || task.reward != *reward
            {
                task.title = (*title).to_string();
                task.time_minutes = *time_minutes;
                task.due_label = due_label;
                task.reward = *reward;
                changed = true;
            }
            continue;
        }

        let (time_minutes, due_label) = match task.id.as_str() {
            "water_bowl_morning" => (
                schedule.water_morning_time_minutes,
                daily_due_label(schedule.water_morning_time_minutes),
            ),
            "water_bowl_night" => (
                schedule.water_evening_time_minutes,
                daily_due_label(schedule.water_evening_time_minutes),
            ),
            "litter_check" => (
                schedule.litter_time_minutes,
                daily_due_label(schedule.litter_time_minutes),
            ),
            "play_session" => (
                schedule.play_time_minutes,
                task_due_label_for("play_session", schedule.play_time_minutes),
            ),
            _ => continue,
        };

        if task.time_minutes != time_minutes || task.due_label != due_label {
            task.time_minutes = time_minutes;
            task.due_label = due_label;
            changed = true;
        }
    }

    changed
}

fn apply_care_schedule_to_tasks(profile: &mut UserProfile) -> bool {
    let mut changed = false;
    for (pet_id, _) in list_pet_summaries(profile) {
        let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
            continue;
        };
        if apply_care_schedule_to_pet_tasks(profile, &snapshot) {
            changed = true;
        }
    }
    changed
}

fn care_calendar_event(date: NaiveDate, title: &str, time_minutes: u16) -> CalendarEvent {
    CalendarEvent {
        id: None,
        day: date.day(),
        month: date.month(),
        year: date.year() as u32,
        title: title.to_string(),
        time_label: format_time_from_minutes(time_minutes),
        time_minutes,
    }
}

pub(crate) fn generate_daily_care_calendar_events_for_snapshot(
    snapshot: &PetSnapshot,
    today: NaiveDate,
    horizon: NaiveDate,
) -> Vec<CalendarEvent> {
    let schedule = &snapshot.care_schedule;
    let plan = feeding_plan_for_snapshot(snapshot, today);
    let pet_name = if snapshot.pet_name.trim().is_empty() {
        "your cat".to_string()
    } else {
        snapshot.pet_name.clone()
    };

    let mut items: Vec<(u16, String)> = feeding_specs_for_plan(plan, schedule)
        .into_iter()
        .map(|(_id, title, time_minutes, _reward)| {
            let label = if title.contains("feeding") {
                format!("Feed {pet_name}")
            } else {
                title.to_string()
            };
            (time_minutes, label)
        })
        .collect();

    items.extend([
        (
            schedule.water_morning_time_minutes,
            format!("Fill water bowl ({pet_name})"),
        ),
        (
            schedule.litter_time_minutes,
            format!("Refresh litter box ({pet_name})"),
        ),
        (
            schedule.water_evening_time_minutes,
            format!("Fill water bowl — evening ({pet_name})"),
        ),
        (
            schedule.play_time_minutes,
            format!("15-minute play session ({pet_name})"),
        ),
    ]);

    let mut events = Vec::new();
    let mut cursor = today;
    while cursor <= horizon {
        for (time_minutes, title) in &items {
            events.push(care_calendar_event(cursor, title, *time_minutes));
        }
        let Some(next) = cursor.succ_opt() else {
            break;
        };
        cursor = next;
    }

    events
}

fn generate_daily_care_calendar_events(
    profile: &UserProfile,
    today: NaiveDate,
    horizon: NaiveDate,
) -> Vec<CalendarEvent> {
    let mut events = Vec::new();
    for (pet_id, _) in list_pet_summaries(profile) {
        let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
            continue;
        };
        events.extend(generate_daily_care_calendar_events_for_snapshot(
            &snapshot,
            today,
            horizon,
        ));
    }
    events
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

fn default_owned_decor() -> Vec<String> {
    vec!["sunny_nook".to_string(), "soft_mat".to_string()]
}

fn default_equipped_decor() -> HashMap<String, String> {
    HashMap::from([
        ("room".to_string(), "sunny_nook".to_string()),
        ("rug".to_string(), "soft_mat".to_string()),
    ])
}

fn default_owned_tap_boosts() -> Vec<String> {
    vec!["basic_pets".to_string()]
}

fn default_equipped_tap_boost() -> String {
    "basic_pets".to_string()
}

pub(crate) fn default_profile(email: &str) -> UserProfile {
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
        pet_birth_date: None,
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
        care_schedule: default_care_schedule(),
        tasks: vec![],
        calendar_events: vec![],
        user_calendar_events: vec![],
        activity: vec![],
        stripe_customer_id: None,
        pet_photo_url: None,
        pet_video_url: None,
        pet_video_clip_start: None,
        pet_video_clip_duration: None,
        pet_video_zoom: None,
        pet_video_offset_x: None,
        pet_video_offset_y: None,
        deceased: false,
        deceased_at: None,
        memorial_videos: Vec::new(),
        memorial_comfort_seen: false,
        pending_purrfect_idea_ids: vec![],
        owned_decor: default_owned_decor(),
        equipped_decor: default_equipped_decor(),
        owned_tap_boosts: default_owned_tap_boosts(),
        equipped_tap_boost: default_equipped_tap_boost(),
        petting_bonus_inventory: HashMap::new(),
        active_petting_bonus: String::new(),
        petting_bonus_expires_at: 0,
        owned_breed_guides: Vec::new(),
        premium_unlocked: false,
        additional_pets: Vec::new(),
        active_pet_id: PRIMARY_PET_ID.to_string(),
        active_pet_owner_email: None,
        care_streak_days: 0,
        care_streak_last_date: None,
        best_care_streak: 0,
        claimed_streak_rewards: Vec::new(),
        community_visible: true,
        notification_prefs: push_notifications::NotificationPrefs::default(),
        notification_sent_dates: HashMap::new(),
        onboarding_emails_enabled: true,
        onboarding_emails_sent: Vec::new(),
    }
}

fn user_has_premium(profile: &UserProfile) -> bool {
    entitlements::has_premium(profile.premium_unlocked, &profile.email)
}

fn admin_profile(email: &str) -> UserProfile {
    let mut profile = default_profile(email);
    profile.tasks = vec![];
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
    pet_id: Option<&str>,
) -> Result<String, storage::StorageError> {
    let uploads_dir = pet_uploads_dir(state);
    fs::create_dir_all(&uploads_dir).await?;

    let basename = email_upload_basename(email);
    let suffix = pet_id
        .filter(|id| *id != PRIMARY_PET_ID)
        .map(|id| format!("-{id}"))
        .unwrap_or_default();
    let filename = format!("{basename}{suffix}.{ext}");
    let disk_path = uploads_dir.join(&filename);
    fs::write(&disk_path, bytes).await?;

    Ok(format!("/uploads/{filename}"))
}

fn apply_pet_photo_url(profile: &mut UserProfile, pet_id: &str, url: String) {
    if pet_id == PRIMARY_PET_ID {
        profile.pet_photo_url = Some(url);
    } else if let Some(pet) = profile
        .additional_pets
        .iter_mut()
        .find(|pet| pet.id == pet_id)
    {
        pet.pet_photo_url = Some(url);
    }
}

fn detect_video_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 12 {
        return None;
    }
    if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("webm");
    }
    if &bytes[4..8] == b"ftyp" {
        return Some("mp4");
    }
    None
}

fn allowed_pet_video_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "video/mp4" | "video/webm" | "video/quicktime" | "video/x-m4v"
    )
}

fn validate_pet_video(content_type: Option<&str>, bytes: &[u8]) -> Result<&'static str, ()> {
    if bytes.is_empty() {
        return Err(());
    }
    if bytes.len() > MAX_PET_VIDEO_BYTES {
        return Err(());
    }
    let ext = detect_video_ext(bytes).ok_or(())?;
    if let Some(content_type) = content_type {
        if !allowed_pet_video_content_type(content_type) {
            return Err(());
        }
    }
    Ok(ext)
}

fn parse_pet_video_clip_start(value: &str) -> f32 {
    value.trim().parse::<f32>().unwrap_or(0.0).max(0.0)
}

fn parse_optional_video_float(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

fn pet_video_framing_data_attrs(
    zoom: Option<f32>,
    offset_x: Option<f32>,
    offset_y: Option<f32>,
) -> String {
    let Some(zoom) = zoom.filter(|value| *value > 0.0) else {
        return String::new();
    };
    format!(
        r#" data-video-zoom="{zoom}" data-video-offset-x="{offset_x}" data-video-offset-y="{offset_y}""#,
        zoom = zoom,
        offset_x = offset_x.unwrap_or(0.0),
        offset_y = offset_y.unwrap_or(0.0),
    )
}

fn parse_pet_video_clip_duration(value: &str) -> f32 {
    let parsed = value
        .trim()
        .parse::<f32>()
        .unwrap_or(PET_VIDEO_CLIP_MAX_SECONDS);
    parsed.clamp(PET_VIDEO_CLIP_MIN_SECONDS, PET_VIDEO_CLIP_MAX_SECONDS)
}

fn pet_video_clip_duration(profile: &UserProfile) -> f32 {
    profile
        .pet_video_clip_duration
        .unwrap_or(PET_VIDEO_CLIP_MAX_SECONDS)
        .clamp(PET_VIDEO_CLIP_MIN_SECONDS, PET_VIDEO_CLIP_MAX_SECONDS)
}

fn format_pet_video_clip_duration_label(seconds: f32) -> String {
    let rounded = (seconds * 10.0).round() / 10.0;
    if (rounded - rounded.round()).abs() < f32::EPSILON {
        format!("{}s", rounded.round() as u32)
    } else {
        format!("{rounded:.1}s")
    }
}

async fn save_pet_video(
    state: &AppState,
    email: &str,
    bytes: &[u8],
    ext: &str,
    pet_id: Option<&str>,
) -> Result<String, storage::StorageError> {
    let uploads_dir = pet_uploads_dir(state);
    fs::create_dir_all(&uploads_dir).await?;

    let basename = email_upload_basename(email);
    let suffix = pet_id
        .filter(|id| *id != PRIMARY_PET_ID)
        .map(|id| format!("-{id}"))
        .unwrap_or_default();
    let filename = format!("{basename}-playing{suffix}.{ext}");
    let disk_path = uploads_dir.join(&filename);
    fs::write(&disk_path, bytes).await?;

    Ok(format!("/uploads/{filename}"))
}

async fn save_memorial_video(
    state: &AppState,
    email: &str,
    bytes: &[u8],
    ext: &str,
    pet_id: &str,
    slot: usize,
) -> Result<String, storage::StorageError> {
    let uploads_dir = pet_uploads_dir(state);
    fs::create_dir_all(&uploads_dir).await?;

    let basename = email_upload_basename(email);
    let suffix = if pet_id == PRIMARY_PET_ID {
        String::new()
    } else {
        format!("-{pet_id}")
    };
    let filename = format!("{basename}-memorial{suffix}-{slot}.{ext}");
    let disk_path = uploads_dir.join(&filename);
    fs::write(&disk_path, bytes).await?;

    Ok(format!("/uploads/{filename}"))
}

fn pet_video_clip_start(profile: &UserProfile) -> f32 {
    profile.pet_video_clip_start.unwrap_or(0.0).max(0.0)
}

fn snapshot_has_pet_video(snapshot: &PetSnapshot) -> bool {
    snapshot
        .pet_video_url
        .as_deref()
        .is_some_and(|value| !value.is_empty())
}

fn profile_has_pet_video(profile: &UserProfile) -> bool {
    active_pet_snapshot(profile).is_some_and(|pet| snapshot_has_pet_video(&pet))
}

fn pet_video_clip_start_for_snapshot(snapshot: &PetSnapshot) -> f32 {
    snapshot.pet_video_clip_start.unwrap_or(0.0).max(0.0)
}

fn pet_video_clip_duration_for_snapshot(snapshot: &PetSnapshot) -> f32 {
    snapshot
        .pet_video_clip_duration
        .unwrap_or(PET_VIDEO_CLIP_MAX_SECONDS)
        .clamp(PET_VIDEO_CLIP_MIN_SECONDS, PET_VIDEO_CLIP_MAX_SECONDS)
}

fn render_pet_user_video_optional(snapshot: &PetSnapshot) -> String {
    let Some(url) = snapshot
        .pet_video_url
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return String::new();
    };
    let name = escape_html(&snapshot.pet_name);
    let clip_start = pet_video_clip_start_for_snapshot(snapshot);
    let clip_duration = pet_video_clip_duration_for_snapshot(snapshot);
    let framing_attrs = pet_video_framing_data_attrs(
        snapshot.pet_video_zoom,
        snapshot.pet_video_offset_x,
        snapshot.pet_video_offset_y,
    );
    format!(
        r#"<div class="pet-user-video-optional" hidden>
      <video
        class="pet-user-video-player"
        src="{url}"
        muted
        playsinline
        webkit-playsinline
        preload="auto"
        data-clip-start="{clip_start}"
        data-clip-duration="{clip_duration}"{framing_attrs}
        aria-label="Video of {name} playing"
      ></video>
    </div>"#,
        url = escape_html_attr(url),
        name = name,
        clip_start = clip_start,
        clip_duration = clip_duration,
        framing_attrs = framing_attrs,
    )
}

fn render_pet_avatar(profile: &UserProfile) -> String {
    if memorial::active_pet_is_deceased(profile) {
        return memorial::render_angel_pet_avatar(profile);
    }

    let snapshot = active_pet_snapshot(profile);
    let pet_name_raw = snapshot
        .as_ref()
        .map(|pet| pet.pet_name.as_str())
        .unwrap_or(profile.pet_name.as_str());
    let pet_name = escape_html(pet_name_raw);
    let display_name = if pet_name_raw.trim().is_empty() {
        "Cinder".to_string()
    } else {
        pet_name.clone()
    };
    let has_video = snapshot
        .as_ref()
        .is_some_and(|pet| snapshot_has_pet_video(pet));
    let photo_toggle = if has_video {
        r#"<button type="button" class="cinder-photo-toggle" aria-pressed="false">Watch my kitty play! 🎬</button>"#
    } else {
        ""
    };
    let user_photo = snapshot
        .as_ref()
        .map(render_pet_user_video_optional)
        .unwrap_or_default();
    let pet_id = escape_html_attr(active_pet_id(profile));
    format!(
        r#"<div class="pet-cinder-stage" id="cinder-pet-stage" data-pet-name="{display_name}" data-pet-id="{pet_id}">
      <p class="cinder-pet-label">{display_name}</p>
      <div class="cinder-pet-image-wrap">
        <img class="cinder-pet-image" src="/cinderanimate.png" alt="{display_name} virtual pet" />
        {user_photo}
      </div>
      {photo_toggle}
    </div>"#,
        display_name = display_name,
        pet_id = pet_id,
        user_photo = user_photo,
        photo_toggle = photo_toggle,
    )
}

async fn save_profile(
    state: &AppState,
    profile: &UserProfile,
) -> Result<(), storage::StorageError> {
    state.storage.save_profile(profile)
}

async fn get_or_create_profile(state: &AppState, email: &str) -> UserProfile {
    let mut profile = if let Ok(Some(mut profile)) = state.storage.load_profile(email) {
        profile.email = email.to_string();
        profile
    } else if is_admin_account(email) {
        admin_profile(email)
    } else {
        default_profile(email)
    };

    let mut profile_changed = false;
    if normalize_profile_pets(&mut profile) {
        profile_changed = true;
    }
    let prefs_before = profile.notification_prefs.daily_checkin_times.clone();
    push_notifications::normalize_notification_prefs(&mut profile.notification_prefs);
    if profile.notification_prefs.daily_checkin_times != prefs_before {
        profile_changed = true;
    }
    if profile_changed {
        let _ = save_profile(state, &profile).await;
    }

    let has_pet = profile_has_pet(&profile);
    if profile.onboarding_completed != has_pet {
        profile.onboarding_completed = has_pet;
        let _ = save_profile(state, &profile).await;
    }

    if refresh_profile_tasks(&mut profile) {
        let _ = save_profile(state, &profile).await;
    }

    if ensure_tap_boost_state(&mut profile) {
        let _ = save_profile(state, &profile).await;
    }

    profile
}

pub(crate) fn is_daily_reset_task(task: &UserTask) -> bool {
    if is_custom_task(&task.id) {
        return true;
    }

    if breed_guides::is_breed_guide_task_id(&task.id) {
        return true;
    }

    if task.id == "replace_litter" {
        return false;
    }

    if task.id == VET_APPOINTMENT_TASK_ID {
        return true;
    }

    if FEEDING_TASK_IDS.contains(&task.id.as_str()) {
        return true;
    }

    matches!(
        task.id.as_str(),
        "water_bowl_morning" | "water_bowl_night" | "litter_check" | "play_session"
    )
}

fn task_due_date(task: &UserTask) -> Option<NaiveDate> {
    match (task.due_year, task.due_month, task.due_day) {
        (Some(year), Some(month), Some(day)) => NaiveDate::from_ymd_opt(year as i32, month, day),
        _ => None,
    }
}

fn refresh_daily_task_schedule(task: &mut UserTask, today: NaiveDate) -> bool {
    if !is_daily_reset_task(task) {
        return false;
    }

    let mut changed = false;
    let scheduled_for = task_due_date(task);

    if let Some(date) = scheduled_for {
        if date < today {
            if task.completed {
                task.completed = false;
            }
            task.due_day = Some(today.day());
            task.due_month = Some(today.month());
            task.due_year = Some(today.year() as u32);
            if task.id == VET_APPOINTMENT_TASK_ID {
                task.due_label = "Daily · urgent".to_string();
            }
            changed = true;
        }
    } else if task.due_day.is_none() && task.due_month.is_none() && task.due_year.is_none() {
        task.due_day = Some(today.day());
        task.due_month = Some(today.month());
        task.due_year = Some(today.year() as u32);
        if task.id == VET_APPOINTMENT_TASK_ID {
            task.due_label = "Daily · urgent".to_string();
        }
        changed = true;
    }

    changed
}

fn vet_appointment_task(today: NaiveDate, pet_id: &str) -> UserTask {
    UserTask {
        id: VET_APPOINTMENT_TASK_ID.to_string(),
        title: "Make vet appointment ASAP".to_string(),
        completed: false,
        due_label: "Daily · urgent".to_string(),
        due_day: Some(today.day()),
        due_month: Some(today.month()),
        due_year: Some(today.year() as u32),
        time_minutes: default_task_time_minutes(),
        reward: 30,
        pet_id: pet_id.to_string(),
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
            if felv_booster <= today && !is_dose_satisfied(VaccineKind::Felv, felv_booster, history)
            {
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

fn is_outdoor_snapshot(snapshot: &PetSnapshot) -> bool {
    snapshot
        .pet_indoor_outdoor
        .as_deref()
        .is_some_and(|value| value == "outdoor")
}

fn vaccines_due_or_overdue_for_snapshot(snapshot: &PetSnapshot, today: NaiveDate) -> bool {
    let Some(birth) = pet_birth_date_for_snapshot(snapshot, today) else {
        return false;
    };

    let history = &snapshot.vaccine_history;

    if let Some(weeks) = snapshot.pet_age_weeks {
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
            if felv_booster <= today && !is_dose_satisfied(VaccineKind::Felv, felv_booster, history)
            {
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

    let felv_interval = if is_outdoor_snapshot(snapshot) {
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

    false
}

fn needs_vet_appointment_asap_for_snapshot(
    profile: &UserProfile,
    snapshot: &PetSnapshot,
    today: NaiveDate,
) -> bool {
    if !entitlements::can_access_health_records(profile.premium_unlocked, &profile.email)
        || is_admin_account(&profile.email)
    {
        return false;
    }

    if snapshot.never_been_to_vet {
        return true;
    }

    if snapshot.pet_vaccines_unknown {
        return true;
    }

    if snapshot.last_vet_date.is_none() {
        return true;
    }

    vaccines_due_or_overdue_for_snapshot(snapshot, today)
}

pub(crate) fn needs_vet_appointment_asap(profile: &UserProfile, today: NaiveDate) -> bool {
    pet_snapshot(profile, PRIMARY_PET_ID)
        .is_some_and(|snapshot| needs_vet_appointment_asap_for_snapshot(profile, &snapshot, today))
}

fn ensure_starter_care_tasks_for_pet(profile: &mut UserProfile, snapshot: &PetSnapshot) -> bool {
    if snapshot.deceased {
        return false;
    }
    let pet_id = snapshot.id.as_str();
    let starters = default_starter_tasks(snapshot, &snapshot.care_schedule);
    let expected_ids: std::collections::HashSet<String> =
        starters.iter().map(|task| task.id.clone()).collect();

    let mut changed = false;
    let before_len = profile.tasks.len();
    profile.tasks.retain(|task| {
        task.pet_id != pet_id
            || !FEEDING_TASK_IDS.contains(&task.id.as_str())
            || expected_ids.contains(&task.id)
    });
    if profile.tasks.len() != before_len {
        changed = true;
    }

    for starter in starters {
        if let Some(task) = profile
            .tasks
            .iter_mut()
            .find(|task| task.id == starter.id && task.pet_id == pet_id)
        {
            if task.reward != starter.reward {
                task.reward = starter.reward;
                changed = true;
            }
            if task.title != starter.title {
                task.title = starter.title.clone();
                changed = true;
            }
            continue;
        }
        profile.tasks.push(starter);
        changed = true;
    }
    changed
}

fn ensure_starter_care_tasks(profile: &mut UserProfile) -> bool {
    let summaries = list_pet_summaries(profile);
    if summaries.is_empty() {
        return false;
    }

    let mut changed = false;
    for (pet_id, _) in summaries {
        let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
            continue;
        };
        if ensure_starter_care_tasks_for_pet(profile, &snapshot) {
            changed = true;
        }
    }
    changed
}

pub(crate) fn pet_ids_for_breed_name(profile: &UserProfile, breed_name: &str) -> Vec<String> {
    list_pet_summaries(profile)
        .into_iter()
        .filter_map(|(pet_id, _)| {
            pet_snapshot(profile, &pet_id).and_then(|snapshot| {
                if snapshot
                    .pet_breed
                    .trim()
                    .eq_ignore_ascii_case(breed_name.trim())
                {
                    Some(pet_id)
                } else {
                    None
                }
            })
        })
        .collect()
}

fn build_breed_guide_task(
    template: &breed_guides::BreedGuideTaskTemplate,
    slug: &str,
    pet_id: &str,
    today: NaiveDate,
) -> UserTask {
    let id = breed_guides::breed_guide_task_id(slug, template.key);
    let due_label = task_due_label_for(&id, template.time_minutes);
    scheduled_task(
        &id,
        &template.title,
        &due_label,
        template.time_minutes,
        template.reward,
        today,
        pet_id,
    )
}

pub(crate) fn accessible_breed_guide_slugs(profile: &UserProfile) -> HashSet<String> {
    let mut slugs: HashSet<String> = profile
        .owned_breed_guides
        .iter()
        .map(|slug| slug.trim().to_lowercase())
        .collect();

    if entitlements::has_premium(profile.premium_unlocked, &profile.email) {
        for (pet_id, _) in list_pet_summaries(profile) {
            let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
                continue;
            };
            if let Some(guide) = breed_guides::guide_for_breed_name(&snapshot.pet_breed) {
                slugs.insert(guide.slug.to_lowercase());
            }
        }
    }

    slugs
}

pub(crate) fn ensure_breed_guide_tasks(profile: &mut UserProfile) -> bool {
    let today = Local::now().date_naive();
    let owned_slugs = accessible_breed_guide_slugs(profile);
    let mut changed = false;
    let pet_breeds: HashMap<String, String> = list_pet_summaries(profile)
        .into_iter()
        .filter_map(|(pet_id, _)| {
            pet_snapshot(profile, &pet_id).map(|snapshot| (pet_id, snapshot.pet_breed.clone()))
        })
        .collect();
    let before_len = profile.tasks.len();
    profile.tasks.retain(|task| {
        if !breed_guides::is_breed_guide_task_id(&task.id) {
            return true;
        }
        let Some(slug) = breed_guides::slug_from_breed_guide_task_id(&task.id) else {
            return false;
        };
        if !owned_slugs.contains(&slug.to_lowercase()) {
            return false;
        }
        let Some(guide) = breed_guides::guide_for_slug(&slug) else {
            return false;
        };
        pet_breeds.get(&task.pet_id).is_some_and(|breed| {
            breed
                .trim()
                .eq_ignore_ascii_case(guide.breed_name.trim())
        })
    });
    if profile.tasks.len() != before_len {
        changed = true;
    }

    for slug in owned_slugs.iter() {
        let Some(guide) = breed_guides::guide_for_slug(slug) else {
            continue;
        };
        let templates = breed_guides::task_templates_for_guide(&guide);
        for pet_id in pet_ids_for_breed_name(profile, &guide.breed_name) {
            if memorial::pet_is_deceased(profile, &pet_id) {
                continue;
            }
            for template in &templates {
                let task_id = breed_guides::breed_guide_task_id(&guide.slug, template.key);
                if let Some(task) = profile
                    .tasks
                    .iter_mut()
                    .find(|task| task.id == task_id && task.pet_id == pet_id)
                {
                    if task.title != template.title {
                        task.title = template.title.clone();
                        changed = true;
                    }
                    if task.reward != template.reward {
                        task.reward = template.reward;
                        changed = true;
                    }
                    continue;
                }
                profile.tasks.push(build_breed_guide_task(
                    template,
                    &guide.slug,
                    &pet_id,
                    today,
                ));
                changed = true;
            }
        }
    }

    changed
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next_month
        .and_then(|date| date.pred_opt())
        .map(|date| date.day())
        .unwrap_or(28)
}

fn add_months_to_date(date: NaiveDate, months: u32) -> NaiveDate {
    let mut month = date.month() as i32 + months as i32;
    let mut year = date.year();
    while month > 12 {
        month -= 12;
        year += 1;
    }
    while month < 1 {
        month += 12;
        year -= 1;
    }
    let month = u32::try_from(month).unwrap_or(1);
    let day = date.day().min(last_day_of_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or(date)
}

fn generate_breed_guide_calendar_events(
    profile: &UserProfile,
    today: NaiveDate,
    horizon: NaiveDate,
) -> Vec<CalendarEvent> {
    let mut events = Vec::new();

    for slug in accessible_breed_guide_slugs(profile) {
        let Some(guide) = breed_guides::guide_for_slug(&slug) else {
            continue;
        };
        if pet_ids_for_breed_name(profile, &guide.breed_name).is_empty() {
            continue;
        }

        let interval = breed_guides::wellness_exam_interval_months(&guide);
        let title = format!("{} breed wellness exam", guide.breed_name);
        let mut cursor = today;
        while cursor <= horizon {
            events.push(care_calendar_event(cursor, &title, 600));
            cursor = add_months_to_date(cursor, interval);
        }
    }

    events
}

pub(crate) fn refresh_profile_tasks(profile: &mut UserProfile) -> bool {
    if list_pet_summaries(profile).is_empty() {
        if profile.tasks.is_empty() {
            return false;
        }
        profile.tasks.clear();
        return true;
    }

    let today = Local::now().date_naive();
    let mut changed = ensure_starter_care_tasks(profile);
    if memorial::ensure_memorial_tasks(profile) {
        changed = true;
    }
    if ensure_breed_guide_tasks(profile) {
        changed = true;
    }
    if apply_care_schedule_to_tasks(profile) {
        changed = true;
    }

    for task in &mut profile.tasks {
        if refresh_daily_task_schedule(task, today) {
            changed = true;
        }
    }

    for (pet_id, _) in list_pet_summaries(profile) {
        let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
            continue;
        };
        if snapshot.deceased {
            continue;
        }
        let needs_vet = needs_vet_appointment_asap_for_snapshot(profile, &snapshot, today);
        let has_vet_task = profile.tasks.iter().any(|task| {
            task.id == VET_APPOINTMENT_TASK_ID && task.pet_id == pet_id
        });

        if needs_vet && !has_vet_task {
            profile
                .tasks
                .push(vet_appointment_task(today, &pet_id));
            changed = true;
        } else if !needs_vet && has_vet_task {
            profile.tasks.retain(|task| {
                !(task.id == VET_APPOINTMENT_TASK_ID && task.pet_id == pet_id)
            });
            changed = true;
        }
    }

    let order_before: Vec<String> = profile.tasks.iter().map(|task| task.id.clone()).collect();
    sort_tasks_by_time(&mut profile.tasks);
    let order_after: Vec<String> = profile.tasks.iter().map(|task| task.id.clone()).collect();
    if order_before != order_after {
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

const XP_PER_LEVEL: u32 = 100;

fn level_progress(profile: &UserProfile) -> (u32, String) {
    let progress = (profile.parent_xp * 100) / XP_PER_LEVEL;
    let remaining = XP_PER_LEVEL.saturating_sub(profile.parent_xp);
    let text = if remaining == 0 {
        "Ready to level up! Complete more tasks.".to_string()
    } else {
        format!(
            "{remaining} XP to reach level {}.",
            profile.parent_level + 1
        )
    };
    (progress.min(100), text)
}

fn task_rewards_earned(profile: &UserProfile) -> u32 {
    profile
        .tasks
        .iter()
        .filter(|task| task.completed)
        .map(|task| task.reward)
        .sum()
}

fn task_xp_earned(profile: &UserProfile) -> u32 {
    task_rewards_earned(profile) / 2
}

fn reverse_parent_xp_for_reward(profile: &mut UserProfile, reward: u32) {
    let mut xp_to_remove = reward / 2;
    while xp_to_remove > 0 {
        if profile.parent_xp >= xp_to_remove {
            profile.parent_xp -= xp_to_remove;
            return;
        }
        xp_to_remove -= profile.parent_xp;
        profile.parent_xp = 0;
        if profile.parent_level <= 1 {
            return;
        }
        profile.parent_level -= 1;
        profile.parent_xp = XP_PER_LEVEL.saturating_sub(xp_to_remove);
        return;
    }
}

fn reopen_completed_task(profile: &mut UserProfile, index: usize) -> (String, u32) {
    let reward = profile.tasks[index].reward;
    let title = profile.tasks[index].title.clone();
    profile.tasks[index].completed = false;
    profile.paw_points = profile.paw_points.saturating_sub(reward);
    reverse_parent_xp_for_reward(profile, reward);
    (title, reward)
}

fn outfit_points_spent(profile: &UserProfile) -> u32 {
    profile
        .owned_outfits
        .iter()
        .filter_map(|id| outfit_by_id(id))
        .map(|outfit| outfit.price)
        .sum()
}

fn decor_points_spent(profile: &UserProfile) -> u32 {
    profile
        .owned_decor
        .iter()
        .filter_map(|id| decor_by_id(id))
        .map(|decor| decor.price)
        .sum()
}

fn shop_points_spent(profile: &UserProfile) -> u32 {
    outfit_points_spent(profile) + decor_points_spent(profile)
}

fn purchased_paw_points(profile: &UserProfile) -> u32 {
    profile
        .activity
        .iter()
        .filter_map(|item| {
            let message = item.message.as_str();
            if !message.contains("Purchased") || !message.contains("paw points") {
                return None;
            }
            message
                .split_whitespace()
                .find_map(|word| word.parse::<u32>().ok())
        })
        .sum()
}

fn lifetime_xp(profile: &UserProfile) -> u32 {
    profile
        .parent_level
        .saturating_sub(1)
        .saturating_mul(XP_PER_LEVEL)
        .saturating_add(profile.parent_xp)
}

fn render_parent_level_breakdown(profile: &UserProfile) -> String {
    let xp_from_tasks = task_xp_earned(profile);
    let paw_from_tasks = task_rewards_earned(profile);
    let paw_spent = shop_points_spent(profile);
    let paw_purchased = purchased_paw_points(profile);
    let lifetime = lifetime_xp(profile);
    let xp_remaining = XP_PER_LEVEL.saturating_sub(profile.parent_xp);
    let progress_pct = (profile.parent_xp * 100) / XP_PER_LEVEL;

    format!(
        r#"<div class="onboarding-backdrop parent-level-backdrop" id="parent-level-modal" role="dialog" aria-modal="true" aria-labelledby="parent-level-title" hidden>
  <div class="onboarding-modal parent-level-modal">
    <button type="button" class="parent-level-close" id="parent-level-close" aria-label="Close breakdown">&times;</button>
    <h2 id="parent-level-title">Parent Level {level} Breakdown</h2>
    <p class="parent-level-intro">Track your XP progress and paw points earned as a cat parent.</p>
    <div class="parent-level-sections">
      <section class="parent-level-section">
        <h3>XP</h3>
        <dl class="parent-level-dl">
          <dt>Current level</dt>
          <dd>Level {level}</dd>
          <dt>XP this level</dt>
          <dd>{parent_xp} / {xp_per_level}</dd>
          <dt>Progress</dt>
          <dd>
            <div class="level-bar" aria-hidden="true"><span class="level-fill" style="width: {progress_pct}%"></span></div>
            <span class="parent-level-progress-text">{xp_remaining} XP to level {next_level}</span>
          </dd>
          <dt>Lifetime XP</dt>
          <dd>{lifetime}</dd>
          <dt>From care tasks</dt>
          <dd>+{xp_from_tasks} XP</dd>
        </dl>
      </section>
      <section class="parent-level-section">
        <h3>Paw Points</h3>
        <dl class="parent-level-dl">
          <dt>Available balance</dt>
          <dd><a href="/home/cat-home" class="parent-level-shop-link">{paw_points_balance}</a></dd>
          <dt>From care tasks</dt>
          <dd>+{paw_from_tasks}</dd>
          <dt>Purchased</dt>
          <dd>+{paw_purchased}</dd>
          <dt>Spent in shop</dt>
          <dd>-{paw_spent}</dd>
        </dl>
        <p class="parent-level-shop-hint"><a href="/home/cat-home" class="parent-level-shop-link">Visit {pet_name}'s home</a> to browse outfits and spend paw points.</p>
      </section>
    </div>
  </div>
</div>"#,
        level = profile.parent_level,
        parent_xp = profile.parent_xp,
        xp_per_level = XP_PER_LEVEL,
        progress_pct = progress_pct,
        xp_remaining = xp_remaining,
        next_level = profile.parent_level + 1,
        lifetime = lifetime,
        xp_from_tasks = xp_from_tasks,
        paw_points_balance = paw_points_amount_html(profile.paw_points),
        paw_from_tasks = paw_from_tasks,
        paw_purchased = paw_purchased,
        paw_spent = paw_spent,
        pet_name = escape_html(&profile.pet_name),
    )
}

fn outfit_by_id(id: &str) -> Option<&'static OutfitCatalogItem> {
    OUTFIT_CATALOG.iter().find(|item| item.id == id)
}

fn decor_by_id(id: &str) -> Option<&'static DecorCatalogItem> {
    DECOR_CATALOG.iter().find(|item| item.id == id)
}

fn tap_boost_by_id(id: &str) -> Option<&'static TapBoostCatalogItem> {
    TAP_BOOST_CATALOG.iter().find(|item| item.id == id)
}

fn ensure_tap_boost_state(profile: &mut UserProfile) -> bool {
    let mut changed = false;

    if profile.owned_tap_boosts.is_empty() {
        profile.owned_tap_boosts = default_owned_tap_boosts();
        changed = true;
    } else if !profile.owned_tap_boosts.iter().any(|id| id == "basic_pets") {
        profile.owned_tap_boosts.insert(0, "basic_pets".to_string());
        changed = true;
    }

    let equipped_id = profile.equipped_tap_boost.trim();
    let equipped_owned = !equipped_id.is_empty()
        && tap_boost_by_id(equipped_id).is_some()
        && profile.owned_tap_boosts.iter().any(|id| id == equipped_id);

    if !equipped_owned {
        let best_id = profile
            .owned_tap_boosts
            .iter()
            .filter_map(|id| tap_boost_by_id(id).map(|boost| (id.clone(), boost.tap_reward)))
            .max_by_key(|(_, reward)| *reward)
            .map(|(id, _)| id)
            .unwrap_or_else(default_equipped_tap_boost);
        profile.equipped_tap_boost = best_id;
        changed = true;
    }

    changed
}

fn equipped_tap_boost_reward(profile: &UserProfile) -> u32 {
    tap_boost_by_id(profile.equipped_tap_boost.trim())
        .map(|boost| boost.tap_reward)
        .unwrap_or(BASE_PET_PET_REWARD)
}

fn petting_bonus_by_id(id: &str) -> Option<&'static PettingBonusCatalogItem> {
    PETTING_BONUS_CATALOG.iter().find(|item| item.id == id)
}

fn clear_expired_petting_bonus(profile: &mut UserProfile) {
    if profile.petting_bonus_expires_at == 0 {
        return;
    }
    if profile.petting_bonus_expires_at <= timestamp_now() {
        profile.active_petting_bonus.clear();
        profile.petting_bonus_expires_at = 0;
    }
}

fn active_petting_bonus_multiplier(profile: &UserProfile) -> Option<u32> {
    if profile.petting_bonus_expires_at <= timestamp_now() {
        return None;
    }
    let bonus_id = profile.active_petting_bonus.trim();
    if bonus_id.is_empty() {
        return None;
    }
    petting_bonus_by_id(bonus_id).map(|bonus| bonus.multiplier)
}

fn petting_bonus_inventory_count(profile: &UserProfile, bonus_id: &str) -> u32 {
    profile
        .petting_bonus_inventory
        .get(bonus_id)
        .copied()
        .unwrap_or(0)
}

fn add_petting_bonus_to_inventory(profile: &mut UserProfile, bonus_id: &str) {
    let entry = profile
        .petting_bonus_inventory
        .entry(bonus_id.to_string())
        .or_insert(0);
    *entry = entry.saturating_add(1);
}

fn consume_petting_bonus_from_inventory(profile: &mut UserProfile, bonus_id: &str) -> bool {
    let Some(count) = profile.petting_bonus_inventory.get_mut(bonus_id) else {
        return false;
    };
    if *count == 0 {
        return false;
    }
    *count -= 1;
    if *count == 0 {
        profile.petting_bonus_inventory.remove(bonus_id);
    }
    true
}

fn effective_tap_reward(profile: &UserProfile) -> (u32, u32, Option<u32>) {
    let tap_boost_base = equipped_tap_boost_reward(profile);
    let petting_multiplier = active_petting_bonus_multiplier(profile);
    let total = petting_multiplier
        .map(|multiplier| tap_boost_base.saturating_mul(multiplier))
        .unwrap_or(tap_boost_base);
    (total, tap_boost_base, petting_multiplier)
}

fn render_petting_bonus_duration_label(duration_secs: u32) -> String {
    if duration_secs == 1 {
        "1 second".to_string()
    } else {
        format!("{duration_secs} seconds")
    }
}

fn render_petting_bonus_stacked_label(profile: &UserProfile, multiplier: u32) -> String {
    let tap_boost_base = equipped_tap_boost_reward(profile);
    let total = tap_boost_base.saturating_mul(multiplier);
    format!(
        "{multiplier}× your +{tap_boost_base} boost · +{total} per tap",
        tap_boost_base = tap_boost_base,
        total = total,
    )
}

fn render_tap_reward_label(reward: u32) -> String {
    format!("+{reward} per pet")
}

fn decor_slot_label(slot: &str) -> &'static str {
    match slot {
        "room" => "Room",
        "rug" => "Rug",
        "bed" => "Bed",
        "toy" => "Toy",
        "plant" => "Plant",
        _ => "Decor",
    }
}

fn equipped_decor_for_slot<'a>(
    profile: &'a UserProfile,
    slot: &str,
) -> Option<&'static DecorCatalogItem> {
    if let Some(id) = profile.equipped_decor.get(slot) {
        if let Some(decor) = decor_by_id(id) {
            return Some(decor);
        }
    }
    profile
        .owned_decor
        .iter()
        .find_map(|id| decor_by_id(id).filter(|decor| decor.slot == slot))
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn validate_pet_birth_date(value: &str, today: NaiveDate) -> Result<NaiveDate, ()> {
    let dob = parse_vet_date(value.trim()).ok_or(())?;
    if dob > today {
        return Err(());
    }
    let days = (today - dob).num_days();
    if days < 28 || days > 25 * 365 {
        return Err(());
    }
    Ok(dob)
}

fn derive_age_from_birth(
    dob: NaiveDate,
    today: NaiveDate,
) -> Result<(Option<u32>, Option<u32>), ()> {
    if dob > today {
        return Err(());
    }
    let days = (today - dob).num_days();
    if days < 28 {
        return Err(());
    }
    if days < 365 {
        let weeks = (days / 7).max(1) as u32;
        return Ok((Some(weeks), None));
    }

    let mut years = today.year() - dob.year();
    if (today.month(), today.day()) < (dob.month(), dob.day()) {
        years -= 1;
    }
    if years < 1 {
        let weeks = (days / 7).max(1) as u32;
        Ok((Some(weeks), None))
    } else {
        Ok((None, Some(years as u32)))
    }
}

fn format_birth_date_display(dob: NaiveDate) -> String {
    let month = MONTH_NAMES.get(dob.month0() as usize).unwrap_or(&"???");
    format!("{month} {}, {}", dob.day(), dob.year())
}

fn age_display_for_snapshot(snapshot: &PetSnapshot) -> String {
    if let Some(dob) = snapshot.pet_birth_date.as_deref().and_then(parse_vet_date) {
        if let Ok((Some(weeks), None)) = derive_age_from_birth(dob, Local::now().date_naive()) {
            return format!(
                "{weeks} weeks old (born {})",
                format_birth_date_display(dob)
            );
        }
        if let Ok((None, Some(years))) = derive_age_from_birth(dob, Local::now().date_naive()) {
            return format!(
                "{years} years old (born {})",
                format_birth_date_display(dob)
            );
        }
        return format!("Born {}", format_birth_date_display(dob));
    }
    if let Some(weeks) = snapshot.pet_age_weeks {
        return format!("{weeks} weeks old");
    }
    if let Some(years) = snapshot.pet_age_years {
        return format!("{years} years old");
    }
    "Age not set".to_string()
}

fn age_display(profile: &UserProfile) -> String {
    if let Some(snapshot) = active_pet_snapshot(profile) {
        return age_display_for_snapshot(&snapshot);
    }
    if let Some(dob) = profile.pet_birth_date.as_deref().and_then(parse_vet_date) {
        if let Ok((Some(weeks), None)) = derive_age_from_birth(dob, Local::now().date_naive()) {
            return format!(
                "{weeks} weeks old (born {})",
                format_birth_date_display(dob)
            );
        }
        if let Ok((None, Some(years))) = derive_age_from_birth(dob, Local::now().date_naive()) {
            return format!(
                "{years} years old (born {})",
                format_birth_date_display(dob)
            );
        }
        return format!("Born {}", format_birth_date_display(dob));
    }
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
    let snapshot = active_pet_snapshot(profile);
    let breed = pet_trait_display(
        snapshot
            .as_ref()
            .map(|pet| pet.pet_breed.as_str())
            .unwrap_or(profile.pet_breed.as_str()),
    );
    let color = snapshot
        .as_ref()
        .map(|pet| pet.pet_color.as_str())
        .unwrap_or(profile.pet_color.as_str())
        .trim();
    let color_part = if color.is_empty() {
        String::new()
    } else {
        format!(" · {}", escape_html(color))
    };
    let mood = snapshot
        .as_ref()
        .map(|pet| pet.pet_mood.as_str())
        .unwrap_or(profile.pet_mood.as_str());
    format!(
        "{breed}{color_part} · Mood: {}",
        escape_html(mood)
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
    let month = MONTH_NAMES.get(date.month0() as usize).unwrap_or(&"???");
    format!("{month} {} · 10:00 AM", date.day())
}

fn format_time_from_minutes(minutes: u16) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    let (hour_12, period) = if hours == 0 {
        (12, "AM")
    } else if hours < 12 {
        (hours, "AM")
    } else if hours == 12 {
        (12, "PM")
    } else {
        (hours - 12, "PM")
    };
    format!("{hour_12}:{mins:02} {period}")
}

fn calendar_event_from_date(date: NaiveDate, title: &str) -> CalendarEvent {
    let time_minutes = default_event_time_minutes();
    CalendarEvent {
        id: None,
        day: date.day(),
        month: date.month(),
        year: date.year() as u32,
        title: title.to_string(),
        time_label: format_event_time_label(date),
        time_minutes,
    }
}

pub(crate) fn visible_calendar_events(
    profile: &UserProfile,
    reference_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let today = Local::now().date_naive();
    let horizon = today + Duration::days(730);
    let mut events = merge_calendar_events(profile, reference_date);
    events.extend(generate_daily_care_calendar_events(profile, today, horizon));
    events.extend(generate_breed_guide_calendar_events(profile, today, horizon));
    events.extend(profile.user_calendar_events.iter().cloned());
    events.sort_by(|left, right| {
        (
            left.year,
            left.month,
            left.day,
            left.time_minutes,
            &left.title,
        )
            .cmp(&(
                right.year,
                right.month,
                right.day,
                right.time_minutes,
                &right.title,
            ))
    });
    events
}

fn resolve_calendar_view(cal_month: Option<&str>, cal_year: Option<&str>) -> (u32, u32) {
    let today = Local::now().date_naive();
    let default_month = today.month();
    let default_year = today.year() as u32;

    let year = cal_year
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default_year);
    let month = cal_month
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default_month);

    if (1..=12).contains(&month) && (1970..=2100).contains(&year) {
        (month, year)
    } else {
        (default_month, default_year)
    }
}

fn parse_calendar_date_fields(day: &str, month: &str, year: &str) -> Option<(u32, u32, u32)> {
    let day: u32 = day.trim().parse().ok()?;
    let month: u32 = month.trim().parse().ok()?;
    let year: u32 = year.trim().parse().ok()?;
    if !(1..=12).contains(&month) || year < 2000 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year as i32, month, day)?;
    Some((date.day(), date.month(), date.year() as u32))
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
    let key = name.trim().to_lowercase().replace([' ', '-', '_'], "");

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
    if let Some(stored) = profile.pet_birth_date.as_deref().and_then(parse_vet_date) {
        return Some(stored);
    }
    if let Some(weeks) = profile.pet_age_weeks {
        return reference.checked_sub_signed(Duration::weeks(weeks as i64));
    }
    if let Some(years) = profile.pet_age_years {
        return reference.checked_sub_signed(Duration::days(i64::from(years) * 365));
    }
    None
}

fn birthday_in_year(birth: NaiveDate, year: i32) -> Option<NaiveDate> {
    let month = birth.month();
    let day = birth.day();
    if month == 2 && day == 29 {
        return NaiveDate::from_ymd_opt(year, 2, 29)
            .or_else(|| NaiveDate::from_ymd_opt(year, 2, 28));
    }
    NaiveDate::from_ymd_opt(year, month, day)
}

fn generate_birthday_calendar_events(
    profile: &UserProfile,
    today: NaiveDate,
    horizon: NaiveDate,
) -> Vec<CalendarEvent> {
    let Some(birth) = profile.pet_birth_date.as_deref().and_then(parse_vet_date) else {
        return vec![];
    };

    let pet_name = if profile.pet_name.is_empty() {
        "Your cat".to_string()
    } else {
        profile.pet_name.clone()
    };

    let mut events = Vec::new();
    for year in today.year()..=horizon.year() {
        let Some(birthday) = birthday_in_year(birth, year) else {
            continue;
        };
        if birthday < today || birthday > horizon {
            continue;
        }
        events.push(calendar_event_from_date(
            birthday,
            &format!("{pet_name}'s birthday"),
        ));
    }

    events
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
    history: &[VaccineRecord],
    birth: NaiveDate,
    today: NaiveDate,
    horizon: NaiveDate,
    pet_name: &str,
) {

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
        push_vaccine_reminder(
            events,
            rabies_at,
            "Rabies vaccine",
            pet_name,
            today,
            horizon,
        );
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
    history: &[VaccineRecord],
    outdoor: bool,
    birth: NaiveDate,
    today: NaiveDate,
    horizon: NaiveDate,
    pet_name: &str,
) {
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

    let felv_interval = if outdoor {
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
            } else if outdoor {
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
    pet_snapshot(profile, PRIMARY_PET_ID)
        .map(|snapshot| generate_vaccine_calendar_events_for_snapshot(&snapshot, reference_date))
        .unwrap_or_default()
}

pub(crate) fn generate_birthday_calendar_events_for_snapshot(
    snapshot: &PetSnapshot,
    today: NaiveDate,
    horizon: NaiveDate,
) -> Vec<CalendarEvent> {
    let Some(birth) = pet_birth_date_for_snapshot(snapshot, today) else {
        return vec![];
    };

    let pet_name = if snapshot.pet_name.is_empty() {
        "Your cat".to_string()
    } else {
        snapshot.pet_name.clone()
    };

    let mut events = Vec::new();
    for year in today.year()..=horizon.year() {
        let Some(birthday) = birthday_in_year(birth, year) else {
            continue;
        };
        if birthday < today || birthday > horizon {
            continue;
        }
        events.push(calendar_event_from_date(
            birthday,
            &format!("{pet_name}'s birthday"),
        ));
    }

    events
}

fn vet_reminder_interval_for_snapshot(snapshot: &PetSnapshot) -> Duration {
    if snapshot.pet_age_weeks.is_some_and(|weeks| weeks < 16) {
        return Duration::weeks(4);
    }

    if snapshot.pet_age_years.is_some_and(|years| years >= 10) {
        return Duration::days(182);
    }

    Duration::days(365)
}

pub(crate) fn generate_vet_calendar_events_for_snapshot(
    snapshot: &PetSnapshot,
    signup_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let today = Local::now().date_naive();
    let anchor = snapshot
        .last_vet_date
        .as_deref()
        .and_then(parse_vet_date)
        .unwrap_or(signup_date);

    let pet_name = if snapshot.pet_name.is_empty() {
        "Your cat".to_string()
    } else {
        snapshot.pet_name.clone()
    };

    let mut events = Vec::new();

    if snapshot.last_vet_date.is_some() {
        events.push(calendar_event_from_date(
            anchor,
            &format!("Last vet visit — {pet_name}"),
        ));
    }

    let horizon = today + Duration::days(730);

    if snapshot.never_been_to_vet {
        let asap_title = format!("Make vet appointment ASAP — {pet_name}");
        let asap_interval = Duration::weeks(2);
        let mut next = today;
        while next <= horizon {
            events.push(calendar_event_from_date(next, &asap_title));
            next += asap_interval;
        }
    } else {
        let interval = vet_reminder_interval_for_snapshot(snapshot);
        let reminder_title = format!("Vet checkup reminder — {pet_name}");
        let mut next = if snapshot.last_vet_date.is_none() {
            today
        } else {
            anchor + interval
        };

        while next <= horizon {
            if snapshot.last_vet_date.is_none() || next > anchor {
                events.push(calendar_event_from_date(next, &reminder_title));
            }
            next += interval;
        }
    }

    events.sort_by_key(|event| (event.year, event.month, event.day));
    events
}

pub(crate) fn generate_vaccine_calendar_events_for_snapshot(
    snapshot: &PetSnapshot,
    reference_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let Some(birth) = pet_birth_date_for_snapshot(snapshot, reference_date) else {
        return Vec::new();
    };

    let pet_name = if snapshot.pet_name.is_empty() {
        "Your cat".to_string()
    } else {
        snapshot.pet_name.clone()
    };

    let today = reference_date;
    let horizon = reference_date + Duration::days(730);
    let mut events = Vec::new();
    let history = &snapshot.vaccine_history;
    let outdoor = is_outdoor_snapshot(snapshot);

    if let Some(weeks) = snapshot.pet_age_weeks {
        if weeks <= 20 {
            schedule_kitten_vaccines(&mut events, history, birth, today, horizon, &pet_name);
        }
        if weeks > 20 {
            schedule_adult_vaccines(&mut events, history, outdoor, birth, today, horizon, &pet_name);
        }
    } else if snapshot
        .pet_age_years
        .is_some_and(|years| (1..=10).contains(&years))
    {
        schedule_adult_vaccines(&mut events, history, outdoor, birth, today, horizon, &pet_name);
    }

    events.sort_by_key(|event| (event.year, event.month, event.day));
    events
}

pub(crate) fn merge_calendar_events(
    profile: &UserProfile,
    signup_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let today = Local::now().date_naive();
    let horizon = signup_date + Duration::days(730);
    let mut events = Vec::new();
    let premium_health =
        entitlements::can_access_health_records(profile.premium_unlocked, &profile.email);

    for (pet_id, _) in list_pet_summaries(profile) {
        let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
            continue;
        };
        if premium_health {
            events.extend(generate_vet_calendar_events_for_snapshot(
                &snapshot,
                signup_date,
            ));
            events.extend(generate_vaccine_calendar_events_for_snapshot(
                &snapshot,
                signup_date,
            ));
        }
        events.extend(generate_birthday_calendar_events_for_snapshot(
            &snapshot,
            today,
            horizon,
        ));
    }

    events.sort_by_key(|event| (event.year, event.month, event.day));
    events
}

fn generate_vet_calendar_events(
    profile: &UserProfile,
    signup_date: NaiveDate,
) -> Vec<CalendarEvent> {
    pet_snapshot(profile, PRIMARY_PET_ID)
        .map(|snapshot| generate_vet_calendar_events_for_snapshot(&snapshot, signup_date))
        .unwrap_or_default()
}

fn render_pet_health_info(profile: &UserProfile) -> String {
    let Some(snapshot) = active_pet_snapshot(profile) else {
        return String::new();
    };

    let lifestyle = escape_html(&indoor_outdoor_display(
        snapshot.pet_indoor_outdoor.as_deref(),
    ));

    if !entitlements::can_access_health_records(profile.premium_unlocked, &profile.email) {
        return format!(
            r#"<dl class="pet-health-dl"><dt>Breed</dt><dd>{breed}</dd><dt>Color</dt><dd>{color}</dd><dt>Age</dt><dd>{age}</dd><dt>Lifestyle</dt><dd>{lifestyle}</dd></dl><p class="field-hint pet-health-tab-hint">Upgrade to <a href="/home?tab=account" class="pet-health-tab-link">WhiskerWatch Plus</a> to unlock health history, vet records, and vaccine tracking.</p>"#,
            breed = pet_trait_display(&snapshot.pet_breed),
            color = pet_trait_display(&snapshot.pet_color),
            age = escape_html(&age_display_for_snapshot(&snapshot)),
            lifestyle = lifestyle,
        );
    }

    let last_vet = snapshot
        .last_vet_date
        .as_deref()
        .map(|date| escape_html(date))
        .unwrap_or_else(|| "Never".to_string());

    let conditions = if snapshot.pet_conditions.trim().is_empty() {
        "None noted".to_string()
    } else {
        escape_html(&snapshot.pet_conditions)
    };

    let medications = if snapshot.pet_medications.trim().is_empty() {
        "None noted".to_string()
    } else {
        escape_html(&snapshot.pet_medications)
    };

    let vaccine_list = if snapshot.pet_vaccines_unknown {
        "Unknown — we recommend a vet visit soon to get vaccines up to date".to_string()
    } else if snapshot.vaccine_history.is_empty() {
        "None recorded".to_string()
    } else {
        let items: String = snapshot
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
        r#"<dl class="pet-health-dl"><dt>Breed</dt><dd>{breed}</dd><dt>Color</dt><dd>{color}</dd><dt>Age</dt><dd>{age}</dd><dt>Lifestyle</dt><dd>{lifestyle}</dd><dt>Last vet appointment</dt><dd>{last_vet}</dd><dt>Conditions</dt><dd>{conditions}</dd><dt>Medications</dt><dd>{medications}</dd><dt>Vaccine history</dt><dd>{vaccine_list}</dd></dl><p class="field-hint pet-health-tab-hint">See the <a href="/home?tab=health" class="pet-health-tab-link">Health</a> tab for full veterinary notes and records.</p>"#,
        breed = pet_trait_display(&snapshot.pet_breed),
        color = pet_trait_display(&snapshot.pet_color),
        age = escape_html(&age_display_for_snapshot(&snapshot)),
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

fn default_vet_visit_date(profile: &UserProfile) -> String {
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    profile
        .last_vet_date
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&today)
        .to_string()
}

fn render_vet_visit_vaccine_rows(profile: &UserProfile) -> String {
    if profile.vaccine_history.is_empty() {
        render_vaccine_row_html("", "")
    } else {
        profile
            .vaccine_history
            .iter()
            .map(|record| render_vaccine_row_html(&record.vaccine_name, &record.date))
            .collect()
    }
}

fn render_vet_visit_form_fields(
    profile: &UserProfile,
    last_vet_input_id: &str,
    vaccine_rows_id: &str,
    add_vaccine_btn_id: &str,
    vet_note_id: &str,
    submit_label: &str,
) -> String {
    format!(
        r#"<label for="{last_vet_input_id}">Last vet appointment</label>
      <input id="{last_vet_input_id}" name="last_vet_date" type="date" value="{last_vet}" />

      <fieldset class="vaccine-history-fieldset">
        <legend>Vaccines given</legend>
        <p class="field-hint">Add or edit vaccines from this visit.</p>
        <div id="{vaccine_rows_id}" class="vaccine-rows">
          {vaccine_rows}
        </div>
        <button type="button" class="download-btn vaccine-add-btn" id="{add_vaccine_btn_id}">+ Add vaccine</button>
      </fieldset>

      <label for="{vet_note_id}">Veterinary notes</label>
      <textarea id="{vet_note_id}" name="vet_note" rows="4" placeholder="Exam findings, recommendations, follow-up instructions…"></textarea>

      <button type="submit" class="download-btn login-submit">{submit_label}</button>"#,
        last_vet_input_id = last_vet_input_id,
        last_vet = escape_html_attr(&default_vet_visit_date(profile)),
        vaccine_rows_id = vaccine_rows_id,
        vaccine_rows = render_vet_visit_vaccine_rows(profile),
        add_vaccine_btn_id = add_vaccine_btn_id,
        vet_note_id = vet_note_id,
        submit_label = submit_label,
    )
}

fn render_vet_followup_modal(profile: &UserProfile, show: bool) -> String {
    if !profile_has_pet(profile)
        || !entitlements::can_access_health_records(profile.premium_unlocked, &profile.email)
    {
        return String::new();
    }

    let hidden = if show { "" } else { " hidden" };
    let form_fields = render_vet_visit_form_fields(
        profile,
        "vet_last_vet_date",
        "vet-vaccine-rows",
        "vet-add-vaccine-row",
        "vet_note",
        "Save vet visit",
    );

    format!(
        r#"<div class="onboarding-backdrop" id="vet-followup-modal" role="dialog" aria-modal="true" aria-labelledby="vet-followup-title"{hidden}>
  <div class="onboarding-modal">
    <h2 id="vet-followup-title">Record vet visit 🏥</h2>
    <p class="onboarding-intro">Update vaccines and add notes from your appointment so your Health tab stays current.</p>
    <form class="onboarding-form login-form" action="/home/vet-visit" method="post">
      {form_fields}
    </form>
  </div>
</div>"#,
        form_fields = form_fields,
    )
}

fn render_health_tab(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return r#"<p class="panel-intro">Health records need a pet profile before you can track vaccines, vet visits, and notes.</p>
<div class="health-tab-setup-alert" role="alert">
  <p>Add your cat to unlock vaccine tracking, vet visit logs, and health notes.</p>
  <p class="health-tab-setup-cta"><button type="button" class="download-btn pet-setup-trigger" id="health-tab-setup-trigger">Create your pet</button></p>
</div>"#
            .to_string();
    }

    let stripe_enabled = stripe_payments::stripe_checkout_enabled();
    let premium = entitlements::can_access_health_records(profile.premium_unlocked, &profile.email);
    let breed_guide_card = breed_guides::render_health_tab_card(
        &profile.pet_name,
        &profile.pet_breed,
        &profile.owned_breed_guides,
        profile.premium_unlocked,
        &profile.email,
        stripe_enabled,
    );
    let breed_guides_shop_link = r#"<p class="health-breed-guides-link"><a href="/home/breed-guides" class="health-breed-guides-btn">Browse premium breed guides 📚</a></p>"#;
    let symptom_checker_card =
        symptom_checker::render_health_tab_card(&display_pet_name(profile));
    let financial_resources_card = vet_financial_resources::render_health_tab_card();

    if !premium {
        let upsell = entitlements::render_health_records_upsell_compact(stripe_enabled);
        return format!(
            r#"<p class="panel-intro">Premium breed care guides for {pet_name}, plus optional WhiskerWatch Plus for vet records.</p>
<div class="health-grid">
  {symptom_checker_card}
  {financial_resources_card}
  {breed_guide_card}
  {breed_guides_shop_link}
  {upsell}
</div>"#,
            pet_name = escape_html(&profile.pet_name),
            symptom_checker_card = symptom_checker_card,
            financial_resources_card = financial_resources_card,
            breed_guide_card = breed_guide_card,
            breed_guides_shop_link = breed_guides_shop_link,
            upsell = upsell,
        );
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

    let (vet_notes_display, vet_notes_label, vet_notes_placeholder, submit_label) = if let Some(
        notes,
    ) =
        vet_notes_value
    {
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
    let vet_visit_form = render_vet_visit_form_fields(
        profile,
        "health-vet-last-date",
        "health-vaccine-rows",
        "health-add-vaccine-row",
        "health-vet-note",
        "Save vet visit",
    );

    format!(
        r#"<p class="panel-intro">Health records for {pet_name} — vaccines, vet visits, and notes.</p>
<div class="health-grid">
  {symptom_checker_card}
  {financial_resources_card}
  {breed_guide_card}
  {breed_guides_shop_link}
  <article class="dashboard-card health-vet-visit-card">
    <details class="health-vet-disclosure" id="health-vet-disclosure">
      <summary class="health-vet-disclosure-summary">
        <span class="health-vet-disclosure-title">Add veterinary information</span>
        <span class="health-vet-disclosure-hint">Record a vet appointment for {pet_name}</span>
      </summary>
      <div class="health-vet-disclosure-body">
        <p class="field-hint">Add the visit date, vaccines given, and notes from your appointment.</p>
        <form class="onboarding-form login-form health-vet-visit-form" action="/home/vet-visit" method="post">
          {vet_visit_form}
        </form>
      </div>
    </details>
  </article>
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
        vet_visit_form = vet_visit_form,
        breed_guide_card = breed_guide_card,
        breed_guides_shop_link = breed_guides_shop_link,
        symptom_checker_card = symptom_checker_card,
        financial_resources_card = financial_resources_card,
    )
}

pub(crate) fn profile_has_pet(profile: &UserProfile) -> bool {
    let name = profile.pet_name.trim();
    let breed = profile.pet_breed.trim();
    let has_name = !name.is_empty()
        && !name.eq_ignore_ascii_case("your cat")
        && !name.eq_ignore_ascii_case("no pet yet");
    let has_breed = !breed.is_empty() && !breed.eq_ignore_ascii_case("add your cat's details");
    let has_age = profile
        .pet_birth_date
        .as_deref()
        .is_some_and(|value| parse_vet_date(value).is_some())
        || profile.pet_age_weeks.is_some()
        || profile.pet_age_years.is_some();
    let has_lifestyle = profile
        .pet_indoor_outdoor
        .as_deref()
        .is_some_and(|value| value == "indoor" || value == "outdoor");

    has_name && has_breed && has_age && has_lifestyle
}

fn user_needs_pet_setup(profile: &UserProfile) -> bool {
    !profile_has_pet(profile)
}

fn display_pet_name(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        "No pet yet".to_string()
    } else if let Some(pet) = active_pet_snapshot(profile) {
        pet.pet_name.clone()
    } else {
        profile.pet_name.clone()
    }
}

fn render_pet_blurb(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return "Create a pet".to_string();
    }

    if memorial::active_pet_is_deceased(profile) {
        let name = escape_html(&display_pet_name(profile));
        return format!(
            "{name} is your angel cat now. Visit their home among the stars and keep their memory close on the Account tab.",
        );
    }

    let name = active_pet_snapshot(profile)
        .map(|pet| pet.pet_name.clone())
        .unwrap_or_else(|| profile.pet_name.clone());

    format!(
        "{} mirrors your real cat's care routine. Complete tasks to keep them happy and earn paw points!",
        escape_html(&name)
    )
}

fn account_profile_photo_src(profile: &UserProfile) -> String {
    profile
        .pet_photo_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "/cinderanimate.png".to_string())
}

fn normalize_pet_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 40 {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("your cat") || trimmed.eq_ignore_ascii_case("no pet yet") {
        return None;
    }
    Some(trimmed.to_string())
}

fn render_account_pet_name_field(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return r#"<dt>Pet name</dt>
<dd class="account-pet-name-empty">Set up your cat on the My Pet tab.</dd>"#
            .to_string();
    }

    let pet_name_display = escape_html(&profile.pet_name);
    format!(
        r#"<dt>Pet name</dt>
<dd class="account-pet-name-dd">
  <div class="account-pet-name-display">
    <span class="account-pet-name-inline">
      <span class="account-pet-name-value">{pet_name_display}</span>
      <button type="button" class="account-pet-name-change-trigger" aria-label="Change pet name">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M12 20h9" />
          <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
        </svg>
      </button>
    </span>
  </div>
  <form class="account-pet-name-form" action="/home/pet-name" method="post" hidden>
    <input id="account-pet-name-input" name="pet_name" type="text" value="{pet_name}" maxlength="40" aria-label="Pet name" required />
    <button type="submit" class="download-btn account-pet-name-save">Save name</button>
    <button type="button" class="onboarding-skip-btn account-pet-name-cancel">Cancel</button>
  </form>
</dd>"#,
        pet_name = escape_html_attr(&profile.pet_name),
        pet_name_display = pet_name_display,
    )
}

fn render_account_pet_media_actions(has_video: bool) -> String {
    let gif_button = if has_video {
        r#"<button type="button" class="download-btn pet-video-upload-trigger account-pet-gif-change-trigger" data-return-tab="account">Change cat GIF</button>"#
    } else {
        r#"<button type="button" class="download-btn pet-video-upload-trigger account-pet-gif-change-trigger" data-return-tab="account">cat GIF creator</button>"#
    };

    format!(
        r#"<div class="account-pet-media-actions">
  <button type="button" class="download-btn account-pet-photo-change-trigger">Change profile photo</button>
  {gif_button}
</div>"#
    )
}

pub(crate) fn render_account_pet_photo_living(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return r#"<p class="account-pet-photo-empty">Add your cat on the My Pet tab to upload a profile photo and playing video clip.</p>"#
            .to_string();
    }

    let pet_name = escape_html(&profile.pet_name);
    let photo_src = escape_html_attr(&account_profile_photo_src(profile));
    let has_video = profile_has_pet_video(profile);
    let media_actions = render_account_pet_media_actions(has_video);

    if !has_video {
        return format!(
            r#"<div class="account-pet-photo" id="account-pet-photo-stage" data-pet-name="{pet_name}">
  <div class="account-pet-photo-wrap">
    <img class="account-pet-photo-image" src="{photo_src}" alt="{pet_name} profile photo" />
  </div>
  <p class="account-pet-photo-caption">{pet_name} profile photo</p>
  {media_actions}
</div>"#,
            pet_name = pet_name,
            photo_src = photo_src,
            media_actions = media_actions,
        );
    }

    let url = profile
        .pet_video_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .expect("profile_has_pet_video implies url");
    let clip_start = pet_video_clip_start(profile);
    let clip_duration = pet_video_clip_duration(profile);
    let clip_label = format_pet_video_clip_duration_label(clip_duration);
    let framing_attrs = pet_video_framing_data_attrs(
        profile.pet_video_zoom,
        profile.pet_video_offset_x,
        profile.pet_video_offset_y,
    );
    format!(
        r#"<div class="account-pet-photo" id="account-pet-photo-stage" data-pet-name="{pet_name}" data-clip-label="{clip_label}">
  <div
    class="account-pet-photo-wrap account-pet-photo-toggle"
    role="button"
    tabindex="0"
    aria-pressed="false"
    aria-label="Show {pet_name} playing video"
  >
    <img class="account-pet-photo-image" src="{photo_src}" alt="{pet_name} profile photo" />
    <div class="account-pet-video-optional" hidden>
      <video
        class="account-pet-video-player"
        src="{url}"
        muted
        playsinline
        webkit-playsinline
        preload="auto"
        data-clip-start="{clip_start}"
        data-clip-duration="{clip_duration}"{framing_attrs}
        aria-label="Playing video clip of {pet_name}"
      ></video>
    </div>
  </div>
  <p class="account-pet-photo-caption">{pet_name} · tap photo for playing clip</p>
  {media_actions}
</div>"#,
        url = escape_html_attr(url),
        pet_name = pet_name,
        photo_src = photo_src,
        clip_start = clip_start,
        clip_duration = clip_duration,
        clip_label = clip_label,
        media_actions = media_actions,
    )
}

const PASSWORD_TOGGLE_BUTTON_HTML: &str = r#"<button
      type="button"
      class="password-toggle password-toggle--hidden"
      data-password-toggle
      aria-label="Show password"
      aria-pressed="false"
    >
      <span class="password-toggle-icon password-toggle-icon--show" aria-hidden="true">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      </span>
      <span class="password-toggle-icon password-toggle-icon--hide" aria-hidden="true" hidden>
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19M1 1l22 22" />
        </svg>
      </span>
    </button>"#;

fn render_password_input_field(id: &str, name: &str, label: &str, autocomplete: &str) -> String {
    format!(
        r#"<label for="{id}">{label}</label>
<div class="password-field">
  <input id="{id}" name="{name}" type="password" required autocomplete="{autocomplete}" />
  {toggle}
</div>"#,
        id = id,
        name = name,
        label = label,
        autocomplete = autocomplete,
        toggle = PASSWORD_TOGGLE_BUTTON_HTML,
    )
}

fn render_account_password_section(email: &str) -> String {
    if is_admin_account(email) {
        return r#"<article class="dashboard-card account-password-card">
  <h2>Password</h2>
  <p class="account-password-note">Admin passwords are managed with the <code>ADMIN_PASSWORD</code> environment variable on the server.</p>
</article>"#
            .to_string();
    }

    format!(
        r#"<article class="dashboard-card account-password-card">
  <h2>Password</h2>
  <p class="account-password-note">Use at least 5 characters with one number and one special character.</p>
  <form id="account-change-password-form" class="login-form account-password-form" action="/home/password" method="post">
    {current_password}
    {new_password}
    {confirm_password}
    <ul class="password-requirements" id="account-password-requirements" aria-live="polite" aria-label="Password requirements">
      <li id="account-pw-req-length" data-requirement="length">
        <span class="password-req-icon" aria-hidden="true"></span>
        At least 5 characters
      </li>
      <li id="account-pw-req-digit" data-requirement="digit">
        <span class="password-req-icon" aria-hidden="true"></span>
        At least one number
      </li>
      <li id="account-pw-req-special" data-requirement="special">
        <span class="password-req-icon" aria-hidden="true"></span>
        At least one special character
      </li>
    </ul>
    <p class="password-confirm-error" id="account-password-confirm-error" role="alert" hidden>Passwords do not match.</p>
    <button type="submit" class="download-btn login-submit">Change password</button>
  </form>
</article>"#,
        current_password = render_password_input_field(
            "account-current-password",
            "current_password",
            "Current password",
            "current-password",
        ),
        new_password = render_password_input_field(
            "account-new-password",
            "new_password",
            "New password",
            "new-password",
        ),
        confirm_password = render_password_input_field(
            "account-confirm-password",
            "confirm_password",
            "Confirm new password",
            "new-password",
        ),
    )
}

fn render_account_pet_photo_modal(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) || memorial::active_pet_is_deceased(profile) {
        return String::new();
    }

    r#"<div class="onboarding-backdrop" id="account-pet-photo-modal" role="dialog" aria-modal="true" aria-labelledby="account-pet-photo-title" hidden>
  <div class="onboarding-modal">
    <h2 id="account-pet-photo-title">Change profile photo</h2>
    <p class="onboarding-intro">Upload a photo of your cat for your account profile.</p>
    <form class="onboarding-form login-form" action="/home/pet-photo" method="post" enctype="multipart/form-data">
      <fieldset class="pet-photo-fieldset">
        <legend>Profile photo</legend>
        <p class="field-hint">JPEG, PNG, or WebP up to 5MB.</p>
        <div class="pet-photo-upload">
          <input id="account_pet_photo" name="pet_photo" type="file" class="pet-photo-input" accept="image/jpeg,image/png,image/webp,.jpg,.jpeg,.png,.webp" required />
          <label for="account_pet_photo" class="pet-photo-paw-btn" aria-label="Choose profile photo">
            <span class="pet-photo-paw-icon" aria-hidden="true">📷</span>
          </label>
        </div>
        <div id="account-pet-photo-preview" class="pet-photo-preview" hidden aria-live="polite"></div>
      </fieldset>
      <input type="hidden" name="return_tab" value="account" />
      <div class="onboarding-actions">
        <button type="submit" class="download-btn login-submit">Save photo</button>
        <button type="button" class="onboarding-skip-btn" id="account-pet-photo-cancel">Cancel</button>
      </div>
    </form>
  </div>
</div>"#
        .to_string()
}

fn render_pet_check_cta(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return r#"<p class="pet-check-cta"><button type="button" class="download-btn pet-setup-trigger" id="pet-home-setup-trigger">Set up your cat's virtual home</button></p>"#.to_string();
    }

    r#"<p class="pet-check-cta"><a href="/home/cat-home" class="download-btn">Check on your cat</a></p>"#.to_string()
}

fn render_pet_video_upload_cta(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile)
        || profile_has_pet_video(profile)
        || memorial::active_pet_is_deceased(profile)
    {
        return String::new();
    }

    r#"<p class="pet-video-upload-cta"><button type="button" class="download-btn pet-video-upload-trigger">cat GIF creator</button></p>"#
        .to_string()
}

fn render_pet_video_modal(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return String::new();
    }

    r#"<div class="onboarding-backdrop" id="pet-video-upload-modal" role="dialog" aria-modal="true" aria-labelledby="pet-video-upload-title" hidden>
  <div class="onboarding-modal">
    <h2 id="pet-video-upload-title">cat GIF creator</h2>
    <p class="onboarding-intro">Upload a video of your cat playing, then pick a 3–6 second clip that loops on the My Pet tab.</p>
    <form class="onboarding-form login-form" action="/home/pet-video" method="post" enctype="multipart/form-data">
      <fieldset class="pet-video-fieldset">
        <legend>Cat playing video</legend>
        <p class="field-hint">MP4, WebM, or MOV up to 50MB. Pick a 3–6 second clip of your cat playing.</p>
        <div class="pet-photo-upload">
          <input id="upload_pet_video" name="pet_video" type="file" class="pet-photo-input" accept="video/mp4,video/webm,video/quicktime,.mp4,.webm,.mov" required />
          <label for="upload_pet_video" class="pet-photo-paw-btn" aria-label="Choose cat playing video">
            <span class="pet-photo-paw-icon" aria-hidden="true">🎬</span>
          </label>
        </div>
        <div id="upload-pet-video-preview" class="pet-video-preview" hidden aria-live="polite"></div>
        <input type="hidden" id="upload_pet_video_clip_start" name="pet_video_clip_start" value="0" />
        <input type="hidden" id="upload_pet_video_clip_duration" name="pet_video_clip_duration" value="6" />
        <input type="hidden" id="upload_pet_video_zoom" name="pet_video_zoom" value="" />
        <input type="hidden" id="upload_pet_video_offset_x" name="pet_video_offset_x" value="" />
        <input type="hidden" id="upload_pet_video_offset_y" name="pet_video_offset_y" value="" />
      </fieldset>
      <input type="hidden" id="pet_video_return_tab" name="return_tab" value="pet" />
      <div class="onboarding-actions">
        <button type="submit" class="download-btn login-submit">Save GIF</button>
        <button type="button" class="onboarding-skip-btn" id="pet-video-upload-cancel">Cancel</button>
      </div>
    </form>
  </div>
</div>"#
        .to_string()
}

fn render_cat_home_nav_link(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return String::new();
    }

    r#"<a href="/home/cat-home">CAT HOME</a>"#.to_string()
}

fn render_pet_setup_cta(profile: &UserProfile) -> String {
    if !user_needs_pet_setup(profile) {
        return String::new();
    }

    r#"<p class="pet-setup-cta"><button type="button" class="download-btn pet-setup-trigger" id="pet-setup-trigger">Create your pet</button></p>"#
        .to_string()
}

fn render_calendar_pet_setup_prompt(profile: &UserProfile) -> String {
    if !user_needs_pet_setup(profile) {
        return String::new();
    }

    r#"<div class="calendar-pet-setup-alert" role="alert">
  <p>Add your cat to unlock a personalized calendar with vet and vaccine reminders.</p>
  <p class="calendar-pet-setup-cta"><button type="button" class="download-btn pet-setup-trigger" id="calendar-pet-setup-trigger">Create your pet</button></p>
</div>"#
    .to_string()
}

fn render_tasks_tab_setup_prompt(profile: &UserProfile) -> String {
    if !user_needs_pet_setup(profile) {
        return String::new();
    }

    r#"<div class="tasks-tab-setup-alert" role="alert">
  <p>Add your cat to unlock personalized care tasks and start earning paw points.</p>
  <p class="tasks-tab-setup-cta"><button type="button" class="download-btn pet-setup-trigger" id="tasks-tab-setup-trigger">Create your pet</button></p>
</div>"#
    .to_string()
}

fn render_pet_color_picker(hidden_id: &str, select_id: &str, custom_id: &str) -> String {
    const OPTIONS: [(&str, &str, &str); 8] = [
        ("Black", "Black", "🖤"),
        ("White", "White", "🤍"),
        ("Gray", "Gray", "🩶"),
        ("Orange", "Orange", "🧡"),
        ("Tabby", "Tabby", "🐯"),
        ("Calico", "Calico", "🎨"),
        ("Tortoiseshell", "Tortoiseshell", "🐢"),
        ("Black and white", "Black and white", "🖤🤍"),
    ];

    let preset_options = OPTIONS
        .iter()
        .map(|(value, label, emoji)| {
            format!(
                r#"<option value="{value}">{emoji} {label}</option>"#,
                value = escape_html(value),
                emoji = emoji,
                label = escape_html(label),
            )
        })
        .collect::<String>();

    format!(
        r#"<div class="pet-color-picker" data-pet-color-picker>
  <label for="{select_id}">Cat color / markings</label>
  <div class="pet-color-select-wrap">
    <select id="{select_id}" class="pet-color-select" data-pet-color-select autocomplete="off">
    <option value="">Pick a color or pattern…</option>
    {preset_options}
    <option value="__other__">✨ Something else…</option>
  </select>
  </div>
  <div class="pet-color-custom" data-pet-color-custom hidden>
    <label class="pet-color-custom-label" for="{custom_id}">Describe their unique look</label>
    <input id="{custom_id}" type="text" class="pet-color-custom-input" data-pet-color-custom-input placeholder="e.g. tuxedo with white socks" maxlength="80" autocomplete="off" />
  </div>
  <input type="hidden" id="{hidden_id}" name="pet_color" value="" />
  <p class="field-hint">Choose a common coat pattern or tell us your kitty's special markings.</p>
</div>"#,
        select_id = select_id,
        custom_id = custom_id,
        hidden_id = hidden_id,
        preset_options = preset_options,
    )
}

fn render_birth_date_picker(hidden_id: &str, month_id: &str, day_id: &str, year_id: &str) -> String {
    let today = Local::now().date_naive();
    let max_year = today.year();
    let min_year = max_year - 30;
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];

    let month_options = std::iter::once(r#"<option value="">Month</option>"#.to_string())
        .chain(MONTHS.iter().enumerate().map(|(index, name)| {
            format!(
                r#"<option value="{:02}">{}</option>"#,
                index + 1,
                escape_html(name)
            )
        }))
        .collect::<String>();

    let day_options = std::iter::once(r#"<option value="">Day</option>"#.to_string())
        .chain((1..=31).map(|day| format!(r#"<option value="{day:02}">{day}</option>"#)))
        .collect::<String>();

    let year_options = std::iter::once(r#"<option value="">Year</option>"#.to_string())
        .chain(
            (min_year..=max_year)
                .rev()
                .map(|year| format!(r#"<option value="{year}">{year}</option>"#)),
        )
        .collect::<String>();

    format!(
        r#"<div class="birth-date-picker" data-birth-date-picker data-max-date="{max_date}">
  <input type="hidden" id="{hidden_id}" name="pet_birth_date" value="" />
  <div class="birth-date-picker-row" role="group" aria-label="Date of birth">
    <div class="birth-date-part">
      <label class="birth-date-part-label" for="{month_id}">Month</label>
      <select id="{month_id}" class="birth-date-part-select" data-birth-part="month" required autocomplete="bday-month">{month_options}</select>
    </div>
    <div class="birth-date-part">
      <label class="birth-date-part-label" for="{day_id}">Day</label>
      <select id="{day_id}" class="birth-date-part-select" data-birth-part="day" required autocomplete="bday-day">{day_options}</select>
    </div>
    <div class="birth-date-part birth-date-part-year">
      <label class="birth-date-part-label" for="{year_id}">Year</label>
      <select id="{year_id}" class="birth-date-part-select" data-birth-part="year" required autocomplete="bday-year">{year_options}</select>
    </div>
  </div>
</div>"#,
        hidden_id = escape_html_attr(hidden_id),
        month_id = escape_html_attr(month_id),
        day_id = escape_html_attr(day_id),
        year_id = escape_html_attr(year_id),
        max_date = today.format("%Y-%m-%d"),
        month_options = month_options,
        day_options = day_options,
        year_options = year_options,
    )
}

fn render_onboarding_modal(profile: &UserProfile) -> String {
    if !user_needs_pet_setup(profile) {
        return String::new();
    }

    let birth_date_picker = render_birth_date_picker(
        "pet_birth_date",
        "pet_birth_month",
        "pet_birth_day",
        "pet_birth_year",
    );
    let pet_color_picker = render_pet_color_picker("pet_color", "pet_color_select", "pet_color_custom");

    format!(
        r#"<div class="onboarding-backdrop" id="onboarding-modal" role="dialog" aria-modal="true" aria-labelledby="onboarding-title" hidden>
  <div class="onboarding-modal">
    <h2 id="onboarding-title">Tell us about your cat 🐾</h2>
    <p class="onboarding-intro">We will personalize your pet tab with a profile photo, breed, age, and lifestyle details. Upgrade to WhiskerWatch Plus later for health records and vet reminders.</p>
    <form class="onboarding-form login-form" action="/home/onboarding" method="post" enctype="multipart/form-data">
      <label for="cat_name">Cat's name</label>
      <input id="cat_name" name="cat_name" type="text" placeholder="Mochi" required />

      <fieldset class="pet-photo-fieldset pet-photo-fieldset-required">
        <legend>Cat profile photo <span class="pet-photo-required">Required</span></legend>
        <p class="field-hint">Every cat needs a profile photo for My Pet and your account. After you choose a photo, drag and zoom to frame your cat in the circle.</p>
        <div class="pet-photo-upload">
          <input id="pet_photo" name="pet_photo" type="file" class="pet-photo-input" accept="image/jpeg,image/png,image/webp,.jpg,.jpeg,.png,.webp" required />
          <label for="pet_photo" class="pet-photo-paw-btn" aria-label="Choose cat profile photo">
            <span class="pet-photo-paw-icon" aria-hidden="true">📷</span>
          </label>
          <p class="pet-photo-upload-cta">Tap to choose a photo</p>
        </div>
        <div id="onboarding-pet-photo-preview" class="pet-photo-preview" hidden aria-live="polite"></div>
      </fieldset>

      <label for="pet_breed">Cat breed</label>
      <input id="pet_breed" name="pet_breed" type="text" class="breed-picker-input" placeholder="Tap to choose a breed" required readonly />
      <p class="field-hint">Opens the breed picker so you can browse types and descriptions.</p>

      {pet_color_picker}

      <label for="pet_birth_month">Date of birth</label>
      {birth_date_picker}
      <p class="field-hint">We use your cat's birthday for vaccine scheduling and add a yearly birthday to your calendar.</p>

      <fieldset class="indoor-outdoor-fieldset">
        <legend>Indoor or outdoor cat?</legend>
        <label class="radio-pill"><input type="radio" name="pet_indoor_outdoor" value="indoor" required /> Indoor</label>
        <label class="radio-pill"><input type="radio" name="pet_indoor_outdoor" value="outdoor" required /> Outdoor</label>
      </fieldset>
      <p class="field-hint">Outdoor cats need FeLV vaccines yearly; indoor cats every 3 years after the first year. Vaccine tracking unlocks with WhiskerWatch Plus.</p>

      <fieldset class="pet-video-fieldset">
        <legend>Cat playing video</legend>
        <p class="field-hint">Upload a video of your cat playing, then pick a 3–6 second clip for the My Pet tab. MP4, WebM, or MOV up to 50MB.</p>
        <div class="pet-photo-upload">
          <input id="pet_video" name="pet_video" type="file" class="pet-photo-input" accept="video/mp4,video/webm,video/quicktime,.mp4,.webm,.mov" />
          <label for="pet_video" class="pet-photo-paw-btn" aria-label="Choose cat playing video">
            <span class="pet-photo-paw-icon" aria-hidden="true">🎬</span>
          </label>
        </div>
        <div id="pet-video-preview" class="pet-video-preview" hidden aria-live="polite"></div>
        <input type="hidden" id="pet_video_clip_start" name="pet_video_clip_start" value="0" />
        <input type="hidden" id="pet_video_clip_duration" name="pet_video_clip_duration" value="6" />
        <input type="hidden" id="pet_video_zoom" name="pet_video_zoom" value="" />
        <input type="hidden" id="pet_video_offset_x" name="pet_video_offset_x" value="" />
        <input type="hidden" id="pet_video_offset_y" name="pet_video_offset_y" value="" />
        <label class="checkbox-pill skip-photo-option">
          <input type="checkbox" id="skip_video" name="skip_video" value="on" />
          Skip video for now
        </label>
      </fieldset>

      <div class="onboarding-actions">
        <button type="submit" class="download-btn login-submit">Save &amp; continue</button>
        <button type="button" class="onboarding-secondary-btn" id="onboarding-skip">Skip for now</button>
      </div>
    </form>
  </div>
</div>"#,
        birth_date_picker = birth_date_picker,
        pet_color_picker = pet_color_picker,
    )
}

fn render_add_cat_onboarding_modal() -> String {
    let birth_date_picker = render_birth_date_picker(
        "add_cat_birth_date",
        "add_cat_birth_month",
        "add_cat_birth_day",
        "add_cat_birth_year",
    );
    let pet_color_picker =
        render_pet_color_picker("add_cat_color", "add_cat_color_select", "add_cat_color_custom");

    format!(
        r#"<div class="onboarding-backdrop" id="add-cat-modal" role="dialog" aria-modal="true" aria-labelledby="add-cat-title" hidden>
  <div class="onboarding-modal add-cat-modal">
    <h2 id="add-cat-title">Add another cat 🐾</h2>
    <p class="onboarding-intro">Same setup as your first cat — profile photo, breed, age, lifestyle, and optional playing video. Each kitty gets their own care tasks; your calendar stays shared.</p>
    <form class="onboarding-form login-form add-cat-onboarding-form" action="/home/onboarding" method="post" enctype="multipart/form-data">
      <input type="hidden" name="add_pet" value="1" />
      <label for="add_cat_name">Cat's name</label>
      <input id="add_cat_name" name="cat_name" type="text" placeholder="Luna" required maxlength="40" autocomplete="off" />

      <fieldset class="pet-photo-fieldset pet-photo-fieldset-required">
        <legend>Cat profile photo <span class="pet-photo-required">Required</span></legend>
        <p class="field-hint">Upload a photo of this cat, then drag and zoom to frame them in the circle.</p>
        <div class="pet-photo-upload">
          <input id="add_cat_photo" name="pet_photo" type="file" class="pet-photo-input" accept="image/jpeg,image/png,image/webp,.jpg,.jpeg,.png,.webp" required />
          <label for="add_cat_photo" class="pet-photo-paw-btn" aria-label="Choose cat profile photo">
            <span class="pet-photo-paw-icon" aria-hidden="true">📷</span>
          </label>
          <p class="pet-photo-upload-cta">Tap to choose a photo</p>
        </div>
        <div id="add-cat-photo-preview" class="pet-photo-preview" hidden aria-live="polite"></div>
      </fieldset>

      <label for="add_cat_breed">Cat breed</label>
      <input id="add_cat_breed" name="pet_breed" type="text" class="breed-picker-input" placeholder="Tap to choose a breed" required readonly />

      {pet_color_picker}

      <label for="add_cat_birth_month">Date of birth</label>
      {birth_date_picker}

      <fieldset class="indoor-outdoor-fieldset">
        <legend>Indoor or outdoor cat?</legend>
        <label class="radio-pill"><input type="radio" name="pet_indoor_outdoor" value="indoor" required /> Indoor</label>
        <label class="radio-pill"><input type="radio" name="pet_indoor_outdoor" value="outdoor" required /> Outdoor</label>
      </fieldset>

      <fieldset class="pet-video-fieldset">
        <legend>Cat playing video</legend>
        <p class="field-hint">Optional — upload a playing clip for this cat's My Pet tab.</p>
        <div class="pet-photo-upload">
          <input id="add_cat_video" name="pet_video" type="file" class="pet-photo-input" accept="video/mp4,video/webm,video/quicktime,.mp4,.webm,.mov" />
          <label for="add_cat_video" class="pet-photo-paw-btn" aria-label="Choose cat playing video">
            <span class="pet-photo-paw-icon" aria-hidden="true">🎬</span>
          </label>
        </div>
        <div id="add-cat-video-preview" class="pet-video-preview" hidden aria-live="polite"></div>
        <input type="hidden" name="pet_video_clip_start" value="0" />
        <input type="hidden" name="pet_video_clip_duration" value="6" />
        <input type="hidden" name="pet_video_zoom" value="" />
        <input type="hidden" name="pet_video_offset_x" value="" />
        <input type="hidden" name="pet_video_offset_y" value="" />
        <label class="checkbox-pill skip-photo-option">
          <input type="checkbox" name="skip_video" value="on" />
          Skip video for now
        </label>
      </fieldset>

      <div class="onboarding-actions">
        <button type="submit" class="download-btn login-submit">Save cat</button>
        <button type="button" class="onboarding-secondary-btn add-cat-cancel">← Cancel</button>
      </div>
    </form>
  </div>
</div>"#,
        birth_date_picker = birth_date_picker,
        pet_color_picker = pet_color_picker,
    )
}

fn household_pet_from_onboarding(
    form: &OnboardingForm,
    dob: NaiveDate,
    pet_age_weeks: Option<u32>,
    pet_age_years: Option<u32>,
    indoor_outdoor: String,
    premium: bool,
) -> HouseholdPet {
    let (never_been_to_vet, last_vet_date, pet_conditions, pet_medications, vaccine_history, pet_vaccines_unknown) =
        if premium {
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
                } else {
                    parse_vet_date(trimmed).map(|_| trimmed.to_string())
                }
            };
            (
                form.never_been_to_vet,
                last_vet_date,
                form.conditions.trim().to_string(),
                form.medications.trim().to_string(),
                vaccine_history,
                form.pet_vaccines_unknown,
            )
        } else {
            (false, None, String::new(), String::new(), vec![], false)
        };

    HouseholdPet {
        id: new_household_pet_id(),
        pet_name: form.cat_name.trim().to_string(),
        pet_breed: form.pet_breed.trim().to_string(),
        pet_color: form.pet_color.trim().to_string(),
        pet_mood: "Happy".to_string(),
        pet_age_weeks,
        pet_age_years,
        pet_birth_date: Some(dob.format("%Y-%m-%d").to_string()),
        last_vet_date,
        never_been_to_vet,
        pet_conditions,
        pet_medications,
        pet_indoor_outdoor: Some(indoor_outdoor),
        vaccine_history,
        pet_vaccines_unknown,
        care_schedule: default_care_schedule(),
        pet_photo_url: None,
        pet_video_url: None,
        pet_video_clip_start: None,
        pet_video_clip_duration: None,
        pet_video_zoom: None,
        pet_video_offset_x: None,
        pet_video_offset_y: None,
        deceased: false,
        deceased_at: None,
        memorial_videos: Vec::new(),
        memorial_comfort_seen: false,
    }
}

fn current_calendar_month() -> u32 {
    Local::now().month()
}

fn current_calendar_year() -> u32 {
    Local::now().year() as u32
}

fn calendar_month_label(month: u32, year: u32) -> String {
    let name = MONTH_NAMES
        .get(month.saturating_sub(1) as usize)
        .unwrap_or(&"???");
    format!("{name} {year} — your cat care schedule")
}

fn create_user_session(state: &AppState, jar: CookieJar, email: &str) -> CookieJar {
    let session_id = Uuid::new_v4().to_string();
    let now = timestamp_now();
    let expires_at = now.saturating_add(AUTH_SESSION_MAX_AGE_SECS as u64);
    if let Err(error) =
        state
            .storage
            .save_auth_session(&session_id, "user", Some(email), now, expires_at)
    {
        eprintln!("failed to persist user session: {error}");
    }

    let mut cookie = Cookie::new(USER_SESSION_COOKIE, session_id);
    apply_session_cookie_settings(&mut cookie);
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
        ensure_user_profile(state, &email);
        create_user_session(state, jar, &email)
    }
}

fn signed_in_redirect(state: &AppState, jar: CookieJar, email: &str) -> Response {
    let jar = complete_sign_in(state, jar, email);
    (jar, Redirect::to("/home")).into_response()
}

/// Baked in at compile time so `/` never depends on runtime cwd or a stale
/// `static/index.html` path left over from older binaries.
const MARKETING_HOME_HTML: &str = include_str!("../templates/marketing-home.html");
const DASHBOARD_HTML: &str = include_str!("../templates/dashboard.html");

fn render_share_card_modal() -> &'static str {
    r##"<div class="onboarding-backdrop share-card-backdrop" id="share-card-modal" role="dialog" aria-modal="true" aria-labelledby="share-card-title" hidden>
  <div class="onboarding-modal share-card-modal">
    <button type="button" class="parent-level-close share-card-close" id="share-card-close" aria-label="Close share card">&times;</button>
    <h2 id="share-card-title">Share your win!</h2>
    <p class="share-card-intro">Spread the word and invite friends to WhiskerWatch.</p>
    <article class="share-card-preview" id="share-card-preview" aria-live="polite"></article>
    <div class="share-card-actions">
      <button type="button" class="download-btn" id="share-card-copy">Copy link</button>
      <button type="button" class="download-btn share-card-native-btn" id="share-card-native" hidden>Share…</button>
      <a class="download-btn share-card-tweet-btn" id="share-card-tweet" href="#" target="_blank" rel="noopener noreferrer">Post on X</a>
    </div>
    <button type="button" class="onboarding-skip-btn share-card-dismiss" id="share-card-dismiss">Maybe later</button>
  </div>
</div>"##
}

async fn share_card_page(Path(token): Path<String>) -> impl IntoResponse {
    let Some(payload) = share_cards::decode_share_token(token.trim()) else {
        return Redirect::to("/").into_response();
    };

    let base = stripe_payments::public_app_url();
    let signup_url = format!("{base}/signup");
    Html(share_cards::render_share_page_html(&payload, &signup_url)).into_response()
}

#[derive(Deserialize, Default)]
struct StreakKeepGoingQuery {
    status: Option<String>,
    points: Option<u32>,
}

#[derive(Deserialize)]
struct StreakRewardClaimForm {
    milestone: String,
}

async fn streak_keep_going_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<StreakKeepGoingQuery>,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let profile = get_or_create_profile(&state, &email).await;
    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let pet_name = display_pet_name(&profile);

    let html = streak_rewards::render_keep_going_content(
        &profile,
        &pet_name,
        query.status.as_deref(),
        query.points,
    )
    .replace("{{USER_NAME}}", &escape_html(&username))
    .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar));

    (jar, Html(html)).into_response()
}

async fn streak_reward_claim_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<StreakRewardClaimForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let milestone = form.milestone.trim().parse::<u32>().unwrap_or(0);
    let mut profile = get_or_create_profile(&state, &email).await;

    let redirect = match streak_rewards::claim_streak_reward(&mut profile, milestone) {
        Ok(points) => match save_profile(&state, &profile).await {
            Ok(()) => Redirect::to(&format!(
                "/home/streak?status=streak_reward_claimed&points={points}"
            )),
            Err(_) => Redirect::to("/home/streak?status=streak_reward_invalid"),
        },
        Err(streak_rewards::ClaimError::NotReached) => {
            Redirect::to("/home/streak?status=streak_reward_locked")
        }
        Err(streak_rewards::ClaimError::AlreadyClaimed) => {
            Redirect::to("/home/streak?status=streak_reward_claimed_already")
        }
        Err(streak_rewards::ClaimError::InvalidMilestone) => {
            Redirect::to("/home/streak?status=streak_reward_invalid")
        }
    };

    redirect
}

async fn index_page(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if user_session_email(&state, &jar).is_some() || admin_session_valid(&state, &jar) {
        return Redirect::to("/home").into_response();
    }

    let html = apply_auth_nav_link(MARKETING_HOME_HTML, &state, &jar);
    Html(html).into_response()
}

fn dashboard_status_block(status: Option<&str>) -> String {
    match status {
        Some("outfit_bought") => {
            r#"<p class="auth-success status-flash" role="status">Outfit purchased and equipped! Your pet looks adorable.</p>"#
        }
        Some("outfit_equipped") => {
            r#"<p class="auth-success status-flash" role="status">Outfit equipped for your pet.</p>"#
        }
        Some("outfit_owned") => {
            r#"<p class="auth-error status-flash" role="alert">You already own that outfit.</p>"#
        }
        Some("outfit_points") => {
            r#"<p class="auth-error status-flash" role="alert">Not enough paw points for that outfit.</p>"#
        }
        Some("outfit_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That outfit is not available.</p>"#
        }
        Some("guide_bought") => {
            r#"<p class="auth-success status-flash" role="status">Yay! Your premium breed care guide is unlocked! 🐾</p>"#
        }
        Some("guide_cancelled") => {
            r#"<p class="auth-error status-flash" role="alert">Checkout was cancelled. Your guide is still available to unlock anytime.</p>"#
        }
        Some("guide_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That breed guide is not available.</p>"#
        }
        Some("points_bought") => {
            r#"<p class="auth-success status-flash" role="status">Payment received! Paw points have been added to your account.</p>"#
        }
        Some("points_cancelled") => {
            r#"<p class="auth-error status-flash" role="alert">Checkout was cancelled. No charge was made.</p>"#
        }
        Some("points_checkout_failed") => {
            r#"<p class="auth-error status-flash" role="alert">Could not start checkout. Try again or contact support.</p>"#
        }
        Some("points_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That point package is not available.</p>"#
        }
        Some("payments_unconfigured") => {
            r#"<p class="auth-error status-flash" role="alert">Payments are not configured on this server yet.</p>"#
        }
        Some("task_done") => "",
        Some("task_reopened") => {
            r#"<p class="auth-success status-flash" role="status">Task marked as incomplete. Paw points for that task were deducted.</p>"#
        }
        Some("task_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That task could not be updated.</p>"#
        }
        Some("task_time_saved") => {
            r#"<p class="auth-success status-flash" role="status">Task time updated.</p>"#
        }
        Some("task_time_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That task time could not be updated.</p>"#
        }
        Some("task_added") => {
            r#"<p class="auth-success status-flash" role="status">Custom care task added.</p>"#
        }
        Some("task_deleted") => {
            r#"<p class="auth-success status-flash" role="status">Custom care task removed.</p>"#
        }
        Some("task_add_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Enter a short task name to add.</p>"#
        }
        Some("task_delete_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Only custom tasks can be deleted.</p>"#
        }
        Some("onboarding_done") => {
            r#"<p class="auth-success status-flash" role="status">Welcome! Your cat profile is saved. Upgrade to WhiskerWatch Plus anytime for health records and vet reminders.</p>"#
        }
        Some("premium_bought") => {
            r#"<p class="auth-success status-flash" role="status">Welcome to WhiskerWatch Plus! Health records, vet logging, and multi-pet support are unlocked.</p>"#
        }
        Some("premium_cancelled") => {
            r#"<p class="auth-error status-flash" role="alert">Checkout was cancelled. WhiskerWatch Plus is still available when you are ready.</p>"#
        }
        Some("premium_required") => {
            r#"<p class="auth-error status-flash" role="alert">That feature requires WhiskerWatch Plus. Upgrade on the Account tab.</p>"#
        }
        Some("premium_owned") => {
            r#"<p class="auth-success status-flash" role="status">You already have WhiskerWatch Plus.</p>"#
        }
        Some("premium_checkout_failed") => {
            r#"<p class="auth-error status-flash" role="alert">Could not start premium checkout. Try again or contact support.</p>"#
        }
        Some("premium_fulfill_failed") => {
            r#"<p class="auth-error status-flash" role="alert">Payment received, but Plus could not be activated automatically. Refresh in a moment or contact support with your receipt.</p>"#
        }
        Some("pet_added") => {
            r#"<p class="auth-success status-flash" role="status">New cat added to your household!</p>"#
        }
        Some("pet_add_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Could not add that cat. Check the name and breed, or upgrade for more slots.</p>"#
        }
        Some("community_visibility_saved") => {
            r#"<p class="auth-success status-flash" role="status">Community privacy setting saved.</p>"#
        }
        Some("memorial_started") => {
            r#"<p class="auth-success status-flash" role="status">A gentle memorial space is ready on the Account tab. You are loved, and it will be okay.</p>"#
        }
        Some("memorial_video_saved") => {
            r#"<p class="auth-success status-flash" role="status">Memory clip saved. Tap their memorial photo to cycle through clips.</p>"#
        }
        Some("memorial_video_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That memory clip could not be saved. Try MP4, WebM, or MOV under 50MB.</p>"#
        }
        Some("memorial_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That memorial update could not be saved.</p>"#
        }
        Some("friend_request_sent") => {
            r#"<p class="auth-success status-flash" role="status">Friend request sent! They will see it on their Friends tab.</p>"#
        }
        Some("friend_accepted") => {
            r#"<p class="auth-success status-flash" role="status">You are now friends — you can share cats from the Friends tab.</p>"#
        }
        Some("friend_declined") => {
            r#"<p class="auth-success status-flash" role="status">Friend request declined.</p>"#
        }
        Some("friend_not_found") => {
            r#"<p class="auth-error status-flash" role="alert">No WhiskerWatch account matches that email or username yet.</p>"#
        }
        Some("friend_already") => {
            r#"<p class="auth-error status-flash" role="alert">You are already friends with that person.</p>"#
        }
        Some("friend_pending") => {
            r#"<p class="auth-error status-flash" role="alert">A friend request is already waiting between you two.</p>"#
        }
        Some("friend_self") => {
            r#"<p class="auth-error status-flash" role="alert">You cannot send a friend request to yourself.</p>"#
        }
        Some("friend_request_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Could not send that friend request. Check the email or username and try again.</p>"#
        }
        Some("share_sent") => {
            r#"<p class="auth-success status-flash" role="status">Care share invite sent! Your friend can accept access to this cat's tasks, schedule, and health records on their Friends tab.</p>"#
        }
        Some("share_accepted") => {
            r#"<p class="auth-success status-flash" role="status">Care share accepted — switch to the shared cat on My Pet to complete tasks and view their calendar.</p>"#
        }
        Some("share_declined") => {
            r#"<p class="auth-success status-flash" role="status">Cat share invite declined.</p>"#
        }
        Some("share_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Could not share that cat. Make sure you are friends and selected a valid cat.</p>"#
        }
        Some("share_not_friends") => {
            r#"<p class="auth-error status-flash" role="alert">You can only share cats with accepted friends.</p>"#
        }
        Some("share_already") => {
            r#"<p class="auth-error status-flash" role="alert">That cat is already shared or has a pending invite with this friend.</p>"#
        }
        Some("share_revoked") => {
            r#"<p class="auth-success status-flash" role="status">Stopped sharing that cat's tasks and schedule.</p>"#
        }
        Some("notification_prefs_saved") => {
            r#"<p class="auth-success status-flash" role="status">Notification settings saved.</p>"#
        }
        Some("notification_prefs_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Could not save notification settings. Check your daily check-in times (up to 5, no duplicates).</p>"#
        }
        Some("push_subscribed") => {
            r#"<p class="auth-success status-flash" role="status">Push notifications enabled for this browser.</p>"#
        }
        Some("onboarding_emails_saved") => {
            r#"<p class="auth-success status-flash" role="status">Onboarding email preferences saved.</p>"#
        }
        Some("onboarding_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Please enter your cat's name, breed, a profile photo, a valid age, and whether they are indoor or outdoor.</p>"#
        }
        Some("onboarding_photo_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Please choose a profile photo. Use a JPEG, PNG, or WebP under 5MB.</p>"#
        }
        Some("onboarding_video_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That video could not be saved. Use an MP4, WebM, or MOV under 50MB with at least 3 seconds of footage, or skip the video.</p>"#
        }
        Some("pet_video_done") => {
            r#"<p class="auth-success status-flash" role="status">Playing video saved! Tap your profile photo on the Account tab or "Watch my kitty play!" on My Pet to watch it loop.</p>"#
        }
        Some("pet_video_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That video could not be saved. Use an MP4, WebM, or MOV under 50MB with at least 3 seconds of footage.</p>"#
        }
        Some("pet_photo_done") => {
            r#"<p class="auth-success status-flash" role="status">Profile photo updated!</p>"#
        }
        Some("pet_photo_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That photo could not be saved. Use a JPEG, PNG, or WebP under 5MB.</p>"#
        }
        Some("password_done") => {
            r#"<p class="auth-success status-flash" role="status">Password updated successfully.</p>"#
        }
        Some("password_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Current password is incorrect.</p>"#
        }
        Some("password_mismatch") => {
            r#"<p class="auth-error status-flash" role="alert">New password and confirmation do not match.</p>"#
        }
        Some("password_requirements") => {
            r#"<p class="auth-error status-flash" role="alert">New password must be at least 5 characters and include one number and one special character.</p>"#
        }
        Some("password_same") => {
            r#"<p class="auth-error status-flash" role="alert">Choose a new password that is different from your current one.</p>"#
        }
        Some("password_missing") => {
            r#"<p class="auth-error status-flash" role="alert">Enter your current password and a new password.</p>"#
        }
        Some("password_error") => {
            r#"<p class="auth-error status-flash" role="alert">Could not update your password right now. Please try again.</p>"#
        }
        Some("pet_name_done") => {
            r#"<p class="auth-success status-flash" role="status">Pet name updated!</p>"#
        }
        Some("pet_name_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Enter a pet name up to 40 characters.</p>"#
        }
        Some("vet_visit_done") => {
            r#"<p class="auth-success status-flash" role="status">Vet visit saved! Vaccines and health notes updated.</p>"#
        }
        Some("vet_visit_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Could not save vet visit. Check vaccine dates and try again.</p>"#
        }
        Some("vet_notes_done") => {
            r#"<p class="auth-success status-flash" role="status">Vet notes saved.</p>"#
        }
        Some("vet_notes_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">Could not save vet notes. Please try again.</p>"#
        }
        Some("feedback_sent") => {
            r#"<p class="auth-success status-flash" role="status">Your feedback was posted to the public forum.</p>"#
        }
        Some("feedback_missing") => {
            r#"<p class="auth-error status-flash" role="alert">Please fill out all feedback fields.</p>"#
        }
        Some("feedback_failed") => {
            r#"<p class="auth-error status-flash" role="alert">We could not save your feedback. Please try again.</p>"#
        }
        Some("feedback_idea_purrfect") => {
            r#"<p class="auth-success status-flash" role="status">Your idea is purrfect! +200 paw points.</p>"#
        }
        Some("forum_post_sent") => {
            r#"<p class="auth-success status-flash" role="status">Your question was posted to the forum.</p>"#
        }
        Some("forum_reply_sent") => {
            r#"<p class="auth-success status-flash" role="status">Your reply was posted.</p>"#
        }
        Some("forum_missing") => {
            r#"<p class="auth-error status-flash" role="alert">Please enter a title and question details.</p>"#
        }
        Some("forum_reply_missing") => {
            r#"<p class="auth-error status-flash" role="alert">Please enter a reply.</p>"#
        }
        Some("forum_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That forum thread could not be found.</p>"#
        }
        Some("forum_failed") => {
            r#"<p class="auth-error status-flash" role="alert">We could not save your forum post. Please try again.</p>"#
        }
        Some("forum_post_deleted") => {
            r#"<p class="auth-success status-flash" role="status">Your question was deleted.</p>"#
        }
        Some("forum_reply_deleted") => {
            r#"<p class="auth-success status-flash" role="status">Your answer was deleted.</p>"#
        }
        Some("forum_delete_denied") => {
            r#"<p class="auth-error status-flash" role="alert">You can only delete your own questions and answers.</p>"#
        }
        Some("forum_delete_failed") => {
            r#"<p class="auth-error status-flash" role="alert">We could not delete that forum item. Please try again.</p>"#
        }
        Some("calendar_event_added") => {
            r#"<p class="auth-success status-flash" role="status">Event saved to your calendar.</p>"#
        }
        Some("calendar_event_missing") => {
            r#"<p class="auth-error status-flash" role="alert">Enter a task name before saving.</p>"#
        }
        Some("calendar_event_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That calendar date could not be saved. Pick a day and try again.</p>"#
        }
        Some("calendar_event_failed") => {
            r#"<p class="auth-error status-flash" role="alert">We could not save your calendar event. Please try again.</p>"#
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
    html.push_str(&render_vet_urgency_alert(
        profile,
        "dashboard-vaccine-alert",
    ));
    html
}

const PURRFECT_IDEA_UPVOTES: u32 = 5;
const PURRFECT_IDEA_REWARD: u32 = 200;

fn feedback_author_email(submission: &FeedbackSubmission) -> String {
    submission
        .user_id
        .clone()
        .unwrap_or_else(|| submission.email.clone())
}

async fn maybe_grant_purrfect_idea_reward(state: &AppState, feedback_id: i64, upvotes: u32) {
    if upvotes < PURRFECT_IDEA_UPVOTES {
        return;
    }
    if state
        .storage
        .feedback_reward_granted(feedback_id)
        .unwrap_or(false)
    {
        return;
    }

    let Ok(Some(submission)) = state.storage.get_feedback_submission(feedback_id) else {
        return;
    };
    if submission.category != "idea" {
        return;
    }

    let author_email = feedback_author_email(&submission);
    if let Ok(Some(mut profile)) = state.storage.load_profile(&author_email) {
        profile.paw_points += PURRFECT_IDEA_REWARD;
        if !profile.pending_purrfect_idea_ids.contains(&feedback_id) {
            profile.pending_purrfect_idea_ids.push(feedback_id);
        }
        push_activity(&mut profile, "Your idea is purrfect! +200 paw points.");
        let _ = save_profile(state, &profile).await;
    }

    let _ = state.storage.mark_feedback_reward_granted(feedback_id);
}

fn feedback_category_label(category: &str) -> &'static str {
    match category {
        "fix" => "Something to fix",
        "idea" => "New idea",
        "bug" => "Bug report",
        _ => "Feedback",
    }
}

fn feedback_message_preview(message: &str) -> String {
    let collapsed = message.trim().replace(['\n', '\r'], " ");
    let max_chars = 90usize;
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        format!("{}…", collapsed.chars().take(max_chars).collect::<String>())
    }
}

fn render_feedback_vote_controls(
    entry: &storage::FeedbackForumEntry,
    voter_email: Option<&str>,
) -> String {
    let id = entry.submission.id;
    let up_active = entry.user_vote == Some(1);
    let down_active = entry.user_vote == Some(-1);
    let up_pressed = if up_active {
        r#" aria-pressed="true""#
    } else {
        ""
    };
    let down_pressed = if down_active {
        r#" aria-pressed="true""#
    } else {
        ""
    };
    let up_class = if up_active {
        "feedback-vote-btn feedback-vote-up is-active"
    } else {
        "feedback-vote-btn feedback-vote-up"
    };
    let down_class = if down_active {
        "feedback-vote-btn feedback-vote-down is-active"
    } else {
        "feedback-vote-btn feedback-vote-down"
    };
    let vote_blocked = if voter_email.is_none() {
        r#" data-vote-blocked="login""#
    } else {
        ""
    };

    format!(
        r#"<div class="feedback-votes" data-feedback-id="{id}"{vote_blocked}>
            <button type="button" class="{up_class}" data-vote="up"{up_pressed} aria-label="Upvote">▲ {upvotes}</button>
            <button type="button" class="{down_class}" data-vote="down"{down_pressed} aria-label="Downvote">▼ {downvotes}</button>
          </div>"#,
        id = id,
        vote_blocked = vote_blocked,
        up_class = up_class,
        down_class = down_class,
        up_pressed = up_pressed,
        down_pressed = down_pressed,
        upvotes = entry.upvotes,
        downvotes = entry.downvotes,
    )
}

fn render_feedback_post(
    entry: &storage::FeedbackForumEntry,
    open: bool,
    voter_email: Option<&str>,
) -> String {
    let item = &entry.submission;
    let open_attr = if open { " open" } else { "" };
    let votes = render_feedback_vote_controls(entry, voter_email);
    format!(
        r#"<article class="feedback-forum-item" data-feedback-id="{id}">
          <div class="feedback-forum-row">
            <details class="feedback-forum-post"{open_attr}>
              <summary class="feedback-forum-summary">
                <span class="feedback-forum-category">{category}</span>
                <span class="feedback-forum-preview">{preview}</span>
                <span class="feedback-forum-meta">by {author} · {when}</span>
              </summary>
              <div class="feedback-forum-body">
                <p>{message}</p>
              </div>
            </details>
            {votes}
          </div>
        </article>"#,
        open_attr = open_attr,
        id = item.id,
        category = escape_html(feedback_category_label(&item.category)),
        preview = escape_html(&feedback_message_preview(&item.message)),
        votes = votes,
        author = escape_html(&item.author_username),
        when = escape_html(&format_timestamp(item.submitted_at)),
        message = escape_html(&item.message),
    )
}

fn render_feedback_forum(
    state: &AppState,
    form_name: &str,
    form_email: &str,
    open_post: Option<i64>,
    voter_email: Option<&str>,
) -> String {
    let posts = state
        .storage
        .load_feedback_forum(voter_email)
        .unwrap_or_default();

    let mut list = String::new();
    if posts.is_empty() {
        list.push_str(
            r#"<p class="feedback-forum-empty">No public feedback yet. Share the first idea with the community!</p>"#,
        );
    } else {
        for post in &posts {
            let open = open_post.is_some_and(|id| id == post.submission.id);
            list.push_str(&render_feedback_post(post, open, voter_email));
        }
    }

    format!(
        r#"<h1>Feedback Forum</h1>
        <p class="panel-intro">Post feedback publicly so other WhiskerWatch parents can see ideas, bugs, and fixes in the works.</p>
        <article class="dashboard-card feedback-forum-compose">
          <h2>Post feedback</h2>
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

            <button type="submit" class="download-btn login-submit">Post to forum</button>
          </form>
        </article>
        <div class="feedback-forum-list">
          <h2 class="feedback-forum-list-title">Community feedback</h2>
          {list}
        </div>"#,
        form_name = form_name,
        form_email = form_email,
        list = list,
    )
}

fn forum_user_owns(content_user_id: &str, current_user_id: &str) -> bool {
    content_user_id.eq_ignore_ascii_case(current_user_id)
}

fn render_forum_reply(reply: &ForumReply, current_user_id: &str) -> String {
    let delete_form = if forum_user_owns(&reply.user_id, current_user_id) {
        format!(
            r#"<form class="forum-delete-form" action="/home/forum/reply/delete" method="post" onsubmit="return confirm('Delete your answer?');">
              <input type="hidden" name="reply_id" value="{reply_id}" />
              <input type="hidden" name="post_id" value="{post_id}" />
              <button type="submit" class="forum-delete-btn">Delete answer</button>
            </form>"#,
            reply_id = reply.id,
            post_id = reply.post_id,
        )
    } else {
        String::new()
    };

    format!(
        r#"<li class="forum-reply">
          <div class="forum-reply-header">
            <p class="forum-reply-meta"><strong>{author}</strong> · {when}</p>
            {delete_form}
          </div>
          <p class="forum-reply-body">{body}</p>
        </li>"#,
        author = escape_html(&reply.author_username),
        when = escape_html(&format_timestamp(reply.created_at)),
        body = escape_html(&reply.body),
        delete_form = delete_form,
    )
}

fn render_forum_thread(
    post: &ForumPost,
    replies: &[ForumReply],
    reply_count: u32,
    open: bool,
    current_user_id: &str,
) -> String {
    let open_attr = if open { " open" } else { "" };
    let answer_label = if reply_count == 1 {
        "1 answer".to_string()
    } else {
        format!("{reply_count} answers")
    };
    let replies_html: String = replies
        .iter()
        .map(|reply| render_forum_reply(reply, current_user_id))
        .collect();
    let replies_block = if replies.is_empty() {
        r#"<p class="forum-no-replies">No answers yet — be the first to help!</p>"#.to_string()
    } else {
        format!(
            r#"<ul class="forum-replies">{replies_html}</ul>"#,
            replies_html = replies_html
        )
    };
    let delete_question_form = if forum_user_owns(&post.user_id, current_user_id) {
        format!(
            r#"<form class="forum-delete-form forum-delete-form-question" action="/home/forum/post/delete" method="post" onsubmit="return confirm('Delete this question and all its answers?');">
              <input type="hidden" name="post_id" value="{id}" />
              <button type="submit" class="forum-delete-minus" aria-label="Delete question" title="Delete question" onclick="event.stopPropagation();">−</button>
            </form>"#,
            id = post.id,
        )
    } else {
        String::new()
    };

    let breed_badge = community::render_breed_badge(&post.breed_slug);

    format!(
        r#"<details class="forum-thread"{open_attr} data-post-id="{id}">
          <summary class="forum-thread-summary">
            <span class="forum-thread-summary-text">
              <span class="forum-thread-title">{title}</span>
              <span class="forum-thread-meta">{breed_badge} · by {author} · {when} · {answers}</span>
            </span>
            {delete_question_form}
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
        delete_question_form = delete_question_form,
        replies_block = replies_block,
        breed_badge = breed_badge,
    )
}

fn normalize_community_section(value: Option<&str>) -> &'static str {
    match value.map(str::trim).filter(|part| !part.is_empty()) {
        Some("forum") => "forum",
        Some("friends") => "friends",
        _ => "cats",
    }
}

fn render_forum_threads_html(
    state: &AppState,
    posts: &[ForumPost],
    open_thread: Option<i64>,
    current_user_id: &str,
    empty_message: &str,
) -> String {
    if posts.is_empty() {
        return format!(r#"<p class="forum-empty">{empty_message}</p>"#);
    }

    let mut threads = String::new();
    for post in posts {
        let replies = state
            .storage
            .list_forum_replies(post.id)
            .unwrap_or_default();
        let reply_count = state
            .storage
            .count_forum_replies(post.id)
            .unwrap_or(replies.len() as u32);
        let open = open_thread.is_some_and(|id| id == post.id);
        threads.push_str(&render_forum_thread(
            post,
            &replies,
            reply_count,
            open,
            current_user_id,
        ));
    }
    threads
}

fn resolve_forum_breed_slug(form_slug: &str, profile: &UserProfile) -> String {
    let trimmed = form_slug.trim();
    if !trimmed.is_empty() {
        return community::breed_slug_for_name(trimmed);
    }
    if profile_has_pet(profile) {
        return community::breed_slug_for_name(&profile.pet_breed);
    }
    String::new()
}

fn render_dashboard_forum_tab(
    state: &AppState,
    profile: &UserProfile,
    open_thread: Option<i64>,
    current_user_id: &str,
    community_section: &str,
    breed_filter: Option<&str>,
) -> String {
    let cats_active = if community_section == "cats" {
        " active"
    } else {
        ""
    };
    let forum_active = if community_section == "forum" {
        " active"
    } else {
        ""
    };
    let friends_active = if community_section == "friends" {
        " active"
    } else {
        ""
    };
    let breed_query = breed_filter
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|slug| format!("&breed={}", urlencoding::encode(slug)))
        .unwrap_or_default();

    let cats_panel = if community_section == "cats" {
        community::render_cat_feed_section(state, current_user_id, profile, breed_filter)
    } else {
        String::new()
    };

    let forum_panel = if community_section == "forum" {
        let posts = state
            .storage
            .list_forum_posts(breed_filter)
            .unwrap_or_default();
        let breed_label = community::breed_label_for_slug(breed_filter.unwrap_or(""));
        let empty_message = if breed_filter.map(str::trim).filter(|v| !v.is_empty()).is_some() {
            format!(
                "No {breed_label} questions yet. Start the conversation for your breed!",
                breed_label = breed_label.to_lowercase(),
            )
        } else {
            "No community questions yet. Start the conversation for your breed!".to_string()
        };
        let threads =
            render_forum_threads_html(state, &posts, open_thread, current_user_id, &empty_message);

        let breed_options =
            community::render_breed_filter_options(breed_filter.unwrap_or(""), profile);
        format!(
            r#"<section class="community-section community-section-forum" id="community-forum-panel">
  <div class="community-section-header">
    <h2>Breed Q&amp;A</h2>
    <p class="field-hint">Ask breed-specific questions and learn from other parents caring for similar cats.</p>
  </div>
  <form class="community-breed-filter login-form" action="/home" method="get">
    <input type="hidden" name="tab" value="forum" />
    <input type="hidden" name="community" value="forum" />
    <label for="community-forum-breed">Show questions for</label>
    <select id="community-forum-breed" name="breed" onchange="this.form.submit()">{breed_options}</select>
  </form>
  <details class="dashboard-card forum-ask-card">
    <summary class="forum-ask-summary">
      <span class="forum-ask-summary-text">Ask a question</span>
    </summary>
    <div class="forum-ask-body">
      <form class="login-form forum-ask-form" action="/home/forum/post" method="post">
        <label for="forum-title">Question title</label>
        <input id="forum-title" name="title" type="text" placeholder="e.g. How often should I brush my Persian?" required maxlength="200" />
        <label for="forum-breed">Breed topic</label>
        <select id="forum-breed" name="breed_slug">{breed_options}</select>
        <p class="field-hint">Defaults to your cat's breed when left on "All breeds".</p>
        <label for="forum-body">Details</label>
        <textarea id="forum-body" name="body" rows="4" placeholder="Tell us more about your pet and what you need help with..." required maxlength="4000"></textarea>
        <button type="submit" class="download-btn login-submit">Post question</button>
      </form>
    </div>
  </details>
  <div class="forum-list">{threads}</div>
</section>"#,
            breed_options = breed_options,
            threads = threads,
        )
    } else {
        String::new()
    };

    let friends_panel = if community_section == "friends" {
        let friend_emails: std::collections::HashSet<String> = sharing::accepted_friend_emails(
            state,
            current_user_id,
        )
        .into_iter()
        .map(|email| email.to_lowercase())
        .collect();

        if friend_emails.is_empty() {
            r#"<section class="community-section community-section-friends" id="community-friends-panel">
  <div class="community-section-header">
    <h2>Friends' posts</h2>
    <p class="field-hint">Questions and advice from people you've connected with on WhiskerWatch.</p>
  </div>
  <div class="community-friends-empty">
    <p class="community-friends-empty-emoji" aria-hidden="true">🐾💕🐾</p>
    <p class="community-friends-empty-title">No friend posts yet — but your cat crew is waiting!</p>
    <p class="community-friends-empty-copy">Connect with other cat parents, then their Breed Q&amp;A questions will show up here like a cozy little bulletin board.</p>
    <a href="/home?tab=friends" class="community-friends-cta">Find cat friends on the Friends tab 🐱✨</a>
  </div>
</section>"#
                .to_string()
        } else {
            let posts = state
                .storage
                .list_forum_posts(breed_filter)
                .unwrap_or_default()
                .into_iter()
                .filter(|post| friend_emails.contains(&post.user_id.to_lowercase()))
                .collect::<Vec<_>>();
            let breed_label = community::breed_label_for_slug(breed_filter.unwrap_or(""));
            let empty_message = if breed_filter.map(str::trim).filter(|v| !v.is_empty()).is_some() {
                format!(
                    "None of your friends have posted {breed_label} questions yet.",
                    breed_label = breed_label.to_lowercase(),
                )
            } else {
                "Your friends haven't posted in Breed Q&A yet. Check back after they ask a question!".to_string()
            };
            let threads =
                render_forum_threads_html(state, &posts, open_thread, current_user_id, &empty_message);
            let breed_options =
                community::render_breed_filter_options(breed_filter.unwrap_or(""), profile);
            format!(
                r#"<section class="community-section community-section-friends" id="community-friends-panel">
  <div class="community-section-header">
    <h2>Friends' posts</h2>
    <p class="field-hint">Questions and advice from your WhiskerWatch friends.</p>
  </div>
  <form class="community-breed-filter login-form" action="/home" method="get">
    <input type="hidden" name="tab" value="forum" />
    <input type="hidden" name="community" value="friends" />
    <label for="community-friends-breed">Show posts for</label>
    <select id="community-friends-breed" name="breed" onchange="this.form.submit()">{breed_options}</select>
  </form>
  <div class="forum-list">{threads}</div>
</section>"#,
                breed_options = breed_options,
                threads = threads,
            )
        }
    } else {
        String::new()
    };

    format!(
        r#"<h1>Community</h1>
<p class="panel-intro">Meet other WhiskerWatch cats and swap breed-specific care advice.</p>
<nav class="community-subtabs" aria-label="Community views">
  <a class="community-subtab{cats_active}" href="/home?tab=forum&amp;community=cats{breed_query}">Community cats</a>
  <a class="community-subtab{forum_active}" href="/home?tab=forum&amp;community=forum{breed_query}">Breed Q&amp;A</a>
  <a class="community-subtab{friends_active}" href="/home?tab=forum&amp;community=friends{breed_query}">Friends' posts</a>
</nav>
<div class="community-panels">
  {cats_panel}
  {forum_panel}
  {friends_panel}
</div>"#,
        cats_active = cats_active,
        forum_active = forum_active,
        friends_active = friends_active,
        breed_query = breed_query,
        cats_panel = cats_panel,
        forum_panel = forum_panel,
        friends_panel = friends_panel,
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

fn shop_return_url(return_to: Option<&str>) -> &'static str {
    match return_to.unwrap_or("").trim() {
        "dashboard" | "home" => "/home?tab=pet",
        _ => "/home/cat-home",
    }
}

fn shop_item_from_cat_home_query(query: &CatHomeQuery) -> Option<ShopItemQuote> {
    shop_item_from_query(&NeedPawPointsQuery {
        decor_id: query.decor_id.clone(),
        outfit_id: query.outfit_id.clone(),
        boost_id: query.boost_id.clone(),
        petting_bonus_id: query.petting_bonus_id.clone(),
        return_to: None,
    })
}

fn cat_home_need_paw_points_redirect(
    decor_id: Option<&str>,
    outfit_id: Option<&str>,
    boost_id: Option<&str>,
    petting_bonus_id: Option<&str>,
) -> Redirect {
    let mut query = "status=need_paw_points".to_string();
    if let Some(id) = decor_id.filter(|id| !id.trim().is_empty()) {
        query.push_str(&format!("&decor_id={}", urlencoding::encode(id.trim())));
    } else if let Some(id) = outfit_id.filter(|id| !id.trim().is_empty()) {
        query.push_str(&format!("&outfit_id={}", urlencoding::encode(id.trim())));
    } else if let Some(id) = boost_id.filter(|id| !id.trim().is_empty()) {
        query.push_str(&format!("&boost_id={}", urlencoding::encode(id.trim())));
    } else if let Some(id) = petting_bonus_id.filter(|id| !id.trim().is_empty()) {
        query.push_str(&format!(
            "&petting_bonus_id={}",
            urlencoding::encode(id.trim())
        ));
    }
    Redirect::to(&format!("/home/cat-home?{query}"))
}

fn render_shop_points_shortfall_trigger(name: &str, price: u32, emoji: &str) -> String {
    format!(
        r#"<button type="button" class="shop-points-shortfall-btn need-paw-points-trigger" data-item-name="{name}" data-item-price="{price}" data-item-emoji="{emoji}">🐾 Not quite enough — get more paw points</button>"#,
        name = escape_html_attr(name),
        price = price,
        emoji = emoji,
    )
}

fn shop_purchase_data_attrs(
    kind: &str,
    id: &str,
    price: u32,
    name: &str,
    emoji: &str,
    return_to: Option<&str>,
) -> String {
    format!(
        r#" data-shop-purchasable="true" data-shop-kind="{kind}" data-shop-id="{id}" data-shop-price="{price}" data-shop-name="{name}" data-shop-emoji="{emoji}" data-shop-return-to="{return_to}""#,
        kind = escape_html_attr(kind),
        id = escape_html_attr(id),
        price = price,
        name = escape_html_attr(name),
        emoji = escape_html_attr(emoji),
        return_to = escape_html_attr(return_to.unwrap_or("")),
    )
}

fn render_need_paw_points_modal(
    profile: &UserProfile,
    auto_open_item: Option<&ShopItemQuote>,
) -> String {
    let balance = profile.paw_points;
    let mut auto_open_attrs = String::new();
    let mut hero_emoji = "🐾".to_string();
    let mut item_name = String::new();
    let mut item_price = String::new();
    let mut points_needed = String::new();

    if let Some(item) = auto_open_item {
        let shortfall = item.price.saturating_sub(balance);
        if shortfall > 0 {
            auto_open_attrs.push_str(r#" data-auto-open="true""#);
        }
        item_name = escape_html_attr(item.name);
        item_price = item.price.to_string();
        points_needed = shortfall.to_string();
        if let Some(decor) = DECOR_CATALOG.iter().find(|entry| entry.name == item.name) {
            hero_emoji = decor.emoji.to_string();
        } else if let Some(outfit) = OUTFIT_CATALOG.iter().find(|entry| entry.name == item.name) {
            hero_emoji = outfit.emoji.to_string();
        } else if let Some(boost) = TAP_BOOST_CATALOG
            .iter()
            .find(|entry| entry.name == item.name)
        {
            hero_emoji = boost.emoji.to_string();
        } else if let Some(bonus) = PETTING_BONUS_CATALOG
            .iter()
            .find(|entry| entry.name == item.name)
        {
            hero_emoji = bonus.emoji.to_string();
        }
    }

    format!(
        r#"<div class="onboarding-backdrop need-paw-points-backdrop" id="need-paw-points-modal" role="dialog" aria-modal="true" aria-labelledby="need-paw-points-title" hidden data-balance="{balance}"{auto_open_attrs} data-item-name="{item_name}" data-item-price="{item_price}" data-item-emoji="{hero_emoji}" data-points-needed="{points_needed}">
  <div class="onboarding-modal need-paw-points-modal">
    <button type="button" class="need-paw-points-close" id="need-paw-points-close" aria-label="Close paw points popup">&times;</button>
    <div class="need-paw-points-hero" aria-hidden="true"><span class="need-paw-points-hero-emoji" id="need-paw-points-hero-emoji">{hero_emoji}</span></div>
    <h2 id="need-paw-points-title">Almost there!</h2>
    <p class="need-paw-points-lead" id="need-paw-points-lead">You need a few more paw points for <strong id="need-paw-points-item-name">{item_name_display}</strong>.</p>
    <p class="need-paw-points-balance">
      Your balance: <strong id="need-paw-points-balance">{balance}</strong> {paw_icon} ·
      You need <strong id="need-paw-points-shortfall">{points_needed_display}</strong> more.
    </p>
    <p class="need-paw-points-price-line">This item costs <strong id="need-paw-points-item-price">{item_price_display}</strong> {paw_icon}.</p>
    <section class="need-paw-points-purchase" aria-labelledby="need-paw-points-buy-title">
      <h3 id="need-paw-points-buy-title">Purchase paw points</h3>
      {buy_points}
    </section>
    <section class="need-paw-points-earn" aria-labelledby="need-paw-points-earn-title">
      <h3 id="need-paw-points-earn-title">Or earn paw points</h3>
      <ul class="need-paw-points-earn-list">
        <li>Complete care tasks on the <a href="/home?tab=tasks">Tasks</a> tab.</li>
        <li>Pet your cat here for bonus points per tap.</li>
      </ul>
    </section>
    <p class="need-paw-points-actions">
      <button type="button" class="onboarding-skip-btn need-paw-points-dismiss" id="need-paw-points-dismiss">Maybe later</button>
    </p>
  </div>
</div>"#,
        balance = balance,
        auto_open_attrs = auto_open_attrs,
        item_name = item_name,
        item_price = item_price,
        hero_emoji = escape_html(&hero_emoji),
        points_needed = points_needed,
        item_name_display = auto_open_item
            .map(|item| escape_html(item.name))
            .unwrap_or_default(),
        points_needed_display = auto_open_item
            .map(|item| item.price.saturating_sub(balance).to_string())
            .unwrap_or_default(),
        item_price_display = auto_open_item
            .map(|item| item.price.to_string())
            .unwrap_or_default(),
        paw_icon = paw_points_icon_html(),
        buy_points = stripe_payments::render_buy_points_section(),
    )
}

fn shop_item_from_query(query: &NeedPawPointsQuery) -> Option<ShopItemQuote> {
    if let Some(id) = query.decor_id.as_deref() {
        let decor = decor_by_id(id.trim())?;
        if decor.price == 0 {
            return None;
        }
        return Some(ShopItemQuote {
            name: decor.name,
            price: decor.price,
        });
    }

    if let Some(id) = query.outfit_id.as_deref() {
        let outfit = outfit_by_id(id.trim())?;
        return Some(ShopItemQuote {
            name: outfit.name,
            price: outfit.price,
        });
    }

    if let Some(id) = query.boost_id.as_deref() {
        let boost = tap_boost_by_id(id.trim())?;
        if boost.price == 0 {
            return None;
        }
        return Some(ShopItemQuote {
            name: boost.name,
            price: boost.price,
        });
    }

    if let Some(id) = query.petting_bonus_id.as_deref() {
        let bonus = petting_bonus_by_id(id.trim())?;
        return Some(ShopItemQuote {
            name: bonus.name,
            price: bonus.price,
        });
    }

    None
}

fn tap_boost_redirect(status: &str) -> Redirect {
    Redirect::to(&format!("/home/cat-home?status={status}"))
}

fn tap_boost_return_hidden_field(return_to: Option<&str>) -> &'static str {
    if return_to == Some("cat_home") {
        r#"<input type="hidden" name="return_to" value="cat_home" />"#
    } else {
        ""
    }
}

fn outfit_redirect(_return_to: &str, status: &str) -> Redirect {
    Redirect::to(&format!("/home/cat-home?status={status}"))
}

fn outfit_return_hidden_field(return_to: Option<&str>) -> &'static str {
    if return_to == Some("cat_home") {
        r#"<input type="hidden" name="return_to" value="cat_home" />"#
    } else {
        ""
    }
}

fn render_cat_home_outfit_shop(profile: &UserProfile) -> String {
    let pet_name = escape_html(&display_pet_name(profile));
    let cards = render_outfit_cards_inner(profile, true, Some("cat_home"));
    format!(
        r#"<section class="cat-home-outfit-shop" aria-label="Outfit shop">
  <h2>Dress up {pet_name}</h2>
  <p class="field-hint">Swipe to browse outfits and spend paw points without leaving the play area.</p>
  <div class="cat-home-outfit-slider" tabindex="0">
    {cards}
  </div>
</section>"#,
        pet_name = pet_name,
        cards = cards,
    )
}

fn render_cat_home_tap_boost_shop(profile: &UserProfile) -> String {
    let cards = render_tap_boost_cards_inner(profile, true, Some("cat_home"));
    let active_reward = equipped_tap_boost_reward(profile);
    let (effective_reward, _, petting_multiplier) = effective_tap_reward(profile);
    let reward_hint = if let Some(multiplier) = petting_multiplier {
        format!(
            "currently <strong>{}</strong> per tap while your petting bonus is active (<strong>{}</strong> base boost × {multiplier})",
            render_tap_reward_label(effective_reward),
            render_tap_reward_label(active_reward),
            multiplier = multiplier,
        )
    } else {
        format!(
            "currently <strong>{}</strong> per tap",
            render_tap_reward_label(active_reward),
        )
    };

    format!(
        r#"<section class="cat-home-tap-boost-shop" aria-label="Pet tap boosts">
  <h2>Level up petting</h2>
  <p class="field-hint">Each tap on your cat earns paw points. Boosts raise how much you get per pet — {reward_hint}.</p>
  <div class="cat-home-tap-boost-slider" tabindex="0">
    {cards}
  </div>
</section>"#,
        reward_hint = reward_hint,
        cards = cards,
    )
}

fn petting_bonus_return_hidden_field(return_to: Option<&str>) -> &'static str {
    if return_to == Some("cat_home") {
        r#"<input type="hidden" name="return_to" value="cat_home" />"#
    } else {
        ""
    }
}

fn petting_bonus_redirect(status: &str) -> Redirect {
    Redirect::to(&format!("/home/cat-home?status={status}"))
}

fn render_petting_bonus_active_banner(profile: &UserProfile) -> String {
    let multiplier = active_petting_bonus_multiplier(profile);
    let Some(multiplier) = multiplier else {
        return String::new();
    };
    let bonus_id = profile.active_petting_bonus.trim();
    let Some(bonus) = petting_bonus_by_id(bonus_id) else {
        return String::new();
    };
    let seconds_left = profile
        .petting_bonus_expires_at
        .saturating_sub(timestamp_now());

    format!(
        r#"<p class="petting-bonus-active" role="status" data-expires-at="{expires_at}" data-multiplier="{multiplier}">
  <span class="petting-bonus-active-emoji" aria-hidden="true">{emoji}</span>
  <span class="petting-bonus-active-copy"><strong>{name}</strong> · {multiplier_label} · <span class="petting-bonus-countdown">{seconds_left}s</span> left</span>
</p>"#,
        expires_at = profile.petting_bonus_expires_at,
        multiplier = multiplier,
        emoji = bonus.emoji,
        name = escape_html(bonus.name),
        multiplier_label = render_petting_bonus_stacked_label(profile, multiplier),
        seconds_left = seconds_left,
    )
}

fn render_cat_home_petting_bonus_shop(profile: &UserProfile) -> String {
    let cards = render_petting_bonus_cards_inner(profile, true, Some("cat_home"));
    let active_boost = render_tap_reward_label(equipped_tap_boost_reward(profile));
    format!(
        r#"<section class="cat-home-petting-bonus-shop" aria-label="Timed petting bonuses">
  <h2>Petting bonuses</h2>
  <p class="field-hint">Timed multipliers stack on your active tap boost ({active_boost}). A 10× bonus on +5 pays +50 per tap.</p>
  <div class="cat-home-petting-bonus-slider" tabindex="0">
    {cards}
  </div>
</section>"#,
        active_boost = active_boost,
        cards = cards,
    )
}

fn render_petting_bonus_cards_inner(
    profile: &UserProfile,
    slider_card: bool,
    return_to: Option<&str>,
) -> String {
    let return_field = petting_bonus_return_hidden_field(return_to);
    let card_class = if slider_card {
        "petting-bonus-card petting-bonus-card-slider"
    } else {
        "petting-bonus-card"
    };
    let now = timestamp_now();

    PETTING_BONUS_CATALOG
        .iter()
        .map(|bonus| {
            let owned = petting_bonus_inventory_count(profile, bonus.id);
            let active = profile.active_petting_bonus == bonus.id
                && profile.petting_bonus_expires_at > now;
            let mut classes = vec![card_class];
            if owned > 0 {
                classes.push("owned");
            }
            if active {
                classes.push("active");
            }

            let effect_label = format!(
                "{} for {}",
                render_petting_bonus_stacked_label(profile, bonus.multiplier),
                render_petting_bonus_duration_label(bonus.duration_secs),
            );

            let data_attrs = if active {
                format!(
                    r#" data-bonus-id="{}" data-expires-at="{}""#,
                    escape_html_attr(bonus.id),
                    profile.petting_bonus_expires_at,
                )
            } else {
                String::new()
            };

            let (action, purchase_attrs) = if active {
                let seconds_left = profile.petting_bonus_expires_at.saturating_sub(now);
                (
                    format!(
                        r#"<span class="petting-bonus-badge">Active · {seconds_left}s left</span>"#,
                        seconds_left = seconds_left,
                    ),
                    String::new(),
                )
            } else if owned > 0 {
                (
                    format!(
                        r#"<form action="/home/petting-bonuses/activate" method="post"><input type="hidden" name="bonus_id" value="{}" />{return_field}<button type="submit" class="download-btn petting-bonus-btn">Activate ({owned} ready)</button></form>"#,
                        escape_html_attr(bonus.id),
                        return_field = return_field,
                        owned = owned,
                    ),
                    String::new(),
                )
            } else {
                let purchase_attrs = shop_purchase_data_attrs(
                    "bonus",
                    bonus.id,
                    bonus.price,
                    bonus.name,
                    bonus.emoji,
                    return_to,
                );
                let action = if profile.paw_points < bonus.price {
                    render_shop_points_shortfall_trigger(bonus.name, bonus.price, bonus.emoji)
                } else {
                    format!(
                        r#"<form action="/home/petting-bonuses/buy" method="post"><input type="hidden" name="bonus_id" value="{}" />{return_field}<button type="submit" class="download-btn petting-bonus-btn">Buy for {} pts</button></form>"#,
                        escape_html_attr(bonus.id),
                        bonus.price,
                        return_field = return_field,
                    )
                };
                (action, purchase_attrs)
            };

            format!(
                r#"<article class="{}{}{}"><div class="petting-bonus-emoji">{}</div><h3>{}</h3><p class="petting-bonus-effect">{}</p><p class="petting-bonus-price">{}</p><div class="petting-bonus-actions">{}</div></article>"#,
                classes.join(" "),
                purchase_attrs,
                data_attrs,
                bonus.emoji,
                escape_html(bonus.name),
                effect_label,
                paw_points_amount_html(bonus.price),
                action,
            )
        })
        .collect()
}

fn render_tap_boost_cards_inner(
    profile: &UserProfile,
    slider_card: bool,
    return_to: Option<&str>,
) -> String {
    let return_field = tap_boost_return_hidden_field(return_to);
    let card_class = if slider_card {
        "tap-boost-card tap-boost-card-slider"
    } else {
        "tap-boost-card"
    };

    TAP_BOOST_CATALOG
        .iter()
        .map(|boost| {
            let owned = profile.owned_tap_boosts.iter().any(|id| id == boost.id);
            let equipped = profile.equipped_tap_boost == boost.id;
            let mut classes = vec![card_class];
            if owned {
                classes.push("owned");
            }
            if equipped {
                classes.push("equipped");
            }

            let price_label = if boost.price == 0 {
                "Included".to_string()
            } else {
                paw_points_amount_html(boost.price)
            };

            let (action, purchase_attrs) = if equipped {
                (
                    r#"<span class="tap-boost-badge">Active boost</span>"#.to_string(),
                    String::new(),
                )
            } else if owned {
                (
                    format!(
                        r#"<form action="/home/tap-boosts/equip" method="post"><input type="hidden" name="boost_id" value="{}" />{return_field}<button type="submit" class="download-btn tap-boost-btn">Activate</button></form>"#,
                        escape_html_attr(boost.id),
                        return_field = return_field,
                    ),
                    String::new(),
                )
            } else if boost.price == 0 {
                (
                    r#"<span class="tap-boost-badge">Starter boost</span>"#.to_string(),
                    String::new(),
                )
            } else {
                let purchase_attrs = shop_purchase_data_attrs(
                    "boost",
                    boost.id,
                    boost.price,
                    boost.name,
                    boost.emoji,
                    return_to,
                );
                let action = if profile.paw_points < boost.price {
                    render_shop_points_shortfall_trigger(boost.name, boost.price, boost.emoji)
                } else {
                    format!(
                        r#"<form action="/home/tap-boosts/buy" method="post"><input type="hidden" name="boost_id" value="{}" />{return_field}<button type="submit" class="download-btn tap-boost-btn">Buy for {} pts</button></form>"#,
                        escape_html_attr(boost.id),
                        boost.price,
                        return_field = return_field,
                    )
                };
                (action, purchase_attrs)
            };

            format!(
                r#"<article class="{}{}"><div class="tap-boost-emoji">{}</div><h3>{}</h3><p class="tap-boost-reward">{}</p><p class="tap-boost-price">{}</p><div class="tap-boost-actions">{}</div></article>"#,
                classes.join(" "),
                purchase_attrs,
                boost.emoji,
                escape_html(boost.name),
                render_tap_reward_label(boost.tap_reward),
                price_label,
                action
            )
        })
        .collect()
}

fn render_outfit_cards_inner(
    profile: &UserProfile,
    slider_card: bool,
    return_to: Option<&str>,
) -> String {
    let return_field = outfit_return_hidden_field(return_to);
    let card_class = if slider_card {
        "outfit-card outfit-card-slider"
    } else {
        "outfit-card"
    };

    OUTFIT_CATALOG
        .iter()
        .map(|outfit| {
            let owned = profile.owned_outfits.iter().any(|id| id == outfit.id);
            let equipped = profile.equipped_outfit == outfit.name;
            let mut classes = vec![card_class];
            if owned {
                classes.push("owned");
            }
            if equipped {
                classes.push("equipped");
            }

            let (action, purchase_attrs) = if equipped {
                (
                    r#"<span class="outfit-badge">Currently equipped</span>"#.to_string(),
                    String::new(),
                )
            } else if owned {
                (
                    format!(
                        r#"<form action="/home/outfits/equip" method="post"><input type="hidden" name="outfit_id" value="{}" />{return_field}<button type="submit" class="download-btn outfit-btn">Equip</button></form>"#,
                        escape_html_attr(outfit.id),
                        return_field = return_field,
                    ),
                    String::new(),
                )
            } else {
                let purchase_attrs = shop_purchase_data_attrs(
                    "outfit",
                    outfit.id,
                    outfit.price,
                    outfit.name,
                    outfit.emoji,
                    return_to,
                );
                let action = if profile.paw_points < outfit.price {
                    render_shop_points_shortfall_trigger(outfit.name, outfit.price, outfit.emoji)
                } else {
                    format!(
                        r#"<form action="/home/outfits/buy" method="post"><input type="hidden" name="outfit_id" value="{}" />{return_field}<button type="submit" class="download-btn outfit-btn">Buy for {} pts</button></form>"#,
                        escape_html_attr(outfit.id),
                        outfit.price,
                        return_field = return_field,
                    )
                };
                (action, purchase_attrs)
            };

            format!(
                r#"<article class="{}{}"><div class="outfit-emoji">{}</div><h3>{}</h3><p class="outfit-price">{}</p><div class="outfit-actions">{}</div></article>"#,
                classes.join(" "),
                purchase_attrs,
                outfit.emoji,
                escape_html(outfit.name),
                paw_points_amount_html(outfit.price),
                action
            )
        })
        .collect()
}

fn render_decor_cards(profile: &UserProfile) -> String {
    DECOR_CATALOG
        .iter()
        .map(|decor| {
            let owned = profile.owned_decor.iter().any(|id| id == decor.id);
            let equipped = profile
                .equipped_decor
                .get(decor.slot)
                .is_some_and(|id| id == decor.id);
            let mut classes = vec!["decor-card"];
            if owned {
                classes.push("owned");
            }
            if equipped {
                classes.push("equipped");
            }

            let price_label = if decor.price == 0 {
                "Included".to_string()
            } else {
                paw_points_amount_html(decor.price)
            };

            let (action, purchase_attrs) = if equipped {
                (
                    r#"<span class="decor-badge">Placed in home</span>"#.to_string(),
                    String::new(),
                )
            } else if owned {
                (
                    format!(
                        r#"<form action="/home/decor/equip" method="post"><input type="hidden" name="decor_id" value="{}" /><button type="submit" class="download-btn decor-btn">Place in home</button></form>"#,
                        escape_html_attr(decor.id)
                    ),
                    String::new(),
                )
            } else if decor.price == 0 {
                (
                    r#"<span class="decor-badge">Starter decor</span>"#.to_string(),
                    String::new(),
                )
            } else {
                let purchase_attrs = shop_purchase_data_attrs(
                    "decor",
                    decor.id,
                    decor.price,
                    decor.name,
                    decor.emoji,
                    None,
                );
                let action = if profile.paw_points < decor.price {
                    render_shop_points_shortfall_trigger(decor.name, decor.price, decor.emoji)
                } else {
                    format!(
                        r#"<form action="/home/decor/buy" method="post"><input type="hidden" name="decor_id" value="{}" /><button type="submit" class="download-btn decor-btn">Buy for {} pts</button></form>"#,
                        escape_html_attr(decor.id),
                        decor.price
                    )
                };
                (action, purchase_attrs)
            };

            format!(
                r#"<article class="{}{}"><div class="decor-emoji">{}</div><p class="decor-slot">{}</p><h3>{}</h3><p class="decor-price">{}</p><div class="decor-actions">{}</div></article>"#,
                classes.join(" "),
                purchase_attrs,
                decor.emoji,
                decor_slot_label(decor.slot),
                escape_html(decor.name),
                price_label,
                action,
            )
        })
        .collect()
}

fn render_cat_home_scene(profile: &UserProfile) -> String {
    let pet_name = if profile.pet_name.trim().is_empty() {
        "Your cat".to_string()
    } else {
        profile.pet_name.clone()
    };
    let room = equipped_decor_for_slot(profile, "room")
        .map(|decor| decor.id)
        .unwrap_or("sunny_nook");
    let rug = equipped_decor_for_slot(profile, "rug");
    let bed = equipped_decor_for_slot(profile, "bed");
    let toy = equipped_decor_for_slot(profile, "toy");
    let plant = equipped_decor_for_slot(profile, "plant");

    let rug_layer = rug.map(|decor| {
        format!(
            r#"<div class="cat-home-prop cat-home-rug" title="{}"><span class="cat-home-prop-emoji" aria-hidden="true">{}</span><span class="cat-home-prop-label">{}</span></div>"#,
            escape_html(decor.name),
            decor.emoji,
            escape_html(decor.name),
        )
    }).unwrap_or_default();

    let bed_layer = bed.map(|decor| {
        format!(
            r#"<div class="cat-home-prop cat-home-bed" title="{}"><span class="cat-home-prop-emoji" aria-hidden="true">{}</span><span class="cat-home-prop-label">{}</span></div>"#,
            escape_html(decor.name),
            decor.emoji,
            escape_html(decor.name),
        )
    }).unwrap_or_default();

    let toy_layer = toy.map(|decor| {
        format!(
            r#"<div class="cat-home-prop cat-home-toy" title="{}"><span class="cat-home-prop-emoji" aria-hidden="true">{}</span><span class="cat-home-prop-label">{}</span></div>"#,
            escape_html(decor.name),
            decor.emoji,
            escape_html(decor.name),
        )
    }).unwrap_or_default();

    let plant_layer = plant.map(|decor| {
        format!(
            r#"<div class="cat-home-prop cat-home-plant" title="{}"><span class="cat-home-prop-emoji" aria-hidden="true">{}</span><span class="cat-home-prop-label">{}</span></div>"#,
            escape_html(decor.name),
            decor.emoji,
            escape_html(decor.name),
        )
    }).unwrap_or_default();

    let petting_bonus_banner = render_petting_bonus_active_banner(profile);

    format!(
        r#"<div class="cat-home-scene" data-room="{room}">
  <div class="cat-home-room-bg" aria-hidden="true"></div>
  {petting_bonus_banner}
  {rug_layer}
  {bed_layer}
  <div class="cat-home-pet-stage">
    <div class="cat-home-pet-stack">
      <p class="cat-home-pet-bubble" role="note">pet me!</p>
      {pet_avatar}
    </div>
  </div>
  {toy_layer}
  {plant_layer}
  <p class="cat-home-mood">{pet_name} is relaxing in their play area.</p>
</div>"#,
        room = escape_html_attr(room),
        petting_bonus_banner = petting_bonus_banner,
        rug_layer = rug_layer,
        bed_layer = bed_layer,
        toy_layer = toy_layer,
        plant_layer = plant_layer,
        pet_avatar = render_pet_avatar(profile),
        pet_name = escape_html(&pet_name),
    )
}

fn cat_home_status_block(status: Option<&str>) -> String {
    match status {
        Some("decor_bought") => {
            r#"<p class="auth-success status-flash" role="status">Yay! Decor purchased and placed in your cat's home! 🏡</p>"#
        }
        Some("decor_equipped") => {
            r#"<p class="auth-success status-flash" role="status">Decor placed in your cat's home! 🏡</p>"#
        }
        Some("decor_owned") => {
            r#"<p class="auth-error status-flash" role="alert">You already own that decor item.</p>"#
        }
        Some("decor_points") | Some("need_paw_points") => "",
        Some("decor_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That decor item is not available.</p>"#
        }
        Some("outfit_bought") => {
            r#"<p class="auth-success status-flash" role="status">Yay! Outfit purchased and equipped for your cat! 👗</p>"#
        }
        Some("outfit_equipped") => {
            r#"<p class="auth-success status-flash" role="status">Outfit equipped for your cat! 👗</p>"#
        }
        Some("outfit_owned") => {
            r#"<p class="auth-error status-flash" role="alert">You already own that outfit.</p>"#
        }
        Some("outfit_points") => "",
        Some("outfit_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That outfit is not available.</p>"#
        }
        Some("boost_bought") => {
            r#"<p class="auth-success status-flash" role="status">Yay! Tap boost purchased and activated! Pet your cat for bigger rewards! 🐾</p>"#
        }
        Some("boost_equipped") => {
            r#"<p class="auth-success status-flash" role="status">Tap boost activated! Pet your cat for bigger paw points! 🐾</p>"#
        }
        Some("boost_owned") => {
            r#"<p class="auth-error status-flash" role="alert">You already own that tap boost.</p>"#
        }
        Some("boost_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That tap boost is not available.</p>"#
        }
        Some("petting_bonus_bought") => {
            r#"<p class="auth-success status-flash" role="status">Yay! Petting bonus purchased! Activate it when you're ready for a points rush! ⚡</p>"#
        }
        Some("petting_bonus_activated") => {
            r#"<p class="auth-success status-flash" role="status">Petting bonus activated! Pet your cat fast while the multiplier lasts! 💥</p>"#
        }
        Some("petting_bonus_active") => {
            r#"<p class="auth-error status-flash" role="alert">A petting bonus is already running. Wait for it to finish before activating another.</p>"#
        }
        Some("petting_bonus_empty") => {
            r#"<p class="auth-error status-flash" role="alert">You don't have that petting bonus in your inventory. Buy one first.</p>"#
        }
        Some("petting_bonus_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That petting bonus is not available.</p>"#
        }
        _ => "",
    }
    .to_string()
}

fn render_task_due_html(task: &UserTask) -> String {
    if !task_has_adjustable_time(&task.id) {
        return format!(
            r#"{due} · +{reward} pts"#,
            due = escape_html(&task.due_label),
            reward = task.reward,
        );
    }

    format!(
        r#"<span class="task-schedule-prefix">{prefix}</span> · <button type="button" class="task-time-btn" data-task-id="{id}" data-pet-id="{pet_id}" data-time="{time_value}" data-time-minutes="{time_minutes}" data-task-title="{title}" aria-label="Change time for {title}">{time_label}</button> · +{reward} pts"#,
        prefix = task_schedule_prefix(&task.id),
        id = escape_html_attr(&task.id),
        pet_id = escape_html_attr(&task.pet_id),
        time_value = minutes_to_time_input_value(task.time_minutes),
        time_minutes = task.time_minutes,
        title = escape_html_attr(&task.title),
        time_label = escape_html(&format_time_from_minutes(task.time_minutes)),
        reward = task.reward,
    )
}

fn render_task_list(profile: &UserProfile) -> String {
    if list_pet_summaries(profile).is_empty() {
        return String::new();
    }

    let active_id = active_pet_id(profile).to_string();
    let mut tasks: Vec<UserTask> = profile
        .tasks
        .iter()
        .filter(|task| task.pet_id == active_id)
        .cloned()
        .collect();
    sort_tasks_by_time(&mut tasks);

    tasks
        .iter()
        .map(|task| {
            let completed_class = if task.completed { " completed" } else { "" };
            let button_label = if task.completed {
                "Mark incomplete"
            } else {
                "Complete"
            };
            let delete_form = if is_custom_task(&task.id) {
                format!(
                    r#"<form action="/home/tasks/delete" method="post" class="task-delete-form"><input type="hidden" name="task_id" value="{id}" /><input type="hidden" name="pet_id" value="{pet_id}" />{pet_owner_field}<button type="submit" class="task-delete-btn" aria-label="Delete {title}">Delete</button></form>"#,
                    id = escape_html_attr(&task.id),
                    pet_id = escape_html_attr(&task.pet_id),
                    pet_owner_field = task_pet_owner_hidden_field(profile),
                    title = escape_html_attr(&task.title),
                )
            } else {
                String::new()
            };
            let pet_owner_field = task_pet_owner_hidden_field(profile);
            format!(
                r#"<li class="task-item{completed_class}"><div><p class="task-title">{title}</p><p class="task-due">{due_html}</p></div><div class="task-item-actions"><form action="/home/tasks/toggle" method="post"><input type="hidden" name="task_id" value="{id}" /><input type="hidden" name="pet_id" value="{pet_id}" />{pet_owner_field}<button type="submit" class="download-btn task-toggle-btn">{button_label}</button></form>{delete_form}</div></li>"#,
                completed_class = completed_class,
                title = escape_html(&task.title),
                due_html = render_task_due_html(task),
                id = escape_html_attr(&task.id),
                pet_id = escape_html_attr(&task.pet_id),
                pet_owner_field = pet_owner_field,
                button_label = button_label,
                delete_form = delete_form,
            )
        })
        .collect()
}

fn render_task_add_section(profile: &UserProfile) -> String {
    if list_pet_summaries(profile).is_empty() {
        return String::new();
    }

    let pet_id = escape_html_attr(active_pet_id(profile));
    let pet_name = active_pet_snapshot(profile)
        .map(|pet| escape_html(&pet.pet_name))
        .unwrap_or_else(|| "your cat".to_string());

    let pet_owner_field = task_pet_owner_hidden_field(profile);
    let shared_hint = if sharing::is_viewing_shared_pet(profile) {
        " · shared cat"
    } else {
        ""
    };
    format!(
        r#"<div class="task-add-row">
  <p class="field-hint task-pet-label">Tasks for <strong>{pet_name}</strong>{shared_hint} · switch cats on the My Pet tab</p>
  <form action="/home/tasks/add" method="post" class="task-add-form">
    <input type="hidden" name="pet_id" value="{pet_id}" />
    {pet_owner_field}
    <input type="text" name="task_title" class="task-add-input" placeholder="Add a task…" maxlength="60" required aria-label="New task name" />
    <button type="submit" class="download-btn task-add-btn">Add</button>
  </form>
  <p class="field-hint task-add-hint">Custom tasks earn 10 paw points. Tap a time to reschedule.</p>
</div>"#,
        pet_name = pet_name,
        pet_id = pet_id,
        pet_owner_field = pet_owner_field,
        shared_hint = shared_hint,
    )
}

fn render_calendar_grid(
    state: &AppState,
    viewer: &UserProfile,
    calendar_profile: &UserProfile,
    month: u32,
    year: u32,
) -> String {
    let weekday_labels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut html = String::new();

    for label in weekday_labels {
        html.push_str(&format!(r#"<span class="calendar-head">{label}</span>"#));
    }

    let first_of_month = NaiveDate::from_ymd_opt(year as i32, month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid fallback date"));
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

    let event_days: HashSet<u32> =
        sharing::visible_calendar_events_for_viewer(state, viewer, today)
            .iter()
            .filter(|e| e.month == month && e.year == year)
            .map(|e| e.day)
            .collect();

    let task_days: HashSet<u32> = calendar_profile
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
        let month_name = MONTH_NAMES
            .get(month.saturating_sub(1) as usize)
            .unwrap_or(&"???");
        html.push_str(&format!(
            r#"<button type="button" class="{}" data-day="{day}" data-month="{month}" data-year="{year}" aria-label="{month_name} {day}, {year}" aria-pressed="false">{day}</button>"#,
            classes.join(" ")
        ));
    }

    html
}

fn render_event_list(state: &AppState, viewer: &UserProfile) -> String {
    let events =
        sharing::visible_calendar_events_for_viewer(state, viewer, Local::now().date_naive());
    if events.is_empty() {
        return "<li>No upcoming events yet.</li>".to_string();
    }

    events
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

fn render_calendar_data_json(
    state: &AppState,
    viewer: &UserProfile,
    calendar_profile: &UserProfile,
    month: u32,
    year: u32,
) -> String {
    let today = Local::now().date_naive();
    let events: Vec<_> = sharing::visible_calendar_events_for_viewer(state, viewer, today)
        .iter()
        .map(|event| {
            serde_json::json!({
                "id": event.id,
                "day": event.day,
                "month": event.month,
                "year": event.year,
                "title": event.title,
                "time_label": event.time_label,
                "time_minutes": event.time_minutes,
                "user_created": event.id.is_some(),
            })
        })
        .collect();

    let mut calendar_tasks: Vec<_> = calendar_profile
        .tasks
        .iter()
        .filter_map(|task| {
            task_schedule_date(task).map(|date| {
                (
                    task.time_minutes,
                    task.title.clone(),
                    task.id.clone(),
                    date,
                    task.due_label.clone(),
                    task.reward,
                    task.completed,
                )
            })
        })
        .collect();
    calendar_tasks.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let tasks: Vec<_> = calendar_tasks
        .into_iter()
        .map(
            |(time_minutes, title, id, date, due_label, reward, completed)| {
                serde_json::json!({
                    "day": date.day(),
                    "month": date.month(),
                    "year": date.year(),
                    "id": id,
                    "title": title,
                    "due_label": due_label,
                    "reward": reward,
                    "completed": completed,
                    "time_minutes": time_minutes,
                    "time_value": minutes_to_time_input_value(time_minutes),
                    "adjustable_time": task_has_adjustable_time(&id),
                })
            },
        )
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
        .map(|user| format_member_since(user.created_at))
        .unwrap_or_else(|| "Recently joined".to_string())
}

fn breed_guide_status_block(status: Option<&str>) -> String {
    match status {
        Some("guide_bought") => {
            r#"<p class="auth-success status-flash" role="status">Yay! Your premium breed care guide is unlocked! 🐾</p>"#
        }
        Some("guide_cancelled") => {
            r#"<p class="auth-error status-flash" role="alert">Checkout was cancelled. Your guide is still available to unlock anytime.</p>"#
        }
        _ => "",
    }
    .to_string()
}

async fn premium_checkout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    if !stripe_payments::stripe_checkout_enabled() {
        return Redirect::to("/home?tab=account&status=payments_unconfigured");
    }

    let profile = get_or_create_profile(&state, &email).await;
    if user_has_premium(&profile) {
        return Redirect::to("/home?tab=account&status=premium_owned");
    }

    match stripe_payments::create_premium_checkout_session(&state, &email).await {
        Ok(url) => Redirect::temporary(&url),
        Err(CheckoutError::NotConfigured) => {
            Redirect::to("/home?tab=account&status=payments_unconfigured")
        }
        Err(_) => Redirect::to("/home?tab=account&status=premium_checkout_failed"),
    }
}

async fn add_pet_submit(
    State(_state): State<AppState>,
    jar: CookieJar,
    Form(_form): Form<AddPetForm>,
) -> impl IntoResponse {
    let _ = jar;
    Redirect::to("/home?tab=pet&add_cat=1")
}

async fn breed_guide_checkout(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<BreedGuideCheckoutForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    if !stripe_payments::stripe_checkout_enabled() {
        return Redirect::to("/home?tab=health&status=payments_unconfigured");
    }

    let Some(guide) = breed_guides::guide_for_slug(form.breed_slug.trim()) else {
        return Redirect::to("/home?tab=health&status=guide_invalid");
    };

    let profile = get_or_create_profile(&state, &email).await;
    if !entitlements::can_access_health_records(profile.premium_unlocked, &profile.email) {
        return Redirect::to("/home?tab=health&status=premium_required");
    }
    if breed_guides::can_access_breed_guide(
        profile.premium_unlocked,
        &profile.email,
        &profile.owned_breed_guides,
        &guide.slug,
    ) {
        return Redirect::to(&format!("/home/breed-guide/{}", guide.slug));
    }

    match stripe_payments::create_breed_guide_checkout_session(
        &state,
        &email,
        &guide.slug,
        &guide.breed_name,
    )
    .await
    {
        Ok(url) => Redirect::temporary(&url),
        Err(CheckoutError::NotConfigured) => {
            Redirect::to("/home?tab=health&status=payments_unconfigured")
        }
        Err(_) => Redirect::to(&format!(
            "/home/breed-guide/{}?status=guide_cancelled",
            guide.slug
        )),
    }
}

async fn public_breeds_index_page() -> impl IntoResponse {
    Html(breed_seo::render_public_breeds_index(&public_base_url()))
}

async fn public_breed_guide_page(Path(slug): Path<String>) -> impl IntoResponse {
    let Some(guide) = breed_guides::guide_for_slug(&slug) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    Html(breed_seo::render_public_breed_page(
        &guide,
        &public_base_url(),
    ))
    .into_response()
}

async fn sitemap_page() -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/xml; charset=utf-8",
        )],
        breed_seo::render_sitemap_xml(&public_base_url()),
    )
}

async fn robots_page() -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        breed_seo::render_robots_txt(&public_base_url()),
    )
}

async fn breed_guide_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
    Query(query): Query<BreedGuideQuery>,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let Some(guide) = breed_guides::guide_for_slug(&slug) else {
        return Redirect::to("/home?tab=health&status=guide_invalid").into_response();
    };

    if let Some(session_id) = query.session_id.as_deref() {
        if !session_id.is_empty() {
            let _ = stripe_payments::fulfill_checkout_session(&state, session_id).await;
        }
    }

    let profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet").into_response();
    }

    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let owned = breed_guides::can_access_breed_guide(
        profile.premium_unlocked,
        &profile.email,
        &profile.owned_breed_guides,
        &guide.slug,
    );
    let content = breed_guides::render_guide_page_html(
        &profile.pet_name,
        &guide,
        owned,
        stripe_payments::stripe_checkout_enabled(),
    );

    match fs::read_to_string("templates/breed-guide.html").await {
        Ok(template) => {
            let html = template
                .replace(
                    "{{PAGE_TITLE}}",
                    &format!("WhiskerWatch — {} Care Guide", guide.breed_name),
                )
                .replace("{{USER_NAME}}", &escape_html(&username))
                .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar))
                .replace(
                    "{{STATUS_BLOCK}}",
                    &breed_guide_status_block(query.status.as_deref()),
                )
                .replace("{{BREED_GUIDE_CONTENT}}", &content);
            (jar, Html(html)).into_response()
        }
        Err(_) => (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load breed guide page",
            ),
        )
            .into_response(),
    }
}

async fn breed_guides_shop_page(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let profile = get_or_create_profile(&state, &email).await;
    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let content = breed_guides::render_breed_guides_shop(
        &profile.owned_breed_guides,
        &profile.pet_breed,
        profile.premium_unlocked,
        &profile.email,
        stripe_payments::stripe_checkout_enabled(),
    );

    match fs::read_to_string("templates/breed-guide.html").await {
        Ok(template) => {
            let html = template
                .replace("{{PAGE_TITLE}}", "WhiskerWatch — Premium Breed Guides")
                .replace("{{USER_NAME}}", &escape_html(&username))
                .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar))
                .replace("{{STATUS_BLOCK}}", "")
                .replace("{{BREED_GUIDE_CONTENT}}", &content);
            (jar, Html(html)).into_response()
        }
        Err(_) => (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load breed guides shop",
            ),
        )
            .into_response(),
    }
}

async fn breed_select_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<BreedSelectQuery>,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let adding_cat = query.add_cat.as_deref() == Some("1");
    let return_params = if adding_cat { "add_cat=1" } else { "setup=pet" };
    let back_url = if adding_cat {
        "/home?add_cat=1"
    } else {
        "/home?setup=pet"
    };
    let intro = if adding_cat {
        "Browse breeds by type. Tap one to return to add cat with your choice filled in."
    } else {
        "Browse breeds by type. Tap one to return to pet setup with your choice filled in."
    };
    let (loading_title, loading_copy) = if adding_cat {
        (
            "Gathering breeds for your new kitty…",
            "We're fluffing up the breed guide with every whisker type — short-haired, long-haired, colorpoint, and more. Just a moment while we get everything purr-ready.",
        )
    } else {
        (
            "Gathering breeds for your cat…",
            "We're fluffing up the breed guide with every whisker type — short-haired, long-haired, colorpoint, and more. Just a moment while we get everything purr-ready.",
        )
    };

    match fs::read_to_string("templates/breed-select.html").await {
        Ok(template) => {
            let html = template
                .replace("{{USER_NAME}}", &escape_html(&username))
                .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar))
                .replace("{{BREED_LOADING_TITLE}}", loading_title)
                .replace("{{BREED_LOADING_COPY}}", loading_copy)
                .replace("{{BREED_INTRO}}", intro)
                .replace("{{BREED_BACK_URL}}", back_url)
                .replace(
                    "{{BREED_CATALOG}}",
                    &breeds::render_catalog_html(return_params),
                );
            (jar, Html(html)).into_response()
        }
        Err(_) => (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load breed page",
            ),
        )
            .into_response(),
    }
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

    let checkout_fulfill_result = if let Some(session_id) = query.session_id.as_deref() {
        if session_id.is_empty() {
            None
        } else {
            Some(
                stripe_payments::fulfill_checkout_session(&state, session_id)
                    .await
                    .map_err(|err| {
                        eprintln!("checkout fulfill failed for {session_id}: {err}");
                        err
                    }),
            )
        }
    } else {
        None
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if let Some(pet_id) = query.pet.as_deref() {
        let owner = query.pet_owner.as_deref();
        if sharing::accessible_pet_exists(&state, &profile, pet_id, owner)
            && sharing::set_active_pet_selection(&mut profile, pet_id, owner)
        {
            let _ = save_profile(&state, &profile).await;
        }
    }
    let pet_view = sharing::active_pet_view_profile(&state, &profile);
    let tasks_view = sharing::tasks_view_profile(&state, &profile);
    let calendar_view = sharing::calendar_view_profile(&state, &profile);
    let owned_pets = sharing::pet_summaries_for_profile(&profile);
    let dashboard_status = if !profile.pending_purrfect_idea_ids.is_empty() {
        profile.pending_purrfect_idea_ids.clear();
        let _ = save_profile(&state, &profile).await;
        Some("feedback_idea_purrfect")
    } else if query.status.as_deref() == Some("premium_bought") && !user_has_premium(&profile) {
        if checkout_fulfill_result.is_some_and(|result| result.is_err()) {
            Some("premium_fulfill_failed")
        } else {
            query.status.as_deref()
        }
    } else {
        query.status.as_deref()
    };
    let show_vet_followup = profile.vet_followup_pending
        || query
            .vet_followup
            .as_deref()
            .is_some_and(|value| value == "1");
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
    let (calendar_month, calendar_year) =
        resolve_calendar_view(query.cal_month.as_deref(), query.cal_year.as_deref());
    let (form_name, form_email) = form_prefill(&state, &jar).await;
    let stripe_enabled = stripe_payments::stripe_checkout_enabled();
    let additional_pet_tuples: Vec<(String, String, String)> = profile
        .additional_pets
        .iter()
        .map(|pet| {
            (
                pet.pet_name.clone(),
                pet.pet_breed.clone(),
                pet.pet_color.clone(),
            )
        })
        .collect();
    let additional_pets_html = entitlements::render_additional_pet_cards(&additional_pet_tuples);
    let account_premium_section = entitlements::render_account_premium_section(
        profile.premium_unlocked,
        &email,
        stripe_enabled,
    );
    let multi_pet_section = entitlements::render_multi_pet_section(
        profile.premium_unlocked,
        &email,
        profile_has_pet(&profile),
        &profile.pet_name,
        profile.additional_pets.len(),
        &additional_pets_html,
        stripe_enabled,
    );
    let add_cat_modal = if entitlements::should_render_add_cat_modal(
        profile.premium_unlocked,
        &email,
        profile_has_pet(&profile),
        profile.additional_pets.len(),
    ) {
        render_add_cat_onboarding_modal()
    } else {
        String::new()
    };
    let streak_card_section = share_cards::render_streak_card(
        &profile,
        &stripe_payments::public_app_url(),
        timestamp_now(),
    );

    let body = DASHBOARD_HTML
        .replace("{{USER_NAME}}", &escape_html(&username))
        .replace("{{USER_FIRST_NAME}}", &escape_html(&first_name))
        .replace("{{USER_LAST_NAME}}", &escape_html(&last_name))
        .replace("{{USER_USERNAME}}", &escape_html(&username))
        .replace("{{USER_EMAIL}}", &escape_html(&email))
        .replace("{{ACCOUNT_PET_PHOTO}}", &memorial::render_account_pet_photo(&pet_view))
        .replace(
            "{{ACCOUNT_MEMORIAL_SECTION}}",
            &format!(
                "{}{}",
                memorial::render_mark_memorial_card(&pet_view),
                memorial::render_memorial_video_uploads(&pet_view),
            ),
        )
        .replace(
            "{{MEMORIAL_COMFORT_MODAL}}",
            &memorial::render_memorial_comfort_modal(&pet_view),
        )
        .replace(
            "{{ACCOUNT_PASSWORD_SECTION}}",
            &render_account_password_section(&email),
        )
        .replace("{{ACCOUNT_PREMIUM_SECTION}}", &account_premium_section)
        .replace(
            "{{COMMUNITY_VISIBILITY_SECTION}}",
            &community::render_account_visibility_section(&profile),
        )
        .replace(
            "{{FRIENDS_TAB_LABEL}}",
            &sharing::render_friends_tab_label(&state, &email),
        )
        .replace(
            "{{FRIENDS_AND_SHARING_SECTION}}",
            &sharing::render_account_friends_section(&state, &email, &owned_pets),
        )
        .replace(
            "{{ACCOUNT_NOTIFICATIONS_SECTION}}",
            &push_notifications::render_account_notifications_section(&profile),
        )
        .replace(
            "{{ACCOUNT_ONBOARDING_EMAILS_SECTION}}",
            &onboarding_emails::render_account_onboarding_emails_section(&profile),
        )
        .replace(
            "{{ACCOUNT_PET_NAME_FIELD}}",
            &render_account_pet_name_field(&pet_view),
        )
        .replace(
            "{{MEMBER_SINCE}}",
            &escape_html(&member_since_label(&state, &email).await),
        )
        .replace("{{PAW_POINTS}}", &profile.paw_points.to_string())
        .replace("{{PAW_POINTS_ICON}}", paw_points_icon_html())
        .replace(
            "{{PAW_POINTS_BALANCE}}",
            &paw_points_amount_html(profile.paw_points),
        )
        .replace("{{PARENT_LEVEL}}", &profile.parent_level.to_string())
        .replace(
            "{{CARE_STREAK_CHIP}}",
            &streak_rewards::render_care_streak_chip(&profile),
        )
        .replace("{{STREAK_CARD_SECTION}}", &streak_card_section)
        .replace("{{LEVEL_PROGRESS}}", &level_progress_pct.to_string())
        .replace(
            "{{LEVEL_PROGRESS_TEXT}}",
            &escape_html(&level_progress_text),
        )
        .replace("{{PET_SWITCHER}}", &sharing::render_pet_switcher(&state, &profile))
        .replace("{{SHARED_PET_BANNER}}", &sharing::render_shared_pet_banner(&state, &profile))
        .replace("{{PET_NAME}}", &escape_html(&display_pet_name(&pet_view)))
        .replace("{{PET_BLURB}}", &render_pet_blurb(&pet_view))
        .replace("{{PET_CHECK_CTA}}", &render_pet_check_cta(&pet_view))
        .replace(
            "{{PET_VIDEO_UPLOAD_CTA}}",
            &render_pet_video_upload_cta(&pet_view),
        )
        .replace("{{PET_VIDEO_MODAL}}", &render_pet_video_modal(&pet_view))
        .replace(
            "{{ACCOUNT_PET_PHOTO_MODAL}}",
            &render_account_pet_photo_modal(&profile),
        )
        .replace("{{CAT_HOME_NAV_LINK}}", &render_cat_home_nav_link(&pet_view))
        .replace("{{PET_SETUP_CTA}}", &render_pet_setup_cta(&profile))
        .replace(
            "{{NEEDS_PET_SETUP_DATA}}",
            if user_needs_pet_setup(&profile)
                && sharing::list_accessible_pets(&state, &profile).is_empty()
            {
                r#"data-needs-pet-setup="true""#
            } else {
                ""
            },
        )
        .replace("{{PET_META}}", &render_pet_meta(&pet_view))
        .replace("{{PET_AVATAR}}", &render_pet_avatar(&pet_view))
        .replace("{{PET_HEALTH_INFO}}", &render_pet_health_info(&pet_view))
        .replace("{{MULTI_PET_SECTION}}", &multi_pet_section)
        .replace(
            "{{PET_VET_ALERT}}",
            &render_vet_urgency_alert(&pet_view, "pet-tab-vet-alert"),
        )
        .replace(
            "{{CALENDAR_VET_ALERT}}",
            &render_vet_urgency_alert(&calendar_view, "calendar-tab-vet-alert"),
        )
        .replace(
            "{{CALENDAR_SHARED_BANNER}}",
            &sharing::render_calendar_shared_banner(&state, &profile),
        )
        .replace(
            "{{CALENDAR_PET_SETUP_PROMPT}}",
            &render_calendar_pet_setup_prompt(&profile),
        )
        .replace(
            "{{TASKS_PET_SETUP_PROMPT}}",
            &render_tasks_tab_setup_prompt(&profile),
        )
        .replace("{{ONBOARDING_MODAL}}", &render_onboarding_modal(&profile))
        .replace("{{ADD_CAT_MODAL}}", &add_cat_modal)
        .replace(
            "{{VET_FOLLOWUP_MODAL}}",
            &render_vet_followup_modal(&profile, show_vet_followup),
        )
        .replace(
            "{{PARENT_LEVEL_BREAKDOWN_MODAL}}",
            &render_parent_level_breakdown(&profile),
        )
        .replace("{{SHARE_CARD_MODAL}}", render_share_card_modal())
        .replace("{{HEALTH_TAB_CONTENT}}", &render_health_tab(&pet_view))
        .replace(
            "{{EQUIPPED_OUTFIT}}",
            &escape_html(&profile.equipped_outfit),
        )
        .replace(
            "{{STATUS_BLOCK}}",
            &render_dashboard_status_area(&profile, dashboard_status),
        )
        .replace("{{ACTIVITY_LIST}}", &render_activity_list(&profile))
        .replace(
            "{{TASKS_SHARED_BANNER}}",
            &sharing::render_tasks_shared_banner(&tasks_view, &state),
        )
        .replace("{{TASK_LIST}}", &render_task_list(&tasks_view))
        .replace("{{TASK_ADD_SECTION}}", &render_task_add_section(&tasks_view))
        .replace(
            "{{CALENDAR_GRID}}",
            &render_calendar_grid(
                &state,
                &profile,
                &calendar_view,
                calendar_month,
                calendar_year,
            ),
        )
        .replace("{{EVENT_LIST}}", &render_event_list(&state, &profile))
        .replace(
            "{{CALENDAR_DATA_JSON}}",
            &render_calendar_data_json(
                &state,
                &profile,
                &calendar_view,
                calendar_month,
                calendar_year,
            ),
        )
        .replace(
            "{{CALENDAR_MONTH_LABEL}}",
            &calendar_month_label(calendar_month, calendar_year),
        )
        .replace(
            "{{BUY_POINTS_SECTION}}",
            &stripe_payments::render_buy_points_section(),
        )
        .replace(
            "{{SAVED_PAYMENT_METHODS}}",
            &stripe_payments::render_saved_payment_methods(&state, &profile).await,
        )
        .replace(
            "{{FEEDBACK_TAB_CONTENT}}",
            &render_feedback_forum(
                &state,
                &form_name,
                &form_email,
                query
                    .feedback
                    .as_deref()
                    .and_then(|value| value.parse::<i64>().ok()),
                Some(email.as_str()),
            ),
        );
    let open_thread = query
        .thread
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok());
    let community_section = normalize_community_section(query.community.as_deref());
    let breed_filter = query.breed.as_deref();
    let body = body.replace(
        "{{FORUM_TAB_CONTENT}}",
        &render_dashboard_forum_tab(
            &state,
            &profile,
            open_thread,
            &email,
            community_section,
            breed_filter,
        ),
    );
    let body = replace_admin_nav_link(&body, &state, &jar);

    (jar, Html(body)).into_response()
}

fn pet_media_return_tab(value: &str) -> &'static str {
    if value.trim() == "account" {
        "account"
    } else {
        "pet"
    }
}

async fn pet_name_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<ChangePetNameForm>,
) -> Response {
    let wants_json = wants_json_response(&headers);
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => {
            return if wants_json {
                api_auth_error(true)
            } else {
                redirect.into_response()
            };
        }
    };

    let Some(new_name) = normalize_pet_name(&form.pet_name) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(PetNameChangeResponse {
                    ok: false,
                    status: "invalid",
                    pet_name: String::new(),
                }),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=account&status=pet_name_invalid").into_response()
        };
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(PetNameChangeResponse {
                    ok: false,
                    status: "invalid",
                    pet_name: String::new(),
                }),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=account&status=pet_name_invalid").into_response()
        };
    }

    if profile.pet_name != new_name {
        let previous_name = profile.pet_name.clone();
        profile.pet_name = new_name.clone();
        let today = Local::now().date_naive();
        profile.calendar_events = merge_calendar_events(&profile, today);
        let _ = refresh_profile_tasks(&mut profile);

        push_activity(
            &mut profile,
            &format!("Renamed {previous_name} to {new_name}."),
        );

        if let Err(error) = save_profile(&state, &profile).await {
            eprintln!("pet name change failed for {email}: {error}");
            return if wants_json {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(PetNameChangeResponse {
                        ok: false,
                        status: "invalid",
                        pet_name: String::new(),
                    }),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=account&status=pet_name_invalid").into_response()
            };
        }
    }

    if wants_json {
        Json(PetNameChangeResponse {
            ok: true,
            status: "done",
            pet_name: new_name,
        })
        .into_response()
    } else {
        Redirect::to("/home?tab=account&status=pet_name_done").into_response()
    }
}

async fn password_change_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChangePasswordForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    if is_admin_account(&email) {
        return Redirect::to("/home?tab=account&status=password_error");
    }

    let current_password = form.current_password.trim();
    let new_password = form.new_password.trim();
    let confirm_password = form.confirm_password.trim();

    if current_password.is_empty() || new_password.is_empty() || confirm_password.is_empty() {
        return Redirect::to("/home?tab=account&status=password_missing");
    }

    if !signup_passwords_match(new_password, confirm_password) {
        return Redirect::to("/home?tab=account&status=password_mismatch");
    }

    if !password_meets_signup_requirements(new_password) {
        return Redirect::to("/home?tab=account&status=password_requirements");
    }

    if current_password == new_password {
        return Redirect::to("/home?tab=account&status=password_same");
    }

    match state.storage.validate_login(&email, current_password) {
        Ok(true) => {}
        Ok(false) => return Redirect::to("/home?tab=account&status=password_invalid"),
        Err(error) => {
            eprintln!("password change validation failed for {email}: {error}");
            return Redirect::to("/home?tab=account&status=password_error");
        }
    }

    match state.storage.set_user_password(&email, new_password) {
        Ok(()) => Redirect::to("/home?tab=account&status=password_done"),
        Err(error) => {
            eprintln!("password change failed for {email}: {error}");
            Redirect::to("/home?tab=account&status=password_error")
        }
    }
}

async fn pet_photo_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut return_tab = "pet";
    let mut photo_bytes: Option<Vec<u8>> = None;
    let mut photo_content_type: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "pet_photo" {
            photo_content_type = field.content_type().map(str::to_string);
            match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => photo_bytes = Some(bytes.to_vec()),
                Ok(_) => {}
                Err(_) => return Redirect::to("/home?tab=account&status=pet_photo_invalid"),
            }
            continue;
        }

        if name == "return_tab" {
            if let Ok(text) = field.text().await {
                return_tab = pet_media_return_tab(&text);
            }
        }
    }

    let redirect_invalid = format!("/home?tab={return_tab}&status=pet_photo_invalid");

    let Some(bytes) = photo_bytes else {
        return Redirect::to(&redirect_invalid);
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        return Redirect::to(&redirect_invalid);
    }

    let ext = match validate_pet_photo(photo_content_type.as_deref(), &bytes) {
        Ok(ext) => ext,
        Err(()) => return Redirect::to(&redirect_invalid),
    };

    let active_id = active_pet_id(&profile).to_string();
    let photo_pet_id = if active_id == PRIMARY_PET_ID {
        None
    } else {
        Some(active_id.as_str())
    };
    match save_pet_photo(&state, &email, &bytes, ext, photo_pet_id).await {
        Ok(url) => apply_pet_photo_url(&mut profile, &active_id, url),
        Err(_) => return Redirect::to(&redirect_invalid),
    }

    let pet_name = active_pet_snapshot(&profile)
        .map(|pet| pet.pet_name.clone())
        .unwrap_or_else(|| profile.pet_name.clone());
    push_activity(
        &mut profile,
        &format!("Updated the profile photo for {pet_name}."),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to(&format!("/home?tab={return_tab}&status=pet_photo_done")),
        Err(_) => Redirect::to(&redirect_invalid),
    }
}

async fn pet_video_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut return_tab = "pet";
    let mut clip_start = 0.0f32;
    let mut clip_duration = PET_VIDEO_CLIP_MAX_SECONDS;
    let mut video_zoom: Option<f32> = None;
    let mut video_offset_x: Option<f32> = None;
    let mut video_offset_y: Option<f32> = None;
    let mut video_bytes: Option<Vec<u8>> = None;
    let mut video_content_type: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "pet_video" {
            video_content_type = field.content_type().map(str::to_string);
            match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => video_bytes = Some(bytes.to_vec()),
                Ok(_) => {}
                Err(_) => return Redirect::to("/home?tab=pet&status=pet_video_invalid"),
            }
            continue;
        }

        if name == "return_tab" {
            if let Ok(text) = field.text().await {
                return_tab = pet_media_return_tab(&text);
            }
        } else if name == "pet_video_clip_start" {
            if let Ok(text) = field.text().await {
                clip_start = parse_pet_video_clip_start(&text);
            }
        } else if name == "pet_video_clip_duration" {
            if let Ok(text) = field.text().await {
                clip_duration = parse_pet_video_clip_duration(&text);
            }
        } else if name == "pet_video_zoom" {
            if let Ok(text) = field.text().await {
                video_zoom = parse_optional_video_float(&text);
            }
        } else if name == "pet_video_offset_x" {
            if let Ok(text) = field.text().await {
                video_offset_x = parse_optional_video_float(&text);
            }
        } else if name == "pet_video_offset_y" {
            if let Ok(text) = field.text().await {
                video_offset_y = parse_optional_video_float(&text);
            }
        }
    }

    let redirect_invalid = format!("/home?tab={return_tab}&status=pet_video_invalid");

    let Some(bytes) = video_bytes else {
        return Redirect::to(&redirect_invalid);
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        return Redirect::to(&redirect_invalid);
    }

    let ext = match validate_pet_video(video_content_type.as_deref(), &bytes) {
        Ok(ext) => ext,
        Err(()) => return Redirect::to(&redirect_invalid),
    };

    let active_id = active_pet_id(&profile).to_string();
    let video_pet_id = if active_id == PRIMARY_PET_ID {
        None
    } else {
        Some(active_id.as_str())
    };
    match save_pet_video(&state, &email, &bytes, ext, video_pet_id).await {
        Ok(url) => {
            if active_id == PRIMARY_PET_ID {
                profile.pet_video_url = Some(url);
                profile.pet_video_clip_start = Some(clip_start);
                profile.pet_video_clip_duration = Some(clip_duration);
                profile.pet_video_zoom = video_zoom;
                profile.pet_video_offset_x = video_offset_x;
                profile.pet_video_offset_y = video_offset_y;
            } else if let Some(pet) = profile
                .additional_pets
                .iter_mut()
                .find(|pet| pet.id == active_id)
            {
                pet.pet_video_url = Some(url);
                pet.pet_video_clip_start = Some(clip_start);
                pet.pet_video_clip_duration = Some(clip_duration);
                pet.pet_video_zoom = video_zoom;
                pet.pet_video_offset_x = video_offset_x;
                pet.pet_video_offset_y = video_offset_y;
            }
        }
        Err(_) => return Redirect::to(&redirect_invalid),
    }

    let pet_name = display_pet_name(&profile);
    push_activity(
        &mut profile,
        &format!("Added a playing video clip for {pet_name}."),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to(&format!("/home?tab={return_tab}&status=pet_video_done")),
        Err(_) => Redirect::to(&redirect_invalid),
    }
}

async fn memorialize_pet_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<MemorialPetForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    let pet_id = form.pet_id.trim();
    if pet_id.is_empty() || profile.active_pet_owner_email.is_some() {
        return Redirect::to("/home?tab=account&status=memorial_invalid");
    }

    let pet_name = pet_snapshot(&profile, pet_id)
        .map(|snapshot| snapshot.pet_name.clone())
        .unwrap_or_else(|| profile.pet_name.clone());

    if !memorial::memorialize_pet(&mut profile, pet_id) {
        return Redirect::to("/home?tab=account&status=memorial_invalid");
    }

    memorial::push_memorial_activity(&mut profile, &pet_name);
    let _ = refresh_profile_tasks(&mut profile);

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account&status=memorial_started"),
        Err(_) => Redirect::to("/home?tab=account&status=memorial_invalid"),
    }
}

async fn memorial_comfort_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<MemorialPetForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    let pet_id = form.pet_id.trim();
    if !memorial::dismiss_memorial_comfort(&mut profile, pet_id) {
        return Redirect::to("/home?tab=account&status=memorial_invalid");
    }

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account"),
        Err(_) => Redirect::to("/home?tab=account&status=memorial_invalid"),
    }
}

async fn memorial_video_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut pet_id = String::new();
    let mut slot: Option<usize> = None;
    let mut video_bytes: Option<Vec<u8>> = None;
    let mut video_content_type: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "memorial_video" {
            video_content_type = field.content_type().map(str::to_string);
            match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => video_bytes = Some(bytes.to_vec()),
                Ok(_) => {}
                Err(_) => return Redirect::to("/home?tab=account&status=memorial_video_invalid"),
            }
            continue;
        }
        if name == "pet_id" {
            if let Ok(text) = field.text().await {
                pet_id = text.trim().to_string();
            }
        } else if name == "slot" {
            if let Ok(text) = field.text().await {
                slot = text.trim().parse().ok();
            }
        }
    }

    let Some(bytes) = video_bytes else {
        return Redirect::to("/home?tab=account&status=memorial_video_invalid");
    };
    let Some(slot) = slot.filter(|value| *value < memorial::MAX_MEMORIAL_VIDEOS) else {
        return Redirect::to("/home?tab=account&status=memorial_video_invalid");
    };
    if pet_id.is_empty() {
        return Redirect::to("/home?tab=account&status=memorial_video_invalid");
    }

    let ext = match validate_pet_video(video_content_type.as_deref(), &bytes) {
        Ok(ext) => ext,
        Err(()) => return Redirect::to("/home?tab=account&status=memorial_video_invalid"),
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if !memorial::pet_is_deceased(&profile, &pet_id) {
        return Redirect::to("/home?tab=account&status=memorial_invalid");
    }

    match save_memorial_video(&state, &email, &bytes, ext, &pet_id, slot).await {
        Ok(url) => {
            memorial::set_memorial_video_slot(&mut profile, &pet_id, slot, url);
        }
        Err(_) => return Redirect::to("/home?tab=account&status=memorial_video_invalid"),
    }

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account&status=memorial_video_saved"),
        Err(_) => Redirect::to("/home?tab=account&status=memorial_video_invalid"),
    }
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
    let mut video_bytes: Option<Vec<u8>> = None;
    let mut video_content_type: Option<String> = None;

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
        if name == "pet_video" {
            video_content_type = field.content_type().map(str::to_string);
            match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => video_bytes = Some(bytes.to_vec()),
                Ok(_) => {}
                Err(_) => return Redirect::to("/home?status=onboarding_video_invalid"),
            }
            continue;
        }

        match field.text().await {
            Ok(text) => fields.entry(name).or_default().push(text),
            Err(_) => return Redirect::to("/home?status=onboarding_invalid"),
        }
    }

    let adding_pet = fields
        .get("add_pet")
        .is_some_and(|values| values.first().is_some_and(|value| value == "1"));
    let photo_invalid_redirect = if adding_pet {
        "/home?tab=pet&add_cat=1&status=onboarding_photo_invalid"
    } else {
        "/home?status=onboarding_photo_invalid"
    };

    let Some(photo_bytes) = photo_bytes else {
        return Redirect::to(photo_invalid_redirect);
    };

    let photo_ext = match validate_pet_photo(photo_content_type.as_deref(), &photo_bytes) {
        Ok(ext) => ext,
        Err(()) => return Redirect::to(photo_invalid_redirect),
    };

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

    let signup_date = Local::now().date_naive();
    let dob = match validate_pet_birth_date(&form.pet_birth_date, signup_date) {
        Ok(dob) => dob,
        Err(()) => return Redirect::to("/home?status=onboarding_invalid"),
    };
    let (pet_age_weeks, pet_age_years) = match derive_age_from_birth(dob, signup_date) {
        Ok(age) => age,
        Err(()) => return Redirect::to("/home?status=onboarding_invalid"),
    };

    let indoor_outdoor = form.pet_indoor_outdoor.trim().to_lowercase();
    if indoor_outdoor != "indoor" && indoor_outdoor != "outdoor" {
        return Redirect::to("/home?status=onboarding_invalid");
    }

    let mut profile = get_or_create_profile(&state, &email).await;

    if adding_pet {
        if !user_has_premium(&profile) {
            return Redirect::to("/home?tab=pet&status=premium_required");
        }
        if !profile_has_pet(&profile) {
            return Redirect::to("/home?tab=pet&status=pet_add_invalid");
        }
        let additional_count = profile.additional_pets.len();
        if !entitlements::can_add_pet(
            profile.premium_unlocked,
            &profile.email,
            true,
            additional_count,
        ) {
            return Redirect::to("/home?tab=pet&status=pet_add_invalid");
        }

        if user_has_premium(&profile) && !form.never_been_to_vet {
            let trimmed = form.last_vet_date.trim();
            if !trimmed.is_empty() && parse_vet_date(trimmed).is_none() {
                return Redirect::to("/home?tab=pet&status=onboarding_invalid");
            }
        }

        let mut new_pet = household_pet_from_onboarding(
            &form,
            dob,
            pet_age_weeks,
            pet_age_years,
            indoor_outdoor.clone(),
            user_has_premium(&profile),
        );
        let new_pet_id = new_pet.id.clone();
        let added_name = new_pet.pet_name.clone();

        match save_pet_photo(
            &state,
            &email,
            &photo_bytes,
            photo_ext,
            Some(&new_pet_id),
        )
        .await
        {
            Ok(url) => new_pet.pet_photo_url = Some(url),
            Err(_) => {
                return Redirect::to("/home?tab=pet&add_cat=1&status=onboarding_photo_invalid")
            }
        }

        if !form.skip_video {
            if let Some(bytes) = video_bytes {
                let ext = match validate_pet_video(video_content_type.as_deref(), &bytes) {
                    Ok(ext) => ext,
                    Err(()) => return Redirect::to("/home?tab=pet&status=onboarding_video_invalid"),
                };
                match save_pet_video(&state, &email, &bytes, ext, Some(&new_pet_id)).await {
                    Ok(url) => {
                        new_pet.pet_video_url = Some(url);
                        new_pet.pet_video_clip_start = Some(form.pet_video_clip_start);
                        new_pet.pet_video_clip_duration = Some(form.pet_video_clip_duration);
                        new_pet.pet_video_zoom = form.pet_video_zoom;
                        new_pet.pet_video_offset_x = form.pet_video_offset_x;
                        new_pet.pet_video_offset_y = form.pet_video_offset_y;
                    }
                    Err(_) => {
                        return Redirect::to("/home?tab=pet&status=onboarding_video_invalid")
                    }
                }
            }
        }

        profile.additional_pets.push(new_pet);
        profile.active_pet_id = new_pet_id.clone();
        profile.calendar_events = merge_calendar_events(&profile, signup_date);
        let _ = refresh_profile_tasks(&mut profile);
        push_activity(
            &mut profile,
            &format!("Added {added_name} to your household with a full care profile."),
        );

        return match save_profile(&state, &profile).await {
            Ok(()) => Redirect::to(&format!(
                "/home?tab=pet&status=pet_added&pet={}",
                urlencoding::encode(&new_pet_id)
            )),
            Err(_) => Redirect::to("/home?tab=pet&status=pet_add_invalid"),
        };
    }

    profile.pet_name = cat_name.to_string();
    profile.pet_breed = pet_breed.to_string();
    profile.pet_color = form.pet_color.trim().to_string();
    profile.pet_mood = "Happy".to_string();
    profile.pet_birth_date = Some(dob.format("%Y-%m-%d").to_string());
    profile.pet_age_weeks = pet_age_weeks;
    profile.pet_age_years = pet_age_years;
    profile.pet_indoor_outdoor = Some(indoor_outdoor);
    profile.onboarding_completed = true;
    profile.active_pet_id = PRIMARY_PET_ID.to_string();

    if user_has_premium(&profile) {
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

        profile.never_been_to_vet = form.never_been_to_vet;
        profile.last_vet_date = last_vet_date;
        profile.pet_conditions = form.conditions.trim().to_string();
        profile.pet_medications = form.medications.trim().to_string();
        profile.vaccine_history = vaccine_history;
        profile.pet_vaccines_unknown = form.pet_vaccines_unknown;
    }
    profile.calendar_events = merge_calendar_events(&profile, signup_date);
    let _ = refresh_profile_tasks(&mut profile);

    match save_pet_photo(&state, &email, &photo_bytes, photo_ext, None).await {
        Ok(url) => profile.pet_photo_url = Some(url),
        Err(_) => return Redirect::to("/home?status=onboarding_photo_invalid"),
    }

    if !form.skip_video {
        if let Some(bytes) = video_bytes {
            let ext = match validate_pet_video(video_content_type.as_deref(), &bytes) {
                Ok(ext) => ext,
                Err(()) => return Redirect::to("/home?status=onboarding_video_invalid"),
            };
            match save_pet_video(&state, &email, &bytes, ext, None).await {
                Ok(url) => {
                    profile.pet_video_url = Some(url);
                    profile.pet_video_clip_start = Some(form.pet_video_clip_start);
                    profile.pet_video_clip_duration = Some(form.pet_video_clip_duration);
                    profile.pet_video_zoom = form.pet_video_zoom;
                    profile.pet_video_offset_x = form.pet_video_offset_x;
                    profile.pet_video_offset_y = form.pet_video_offset_y;
                }
                Err(_) => return Redirect::to("/home?status=onboarding_video_invalid"),
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
    headers: HeaderMap,
    Form(form): Form<OutfitBuyForm>,
) -> impl IntoResponse {
    let wants_json = wants_json_response(&headers);
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let Some(outfit) = outfit_by_id(form.outfit_id.trim()) else {
        return if wants_json {
            shop_buy_json_response(false, "outfit_invalid", 0, "outfit", "", false)
        } else {
            outfit_redirect("", "outfit_invalid").into_response()
        };
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if profile.owned_outfits.iter().any(|id| id == outfit.id) {
        return if wants_json {
            shop_buy_json_response(
                false,
                "outfit_owned",
                profile.paw_points,
                "outfit",
                outfit.id,
                false,
            )
        } else {
            outfit_redirect("", "outfit_owned").into_response()
        };
    }

    if profile.paw_points < outfit.price {
        return if wants_json {
            shop_buy_json_response(
                false,
                "need_paw_points",
                profile.paw_points,
                "outfit",
                outfit.id,
                false,
            )
        } else {
            cat_home_need_paw_points_redirect(None, Some(outfit.id), None, None).into_response()
        };
    }

    profile.paw_points -= outfit.price;
    profile.owned_outfits.push(outfit.id.to_string());
    profile.equipped_outfit = outfit.name.to_string();
    push_activity(
        &mut profile,
        &format!("Purchased {} for {} paw points.", outfit.name, outfit.price),
    );

    match save_profile(&state, &profile).await {
        Ok(()) if wants_json => shop_buy_json_response(
            true,
            "outfit_bought",
            profile.paw_points,
            "outfit",
            outfit.id,
            true,
        ),
        Ok(()) => outfit_redirect("", "outfit_bought").into_response(),
        Err(_) if wants_json => {
            shop_buy_json_response(false, "outfit_invalid", profile.paw_points, "outfit", outfit.id, false)
        }
        Err(_) => outfit_redirect("", "outfit_invalid").into_response(),
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
        return outfit_redirect("", "outfit_invalid");
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile.owned_outfits.iter().any(|id| id == outfit.id) {
        return outfit_redirect("", "outfit_invalid");
    }

    profile.equipped_outfit = outfit.name.to_string();
    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Equipped {} on {}.", outfit.name, pet_name),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => outfit_redirect("", "outfit_equipped"),
        Err(_) => outfit_redirect("", "outfit_invalid"),
    }
}

async fn decor_buy(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<DecorBuyForm>,
) -> impl IntoResponse {
    let wants_json = wants_json_response(&headers);
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let Some(decor) = decor_by_id(form.decor_id.trim()) else {
        return if wants_json {
            shop_buy_json_response(false, "decor_invalid", 0, "decor", "", false)
        } else {
            Redirect::to("/home/cat-home?status=decor_invalid").into_response()
        };
    };

    if decor.price == 0 {
        return if wants_json {
            shop_buy_json_response(false, "decor_invalid", 0, "decor", decor.id, false)
        } else {
            Redirect::to("/home/cat-home?status=decor_invalid").into_response()
        };
    }

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet").into_response();
    }

    if profile.owned_decor.iter().any(|id| id == decor.id) {
        return if wants_json {
            shop_buy_json_response(
                false,
                "decor_owned",
                profile.paw_points,
                "decor",
                decor.id,
                false,
            )
        } else {
            Redirect::to("/home/cat-home?status=decor_owned").into_response()
        };
    }

    if profile.paw_points < decor.price {
        return if wants_json {
            shop_buy_json_response(
                false,
                "need_paw_points",
                profile.paw_points,
                "decor",
                decor.id,
                false,
            )
        } else {
            cat_home_need_paw_points_redirect(Some(decor.id), None, None, None).into_response()
        };
    }

    profile.paw_points -= decor.price;
    profile.owned_decor.push(decor.id.to_string());
    profile
        .equipped_decor
        .insert(decor.slot.to_string(), decor.id.to_string());
    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!(
            "Purchased {} for {} paw points for {}'s home.",
            decor.name, decor.price, pet_name
        ),
    );

    match save_profile(&state, &profile).await {
        Ok(()) if wants_json => shop_buy_json_response(
            true,
            "decor_bought",
            profile.paw_points,
            "decor",
            decor.id,
            true,
        ),
        Ok(()) => Redirect::to("/home/cat-home?status=decor_bought").into_response(),
        Err(_) if wants_json => {
            shop_buy_json_response(false, "decor_invalid", profile.paw_points, "decor", decor.id, false)
        }
        Err(_) => Redirect::to("/home/cat-home?status=decor_invalid").into_response(),
    }
}

async fn tap_boost_buy(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<TapBoostBuyForm>,
) -> impl IntoResponse {
    let wants_json = wants_json_response(&headers);
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let Some(boost) = tap_boost_by_id(form.boost_id.trim()) else {
        return if wants_json {
            shop_buy_json_response(false, "boost_invalid", 0, "boost", "", false)
        } else {
            tap_boost_redirect("boost_invalid").into_response()
        };
    };

    if boost.price == 0 {
        return if wants_json {
            shop_buy_json_response(false, "boost_invalid", 0, "boost", boost.id, false)
        } else {
            tap_boost_redirect("boost_invalid").into_response()
        };
    }

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet").into_response();
    }

    if profile.owned_tap_boosts.iter().any(|id| id == boost.id) {
        return if wants_json {
            shop_buy_json_response(
                false,
                "boost_owned",
                profile.paw_points,
                "boost",
                boost.id,
                false,
            )
        } else {
            tap_boost_redirect("boost_owned").into_response()
        };
    }

    if profile.paw_points < boost.price {
        return if wants_json {
            shop_buy_json_response(
                false,
                "need_paw_points",
                profile.paw_points,
                "boost",
                boost.id,
                false,
            )
        } else {
            cat_home_need_paw_points_redirect(None, None, Some(boost.id), None).into_response()
        };
    }

    profile.paw_points -= boost.price;
    profile.owned_tap_boosts.push(boost.id.to_string());
    profile.equipped_tap_boost = boost.id.to_string();
    push_activity(
        &mut profile,
        &format!(
            "Purchased {} tap boost (+{} per pet) for {} paw points.",
            boost.name, boost.tap_reward, boost.price
        ),
    );

    match save_profile(&state, &profile).await {
        Ok(()) if wants_json => shop_buy_json_response(
            true,
            "boost_bought",
            profile.paw_points,
            "boost",
            boost.id,
            true,
        ),
        Ok(()) => tap_boost_redirect("boost_bought").into_response(),
        Err(_) if wants_json => {
            shop_buy_json_response(false, "boost_invalid", profile.paw_points, "boost", boost.id, false)
        }
        Err(_) => tap_boost_redirect("boost_invalid").into_response(),
    }
}

async fn tap_boost_equip(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<TapBoostEquipForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let Some(boost) = tap_boost_by_id(form.boost_id.trim()) else {
        return tap_boost_redirect("boost_invalid");
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet");
    }

    if !profile.owned_tap_boosts.iter().any(|id| id == boost.id) {
        return tap_boost_redirect("boost_invalid");
    }

    profile.equipped_tap_boost = boost.id.to_string();
    push_activity(
        &mut profile,
        &format!(
            "Activated {} tap boost (+{} per pet).",
            boost.name, boost.tap_reward
        ),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => tap_boost_redirect("boost_equipped"),
        Err(_) => tap_boost_redirect("boost_invalid"),
    }
}

async fn petting_bonus_buy(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<PettingBonusBuyForm>,
) -> impl IntoResponse {
    let wants_json = wants_json_response(&headers);
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let Some(bonus) = petting_bonus_by_id(form.bonus_id.trim()) else {
        return if wants_json {
            shop_buy_json_response(false, "petting_bonus_invalid", 0, "bonus", "", false)
        } else {
            petting_bonus_redirect("petting_bonus_invalid").into_response()
        };
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet").into_response();
    }

    if profile.paw_points < bonus.price {
        return if wants_json {
            shop_buy_json_response(
                false,
                "need_paw_points",
                profile.paw_points,
                "bonus",
                bonus.id,
                false,
            )
        } else {
            cat_home_need_paw_points_redirect(None, None, None, Some(bonus.id)).into_response()
        };
    }

    profile.paw_points -= bonus.price;
    add_petting_bonus_to_inventory(&mut profile, bonus.id);
    push_activity(
        &mut profile,
        &format!(
            "Purchased {} petting bonus ({}× for {} seconds) for {} paw points.",
            bonus.name, bonus.multiplier, bonus.duration_secs, bonus.price
        ),
    );

    match save_profile(&state, &profile).await {
        Ok(()) if wants_json => shop_buy_json_response(
            true,
            "petting_bonus_bought",
            profile.paw_points,
            "bonus",
            bonus.id,
            false,
        ),
        Ok(()) => petting_bonus_redirect("petting_bonus_bought").into_response(),
        Err(_) if wants_json => shop_buy_json_response(
            false,
            "petting_bonus_invalid",
            profile.paw_points,
            "bonus",
            bonus.id,
            false,
        ),
        Err(_) => petting_bonus_redirect("petting_bonus_invalid").into_response(),
    }
}

async fn petting_bonus_activate(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<PettingBonusActivateForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let Some(bonus) = petting_bonus_by_id(form.bonus_id.trim()) else {
        return petting_bonus_redirect("petting_bonus_invalid");
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet");
    }

    clear_expired_petting_bonus(&mut profile);

    if active_petting_bonus_multiplier(&profile).is_some() {
        return petting_bonus_redirect("petting_bonus_active");
    }

    if petting_bonus_inventory_count(&profile, bonus.id) == 0 {
        return petting_bonus_redirect("petting_bonus_empty");
    }

    if !consume_petting_bonus_from_inventory(&mut profile, bonus.id) {
        return petting_bonus_redirect("petting_bonus_empty");
    }

    profile.active_petting_bonus = bonus.id.to_string();
    profile.petting_bonus_expires_at = timestamp_now() + u64::from(bonus.duration_secs);
    push_activity(
        &mut profile,
        &format!(
            "Activated {} petting bonus ({}× points for {} seconds).",
            bonus.name, bonus.multiplier, bonus.duration_secs
        ),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => petting_bonus_redirect("petting_bonus_activated"),
        Err(_) => petting_bonus_redirect("petting_bonus_invalid"),
    }
}

async fn decor_equip(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<DecorEquipForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let Some(decor) = decor_by_id(form.decor_id.trim()) else {
        return Redirect::to("/home/cat-home?status=decor_invalid");
    };

    let mut profile = get_or_create_profile(&state, &email).await;

    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet");
    }

    if !profile.owned_decor.iter().any(|id| id == decor.id) {
        return Redirect::to("/home/cat-home?status=decor_invalid");
    }

    profile
        .equipped_decor
        .insert(decor.slot.to_string(), decor.id.to_string());
    let pet_name = profile.pet_name.clone();
    push_activity(
        &mut profile,
        &format!("Placed {} in {}'s home.", decor.name, pet_name),
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home/cat-home?status=decor_equipped"),
        Err(_) => Redirect::to("/home/cat-home?status=decor_invalid"),
    }
}

async fn paw_points_balance(State(state): State<AppState>, jar: CookieJar) -> Response {
    let Some(email) = api_user_email(&state, &jar) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(PawPointsBalanceResponse {
                ok: false,
                paw_points: 0,
            }),
        )
            .into_response();
    };

    let profile = get_or_create_profile(&state, &email).await;
    (
        StatusCode::OK,
        Json(PawPointsBalanceResponse {
            ok: true,
            paw_points: profile.paw_points,
        }),
    )
        .into_response()
}

async fn pet_pet(State(state): State<AppState>, jar: CookieJar, headers: HeaderMap) -> Response {
    let wants_json = wants_json_response(&headers);
    let Some(email) = api_user_email(&state, &jar) else {
        return api_auth_error(wants_json);
    };

    let profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=pet").into_response()
        };
    }

    let pet_view = sharing::active_pet_view_profile(&state, &profile);
    if memorial::active_pet_is_deceased(&pet_view) {
        return if wants_json {
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "status": "memorial",
                    "message": "Your angel cat is resting peacefully."
                })),
            )
                .into_response()
        } else {
            Redirect::to("/home/cat-home?status=memorial_peace").into_response()
        };
    }

    let mut profile = profile;
    clear_expired_petting_bonus(&mut profile);
    let (tap_reward, tap_boost_base, tap_multiplier) = effective_tap_reward(&profile);
    profile.paw_points = profile.paw_points.saturating_add(tap_reward);
    let petting_bonus_expires_at = if tap_multiplier.is_some() {
        Some(profile.petting_bonus_expires_at)
    } else {
        None
    };

    match save_profile(&state, &profile).await {
        Ok(()) => {
            if wants_json {
                Json(PetPetResponse {
                    ok: true,
                    paw_points: profile.paw_points,
                    tap_reward,
                    tap_boost_base,
                    tap_multiplier,
                    petting_bonus_expires_at,
                })
                .into_response()
            } else {
                Redirect::to("/home/cat-home").into_response()
            }
        }
        Err(_) => {
            if wants_json {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "ok": false, "status": "error" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home/cat-home?status=pet_pet_failed").into_response()
            }
        }
    }
}

async fn cat_home_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<CatHomeQuery>,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=pet").into_response();
    }

    let pet_view = sharing::active_pet_view_profile(&state, &profile);
    let mut profile = profile;
    let expires_before = profile.petting_bonus_expires_at;
    clear_expired_petting_bonus(&mut profile);
    if profile.petting_bonus_expires_at != expires_before {
        let _ = save_profile(&state, &profile).await;
    }

    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let pet_name = display_pet_name(&pet_view);
    let cat_home_scene = if memorial::active_pet_is_deceased(&pet_view) {
        memorial::render_angel_cat_home_scene(&pet_view)
    } else {
        render_cat_home_scene(&pet_view)
    };

    match fs::read_to_string("templates/cat-home.html").await {
        Ok(template) => {
            let html = template
                .replace("{{USER_NAME}}", &escape_html(&username))
                .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar))
                .replace(
                    "{{STATUS_BLOCK}}",
                    &cat_home_status_block(query.status.as_deref()),
                )
                .replace("{{PET_NAME}}", &escape_html(&pet_name))
                .replace("{{PAW_POINTS}}", &profile.paw_points.to_string())
                .replace("{{PAW_POINTS_ICON}}", paw_points_icon_html())
                .replace("{{CAT_HOME_SCENE}}", &cat_home_scene)
                .replace(
                    "{{CAT_HOME_OUTFIT_SHOP}}",
                    &render_cat_home_outfit_shop(&profile),
                )
                .replace(
                    "{{CAT_HOME_TAP_BOOST_SHOP}}",
                    &render_cat_home_tap_boost_shop(&profile),
                )
                .replace(
                    "{{CAT_HOME_PETTING_BONUS_SHOP}}",
                    &render_cat_home_petting_bonus_shop(&profile),
                )
                .replace("{{DECOR_CARDS}}", &render_decor_cards(&profile))
                .replace(
                    "{{NEED_PAW_POINTS_MODAL}}",
                    &render_need_paw_points_modal(
                        &profile,
                        shop_item_from_cat_home_query(&query)
                            .filter(|item| profile.paw_points < item.price)
                            .as_ref(),
                    ),
                );
            (jar, Html(html)).into_response()
        }
        Err(_) => (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load cat home page",
            ),
        )
            .into_response(),
    }
}

fn wants_json_response(headers: &HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("application/json"))
}

fn task_dashboard_json_response(
    state: &AppState,
    profile: &UserProfile,
    status: &'static str,
    show_vet_followup: bool,
    share_card: Option<share_cards::ShareCardOffer>,
) -> Response {
    let (level_progress_pct, level_progress_text) = level_progress(profile);
    let calendar_month = current_calendar_month();
    let calendar_year = current_calendar_year();
    let calendar_profile = sharing::calendar_view_profile(state, profile);
    let tasks_profile = sharing::tasks_view_profile(state, profile);
    let calendar_data = serde_json::from_str(&render_calendar_data_json(
        state,
        profile,
        &calendar_profile,
        calendar_month,
        calendar_year,
    ))
    .unwrap_or_else(|_| serde_json::json!({}));

    Json(TaskToggleResponse {
        ok: true,
        status,
        tasks_html: render_task_list(&tasks_profile),
        activity_html: render_activity_list(profile),
        paw_points: profile.paw_points,
        paw_from_tasks: task_rewards_earned(profile),
        xp_from_tasks: task_xp_earned(profile),
        parent_level: profile.parent_level,
        parent_xp: profile.parent_xp,
        level_progress: level_progress_pct,
        level_progress_text,
        calendar_data,
        show_vet_followup,
        care_streak_days: profile.care_streak_days,
        share_card,
    })
    .into_response()
}

async fn task_time_update(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<TaskTimeForm>,
) -> Response {
    let wants_json = wants_json_response(&headers);
    let Some(email) = api_user_email(&state, &jar) else {
        return api_auth_error(wants_json);
    };

    let mut viewer = get_or_create_profile(&state, &email).await;
    if sharing::list_accessible_pets(&state, &viewer).is_empty() {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response()
        };
    }

    let task_id = form.task_id.trim();
    let pet_id = resolve_task_pet_id(&viewer, &form.pet_id);
    let Some(time_minutes) = parse_time_input(&form.task_time) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response()
        };
    };

    let Some(target) = sharing::resolve_pet_care_target(
        &state,
        &viewer,
        &pet_id,
        task_owner_hint(&viewer, &form.pet_owner),
    ) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response()
        };
    };

    let today = Local::now().date_naive();
    let activity_title;
    let save_result = if target.is_shared {
        let Some(mut owner) = sharing::load_profile_by_email(&state, &target.owner_email) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response()
            };
        };
        if find_task_index(&owner, task_id, &target.pet_id).is_none()
            || !apply_task_time_to_profile(&mut owner, task_id, time_minutes)
        {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response()
            };
        }
        owner.calendar_events = merge_calendar_events(&owner, today);
        activity_title = owner
            .tasks
            .iter()
            .find(|task| task.id == task_id && task.pet_id == target.pet_id)
            .map(|task| task.title.clone());
        save_profile(&state, &owner).await
    } else {
        if find_task_index(&viewer, task_id, &pet_id).is_none()
            || !apply_task_time_to_profile(&mut viewer, task_id, time_minutes)
        {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response()
            };
        }
        viewer.calendar_events = merge_calendar_events(&viewer, today);
        activity_title = viewer
            .tasks
            .iter()
            .find(|task| task.id == task_id && task.pet_id == pet_id)
            .map(|task| task.title.clone());
        save_profile(&state, &viewer).await
    };

    if let Some(title) = activity_title {
        push_activity(
            &mut viewer,
            &format!(
                "Updated \"{title}\" to {}.",
                format_time_from_minutes(time_minutes)
            ),
        );
        let _ = save_profile(&state, &viewer).await;
    }

    match save_result {
        Ok(()) if wants_json => {
            task_dashboard_json_response(&state, &viewer, "time_updated", false, None)
        }
        Ok(()) => Redirect::to("/home?tab=tasks&status=task_time_saved").into_response(),
        Err(_) if wants_json => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "status": "error" })),
        )
            .into_response(),
        Err(_) => Redirect::to("/home?tab=tasks&status=task_time_invalid").into_response(),
    }
}

async fn task_add(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<TaskAddForm>,
) -> Response {
    let wants_json = wants_json_response(&headers);
    let Some(email) = api_user_email(&state, &jar) else {
        return api_auth_error(wants_json);
    };

    let mut viewer = get_or_create_profile(&state, &email).await;
    if sharing::list_accessible_pets(&state, &viewer).is_empty() {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response()
        };
    }

    let Some(title) = sanitize_custom_task_title(&form.task_title) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response()
        };
    };

    let pet_id = resolve_task_pet_id(&viewer, &form.pet_id);
    let Some(target) = sharing::resolve_pet_care_target(
        &state,
        &viewer,
        &pet_id,
        task_owner_hint(&viewer, &form.pet_owner),
    ) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response()
        };
    };

    let today = Local::now().date_naive();
    let save_result = if target.is_shared {
        let Some(mut owner) = sharing::load_profile_by_email(&state, &target.owner_email) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response()
            };
        };
        if custom_task_count(&owner) >= MAX_CUSTOM_TASKS {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response()
            };
        }
        let task = create_custom_task(&owner, &target.pet_id, title.clone(), today);
        owner.tasks.push(task);
        sort_tasks_by_time(&mut owner.tasks);
        owner.calendar_events = merge_calendar_events(&owner, today);
        save_profile(&state, &owner).await
    } else {
        if custom_task_count(&viewer) >= MAX_CUSTOM_TASKS {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response()
            };
        }
        let task = create_custom_task(&viewer, &pet_id, title.clone(), today);
        viewer.tasks.push(task);
        sort_tasks_by_time(&mut viewer.tasks);
        viewer.calendar_events = merge_calendar_events(&viewer, today);
        save_profile(&state, &viewer).await
    };

    push_activity(
        &mut viewer,
        &format!("Added custom care task: {title} (+{CUSTOM_TASK_REWARD} paw points)."),
    );
    let _ = save_profile(&state, &viewer).await;

    match save_result {
        Ok(()) if wants_json => task_dashboard_json_response(&state, &viewer, "added", false, None),
        Ok(()) => Redirect::to("/home?tab=tasks&status=task_added").into_response(),
        Err(_) if wants_json => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "status": "error" })),
        )
            .into_response(),
        Err(_) => Redirect::to("/home?tab=tasks&status=task_add_invalid").into_response(),
    }
}

async fn task_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<TaskDeleteForm>,
) -> Response {
    let wants_json = wants_json_response(&headers);
    let Some(email) = api_user_email(&state, &jar) else {
        return api_auth_error(wants_json);
    };

    let mut viewer = get_or_create_profile(&state, &email).await;
    let task_id = form.task_id.trim();
    let pet_id = resolve_task_pet_id(&viewer, &form.pet_id);
    let Some(target) = sharing::resolve_pet_care_target(
        &state,
        &viewer,
        &pet_id,
        task_owner_hint(&viewer, &form.pet_owner),
    ) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_delete_invalid").into_response()
        };
    };

    let today = Local::now().date_naive();
    let (save_result, removed_title) = if target.is_shared {
        let Some(mut owner) = sharing::load_profile_by_email(&state, &target.owner_email) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_delete_invalid").into_response()
            };
        };
        let Some(task) = remove_custom_task(&mut owner, task_id, &target.pet_id) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_delete_invalid").into_response()
            };
        };
        owner.calendar_events = merge_calendar_events(&owner, today);
        (save_profile(&state, &owner).await, task.title)
    } else {
        let Some(task) = remove_custom_task(&mut viewer, task_id, &pet_id) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_delete_invalid").into_response()
            };
        };
        viewer.calendar_events = merge_calendar_events(&viewer, today);
        (save_profile(&state, &viewer).await, task.title)
    };

    push_activity(
        &mut viewer,
        &format!("Removed custom care task: {removed_title}."),
    );
    let _ = save_profile(&state, &viewer).await;

    match save_result {
        Ok(()) if wants_json => {
            task_dashboard_json_response(&state, &viewer, "deleted", false, None)
        }
        Ok(()) => Redirect::to("/home?tab=tasks&status=task_deleted").into_response(),
        Err(_) if wants_json => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "status": "error" })),
        )
            .into_response(),
        Err(_) => Redirect::to("/home?tab=tasks&status=task_delete_invalid").into_response(),
    }
}

async fn task_toggle(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<TaskToggleForm>,
) -> Response {
    let wants_json = wants_json_response(&headers);
    let Some(email) = api_user_email(&state, &jar) else {
        return api_auth_error(wants_json);
    };

    let mut viewer = get_or_create_profile(&state, &email).await;
    let task_id = form.task_id.trim();
    let pet_id = resolve_task_pet_id(&viewer, &form.pet_id);
    let Some(target) = sharing::resolve_pet_care_target(
        &state,
        &viewer,
        &pet_id,
        task_owner_hint(&viewer, &form.pet_owner),
    ) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_invalid").into_response()
        };
    };

    let task_pet_id = if target.is_shared {
        target.pet_id.clone()
    } else {
        pet_id.clone()
    };

    if target.is_shared {
        let Some(mut owner) = sharing::load_profile_by_email(&state, &target.owner_email) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_invalid").into_response()
            };
        };

        let Some(index) = find_task_index(&owner, task_id, &task_pet_id) else {
            return if wants_json {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "ok": false, "status": "invalid" })),
                )
                    .into_response()
            } else {
                Redirect::to("/home?tab=tasks&status=task_invalid").into_response()
            };
        };

        if owner.tasks[index].completed {
            let (title, reward) = reopen_completed_task(&mut owner, index);
            viewer.paw_points = viewer.paw_points.saturating_sub(reward);
            push_activity(
                &mut viewer,
                &format!("Reopened task: {title}. −{reward} paw points."),
            );
            return match (
                save_profile(&state, &owner).await,
                save_profile(&state, &viewer).await,
            ) {
                (Ok(()), Ok(())) if wants_json => {
                    task_dashboard_json_response(&state, &viewer, "reopened", false, None)
                }
                (Ok(()), Ok(())) => Redirect::to("/home?tab=tasks&status=task_reopened").into_response(),
                _ if wants_json => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "ok": false, "status": "error" })),
                )
                    .into_response(),
                _ => Redirect::to("/home?tab=tasks&status=task_invalid").into_response(),
            };
        }

        let reward = owner.tasks[index].reward;
        let title = owner.tasks[index].title.clone();
        owner.tasks[index].completed = true;
        let today = Local::now().date_naive();
        if is_daily_reset_task(&owner.tasks[index]) {
            owner.tasks[index].due_day = Some(today.day());
            owner.tasks[index].due_month = Some(today.month());
            owner.tasks[index].due_year = Some(today.year() as u32);
        }

        viewer.paw_points += reward;
        let mut level_up = None;
        viewer.parent_xp += reward / 2;
        if viewer.parent_xp >= 100 {
            viewer.parent_xp -= 100;
            viewer.parent_level += 1;
            let new_level = viewer.parent_level;
            level_up = Some(new_level);
            push_activity(
                &mut viewer,
                &format!("Leveled up to Parent Level {new_level}!"),
            );
        }
        push_activity(
            &mut viewer,
            &format!("Completed \"{title}\" and earned {reward} paw points."),
        );

        let streak_milestone = if share_cards::is_care_streak_task(task_id) {
            share_cards::update_care_streak(&mut viewer, today)
        } else {
            None
        };
        if let Some(days) = streak_milestone {
            push_activity(
                &mut viewer,
                &format!("{days}-day care streak! Keep it going."),
            );
        }

        let share_card = share_cards::share_offer_for_task_completion(
            &viewer,
            level_up,
            streak_milestone,
            &stripe_payments::public_app_url(),
            timestamp_now(),
        );

        let is_vet_task = task_id == VET_APPOINTMENT_TASK_ID;
        if is_vet_task {
            owner.vet_followup_pending = true;
        }

        let show_vet_followup = is_vet_task;
        return match (
            save_profile(&state, &owner).await,
            save_profile(&state, &viewer).await,
        ) {
            (Ok(()), Ok(())) if wants_json => task_dashboard_json_response(
                &state,
                &viewer,
                "completed",
                show_vet_followup,
                share_card,
            ),
            (Ok(()), Ok(())) if is_vet_task => {
                Redirect::to("/home?tab=tasks&vet_followup=1&status=task_done").into_response()
            }
            (Ok(()), Ok(())) => Redirect::to("/home?tab=tasks&status=task_done").into_response(),
            _ if wants_json => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "status": "error" })),
            )
                .into_response(),
            _ => Redirect::to("/home?tab=tasks&status=task_invalid").into_response(),
        };
    }

    let Some(index) = find_task_index(&viewer, task_id, &task_pet_id) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=tasks&status=task_invalid").into_response()
        };
    };

    if viewer.tasks[index].completed {
        let (title, reward) = reopen_completed_task(&mut viewer, index);
        push_activity(
            &mut viewer,
            &format!("Reopened task: {title}. −{reward} paw points."),
        );
        return match save_profile(&state, &viewer).await {
            Ok(()) if wants_json => {
                task_dashboard_json_response(&state, &viewer, "reopened", false, None)
            }
            Ok(()) => Redirect::to("/home?tab=tasks&status=task_reopened").into_response(),
            Err(_) if wants_json => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "status": "error" })),
            )
                .into_response(),
            Err(_) => Redirect::to("/home?tab=tasks&status=task_invalid").into_response(),
        };
    }

    let reward = viewer.tasks[index].reward;
    let title = viewer.tasks[index].title.clone();
    viewer.tasks[index].completed = true;
    let today = Local::now().date_naive();
    if is_daily_reset_task(&viewer.tasks[index]) {
        viewer.tasks[index].due_day = Some(today.day());
        viewer.tasks[index].due_month = Some(today.month());
        viewer.tasks[index].due_year = Some(today.year() as u32);
    }

    viewer.paw_points += reward;
    let mut level_up = None;
    viewer.parent_xp += reward / 2;
    if viewer.parent_xp >= 100 {
        viewer.parent_xp -= 100;
        viewer.parent_level += 1;
        let new_level = viewer.parent_level;
        level_up = Some(new_level);
        push_activity(
            &mut viewer,
            &format!("Leveled up to Parent Level {new_level}!"),
        );
    }
    push_activity(
        &mut viewer,
        &format!("Completed \"{title}\" and earned {reward} paw points."),
    );

    let streak_milestone = if share_cards::is_care_streak_task(task_id) {
        share_cards::update_care_streak(&mut viewer, today)
    } else {
        None
    };
    if let Some(days) = streak_milestone {
        push_activity(
            &mut viewer,
            &format!("{days}-day care streak! Keep it going."),
        );
    }

    let share_card = share_cards::share_offer_for_task_completion(
        &viewer,
        level_up,
        streak_milestone,
        &stripe_payments::public_app_url(),
        timestamp_now(),
    );

    let is_vet_task = task_id == VET_APPOINTMENT_TASK_ID;
    if is_vet_task {
        viewer.vet_followup_pending = true;
    }

    let show_vet_followup = is_vet_task && profile_has_pet(&viewer);
    let save_result = save_profile(&state, &viewer).await;

    match save_result {
        Ok(()) if wants_json => task_dashboard_json_response(
            &state,
            &viewer,
            "completed",
            show_vet_followup,
            share_card,
        ),
        Ok(()) if is_vet_task => {
            Redirect::to("/home?tab=tasks&vet_followup=1&status=task_done").into_response()
        }
        Ok(()) => Redirect::to("/home?tab=tasks&status=task_done").into_response(),
        Err(_) if wants_json => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "status": "error" })),
        )
            .into_response(),
        Err(_) => Redirect::to("/home?tab=tasks&status=task_invalid").into_response(),
    }
}

fn friend_request_error_status(error: &storage::StorageError) -> &'static str {
    match error {
        storage::StorageError::InvalidInput(message) if message.contains("user not found") => {
            "friend_not_found"
        }
        storage::StorageError::InvalidInput(message) if message.contains("already friends") => {
            "friend_already"
        }
        storage::StorageError::InvalidInput(message)
            if message.contains("request already pending") =>
        {
            "friend_pending"
        }
        _ => "friend_request_invalid",
    }
}

async fn friend_request_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<sharing::FriendRequestForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let friend_email = match sharing::resolve_friend_identifier(&state, &form.friend_email) {
        Ok(value) => value,
        Err(sharing::FriendLookupError::Empty) => {
            return Redirect::to("/home?tab=friends&status=friend_request_invalid");
        }
        Err(sharing::FriendLookupError::NotFound) => {
            return Redirect::to("/home?tab=friends&status=friend_not_found");
        }
    };

    if friend_email == sharing::normalize_email(&email) {
        return Redirect::to("/home?tab=friends&status=friend_self");
    }

    match state
        .storage
        .create_friend_request(&email, &friend_email, timestamp_now())
    {
        Ok(()) => Redirect::to("/home?tab=friends&status=friend_request_sent"),
        Err(error) => Redirect::to(&format!(
            "/home?tab=friends&status={}",
            friend_request_error_status(&error)
        )),
    }
}

async fn friend_respond_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<sharing::FriendRespondForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let accept = form.action.trim().eq_ignore_ascii_case("accept");
    match state
        .storage
        .respond_friend_request(form.request_id.trim(), &email, accept)
    {
        Ok(()) if accept => Redirect::to("/home?tab=friends&status=friend_accepted"),
        Ok(()) => Redirect::to("/home?tab=friends&status=friend_declined"),
        Err(_) => Redirect::to("/home?tab=friends&status=friend_request_invalid"),
    }
}

fn pet_share_error_status(error: &storage::StorageError) -> &'static str {
    match error {
        storage::StorageError::InvalidInput(message) if message.contains("not friends") => {
            "share_not_friends"
        }
        storage::StorageError::InvalidInput(message) if message.contains("already shared") => {
            "share_already"
        }
        _ => "share_invalid",
    }
}

async fn pet_share_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<sharing::PetShareForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let profile = get_or_create_profile(&state, &email).await;
    let friend_email = sharing::normalize_email(&form.friend_email);
    let pet_id = form.pet_id.trim();
    if friend_email.is_empty() || !sharing::owner_has_pet(&profile, pet_id) {
        return Redirect::to("/home?tab=friends&status=share_invalid");
    }

    match state
        .storage
        .create_pet_share(&email, &friend_email, pet_id, timestamp_now())
    {
        Ok(()) => Redirect::to("/home?tab=friends&status=share_sent"),
        Err(error) => Redirect::to(&format!(
            "/home?tab=friends&status={}",
            pet_share_error_status(&error)
        )),
    }
}

async fn pet_share_revoke_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<sharing::PetShareRevokeForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    match state
        .storage
        .revoke_pet_share(form.share_id.trim(), &email)
    {
        Ok(()) => Redirect::to("/home?tab=friends&status=share_revoked"),
        Err(_) => Redirect::to("/home?tab=friends&status=share_invalid"),
    }
}

async fn pet_share_respond_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<sharing::PetShareRespondForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let accept = form.action.trim().eq_ignore_ascii_case("accept");
    match state
        .storage
        .respond_pet_share(form.share_id.trim(), &email, accept)
    {
        Ok(()) if accept => Redirect::to("/home?tab=friends&status=share_accepted"),
        Ok(()) => Redirect::to("/home?tab=friends&status=share_declined"),
        Err(_) => Redirect::to("/home?tab=friends&status=share_invalid"),
    }
}

fn calendar_event_add_json_response(
    state: &AppState,
    profile: &UserProfile,
    status: &'static str,
) -> Response {
    let calendar_month = current_calendar_month();
    let calendar_year = current_calendar_year();
    let calendar_profile = sharing::calendar_view_profile(state, profile);
    let calendar_data = serde_json::from_str(&render_calendar_data_json(
        state,
        profile,
        &calendar_profile,
        calendar_month,
        calendar_year,
    ))
    .unwrap_or_else(|_| serde_json::json!({}));

    Json(CalendarEventAddResponse {
        ok: true,
        status,
        calendar_data,
    })
    .into_response()
}

fn parse_time_minutes_field(value: &str) -> Option<u16> {
    let minutes: u32 = value.trim().parse().ok()?;
    if minutes < 360 || minutes > 1320 || minutes % 15 != 0 {
        return None;
    }
    Some(minutes as u16)
}

fn calendar_date_heading(day: u32, month: u32, year: u32) -> String {
    let month_name = MONTH_NAMES
        .get(month.saturating_sub(1) as usize)
        .unwrap_or(&"Month");
    format!("{month_name} {day}, {year}")
}

async fn calendar_event_form_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<CalendarEventFormQuery>,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let day = query.day.as_deref().unwrap_or("");
    let month = query.month.as_deref().unwrap_or("");
    let year = query.year.as_deref().unwrap_or("");
    let Some((day, month, year)) = parse_calendar_date_fields(day, month, year) else {
        return Redirect::to("/home?tab=calendar&status=calendar_event_invalid").into_response();
    };

    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let date_label = calendar_date_heading(day, month, year);
    let back_url = format!("/home?tab=calendar&cal_day={day}&cal_month={month}&cal_year={year}");

    match fs::read_to_string("templates/calendar-event-form.html").await {
        Ok(template) => {
            let html = template
                .replace("{{USER_NAME}}", &escape_html(&username))
                .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar))
                .replace("{{DATE_LABEL}}", &escape_html(&date_label))
                .replace("{{EVENT_DAY}}", &day.to_string())
                .replace("{{EVENT_MONTH}}", &month.to_string())
                .replace("{{EVENT_YEAR}}", &year.to_string())
                .replace("{{BACK_URL}}", &escape_html(&back_url));
            (jar, Html(html)).into_response()
        }
        Err(_) => (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load calendar event form",
            ),
        )
            .into_response(),
    }
}

async fn calendar_event_add(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<CalendarEventAddForm>,
) -> Response {
    let wants_json = wants_json_response(&headers);
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let title = form.title.trim();
    if title.is_empty() {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "missing" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=calendar&status=calendar_event_missing").into_response()
        };
    }

    let Some((day, month, year)) = parse_calendar_date_fields(&form.day, &form.month, &form.year)
    else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=calendar&status=calendar_event_invalid").into_response()
        };
    };

    let Some(time_minutes) = parse_time_minutes_field(&form.time_minutes) else {
        return if wants_json {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "status": "invalid" })),
            )
                .into_response()
        } else {
            Redirect::to("/home?tab=calendar&status=calendar_event_invalid").into_response()
        };
    };
    let time_label = format_time_from_minutes(time_minutes);

    let mut profile = get_or_create_profile(&state, &email).await;
    profile.user_calendar_events.push(CalendarEvent {
        id: Some(Uuid::new_v4().to_string()),
        day,
        month,
        year,
        title: title.to_string(),
        time_label,
        time_minutes,
    });
    push_activity(&mut profile, &format!("Added \"{title}\" to the calendar."));

    match save_profile(&state, &profile).await {
        Ok(()) if wants_json => calendar_event_add_json_response(&state, &profile, "added"),
        Ok(()) => {
            Redirect::to(&format!(
                "/home?tab=calendar&cal_day={day}&cal_month={month}&cal_year={year}&status=calendar_event_added"
            ))
            .into_response()
        }
        Err(_) if wants_json => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "status": "error" })),
        )
            .into_response(),
        Err(_) => Redirect::to("/home?tab=calendar&status=calendar_event_failed").into_response(),
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
    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=health&status=vet_visit_invalid");
    }
    if !entitlements::can_access_health_records(profile.premium_unlocked, &profile.email) {
        return Redirect::to("/home?tab=health&status=premium_required");
    }

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
    if !profile_has_pet(&profile) {
        return Redirect::to("/home?tab=health&status=vet_notes_invalid");
    }
    if !entitlements::can_access_health_records(profile.premium_unlocked, &profile.email) {
        return Redirect::to("/home?tab=health&status=premium_required");
    }

    let trimmed = form.vet_notes.trim();
    profile.vet_notes = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };

    let pet_name = profile.pet_name.clone();
    push_activity(&mut profile, &format!("Updated vet notes for {pet_name}."));

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=health&status=vet_notes_done"),
        Err(_) => Redirect::to("/home?tab=health&status=vet_notes_invalid"),
    }
}

fn symptom_context_from_profile(profile: &UserProfile) -> symptom_checker::PetContext {
    symptom_checker::PetContext {
        name: display_pet_name(profile),
        breed: profile.pet_breed.clone(),
        age: age_display(profile),
        conditions: if profile.pet_conditions.trim().is_empty() {
            "None noted".to_string()
        } else {
            profile.pet_conditions.clone()
        },
        lifestyle: indoor_outdoor_display(profile.pet_indoor_outdoor.as_deref()),
    }
}

async fn symptom_check_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<SymptomCheckForm>,
) -> impl IntoResponse {
    let wants_json = wants_json_response(&headers);
    let email = match user_session_email(&state, &jar) {
        Some(email) => email,
        None => return api_auth_error(wants_json),
    };

    let profile = get_or_create_profile(&state, &email).await;
    if !profile_has_pet(&profile) {
        if wants_json {
            return (
                StatusCode::BAD_REQUEST,
                Json(SymptomCheckResponse {
                    ok: false,
                    status: Some("no_pet"),
                    analysis: None,
                }),
            )
                .into_response();
        }
        return Redirect::to("/home?tab=health&status=symptom_check_invalid").into_response();
    }

    let context = symptom_context_from_profile(&profile);
    let analysis = symptom_checker::analyze_symptoms(
        &form.symptoms,
        &form.quick_symptoms,
        &context,
    );

    if wants_json {
        return Json(SymptomCheckResponse {
            ok: true,
            status: None,
            analysis: Some(analysis),
        })
        .into_response();
    }

    Redirect::to("/home?tab=health&status=symptom_check_done").into_response()
}

async fn shelter_search_submit(
    State(_state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(form): Form<ShelterSearchForm>,
) -> impl IntoResponse {
    let wants_json = wants_json_response(&headers);
    if user_session_email(&_state, &jar).is_none() {
        return api_auth_error(wants_json);
    }

    let result = shelter_locator::search_nearby_shelters(
        &form.shelter_zip,
        &form.shelter_city,
        &form.shelter_state,
    )
    .await;

    if wants_json {
        return Json(result).into_response();
    }

    Redirect::to("/home?tab=health").into_response()
}

async fn paw_points_needed_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<NeedPawPointsQuery>,
) -> impl IntoResponse {
    let (jar, email) = match ensure_dashboard_session(&state, jar) {
        Ok(pair) => pair,
        Err(redirect) => return redirect.into_response(),
    };

    let Some(item) = shop_item_from_query(&query) else {
        return Redirect::to("/home/cat-home").into_response();
    };

    let profile = get_or_create_profile(&state, &email).await;
    let return_url = shop_return_url(query.return_to.as_deref());

    if profile.paw_points >= item.price {
        return Redirect::to(return_url).into_response();
    }

    let user = user_for_email(&state, &email);
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "Parent".to_string());
    let points_needed = item.price.saturating_sub(profile.paw_points);

    match fs::read_to_string("templates/need-paw-points.html").await {
        Ok(template) => {
            let html = template
                .replace("{{USER_NAME}}", &escape_html(&username))
                .replace("{{ADMIN_NAV_LINK}}", admin_dashboard_nav_link(&state, &jar))
                .replace("{{ITEM_NAME}}", &escape_html(item.name))
                .replace("{{ITEM_PRICE}}", &item.price.to_string())
                .replace("{{PAW_POINTS}}", &profile.paw_points.to_string())
                .replace("{{POINTS_NEEDED}}", &points_needed.to_string())
                .replace(
                    "{{BUY_POINTS_SECTION}}",
                    &stripe_payments::render_buy_points_section(),
                )
                .replace("{{RETURN_URL}}", return_url);
            (jar, Html(html)).into_response()
        }
        Err(_) => (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load paw points page",
            ),
        )
            .into_response(),
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

    let signature = match headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
    {
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
                    r#"<p class="auth-error status-flash" role="alert">Incorrect password for the admin account. Use the <code>ADMIN_PASSWORD</code> from your server environment (Render → Environment tab in production). Locally, the default is <code>WhiskerAdmin2026!</code> unless you set <code>ADMIN_PASSWORD</code>.</p>"#
                }
                Some("invalid") => {
                    r#"<p class="auth-error status-flash" role="alert">Incorrect password. Please try again.</p>"#
                }
                Some("missing") => {
                    r#"<p class="auth-error status-flash" role="alert">Please enter both email and password.</p>"#
                }
                Some("storage") => {
                    r#"<p class="auth-error status-flash" role="alert">We could not verify your account right now. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let signup_success_block = match query.signup.as_deref() {
                Some("created") => {
                    r#"<p class="auth-success status-flash" role="status">Account created! You can log in with your new email and password.</p>"#
                }
                _ => "",
            };
            let reset_success_block = match query.reset.as_deref() {
                Some("success") => {
                    r#"<p class="auth-success status-flash" role="status">Your password was updated. You can log in with your new password.</p>"#
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
                r#"<p class="auth-success status-flash" role="status">An account with this email already exists. Log in below.</p>"#
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
                    r#"<p class="auth-error status-flash" role="alert">Please enter your email address.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error status-flash" role="alert">We could not process your request right now. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let forgot_success_block = match query.sent.as_deref() {
                Some("1") => {
                    r#"<p class="auth-success status-flash" role="status">If an account exists for that email, password reset instructions have been sent.</p>"#
                }
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
                    r#"<p class="auth-success status-flash" role="status">If an account exists for that email, password reset instructions have been sent.</p>"#,
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
                    r#"<p class="auth-error status-flash" role="alert">Please enter and confirm your new password.</p>"#
                }
                Some("password") => {
                    r#"<p class="auth-error status-flash" role="alert">Password must be at least 5 characters and include a number and a special character.</p>"#
                }
                Some("password_mismatch") => {
                    r#"<p class="auth-error status-flash" role="alert">Passwords do not match. Please re-enter your password and try again.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error status-flash" role="alert">This reset link is invalid or has expired. Please request a new one.</p>"#
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
                    r#"<p class="auth-error status-flash" role="alert">Please fill out all sign up fields.</p>"#
                }
                Some("email_exists") | Some("exists") => {
                    r#"<p class="auth-error status-flash" role="alert">An account with that email already exists. <a href="/login">Log in</a> instead.</p>"#
                }
                Some("username") => {
                    r#"<p class="auth-error status-flash" role="alert">That username is already taken. Please choose another.</p>"#
                }
                Some("password") => {
                    r#"<p class="auth-error status-flash" role="alert">Password must be at least 5 characters and include a number and a special character.</p>"#
                }
                Some("password_mismatch") => {
                    r#"<p class="auth-error status-flash" role="alert">Passwords do not match. Please re-enter your password and try again.</p>"#
                }
                Some("no_account") => {
                    r#"<p class="auth-error status-flash" role="alert">You don't have an account yet. Create one below.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error status-flash" role="alert">We could not create your account. Please try again.</p>"#
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
                    r#"<p class="auth-success status-flash" role="status">Thanks! Your message was received. We will get back to you soon.</p>"#
                }
                _ => "",
            };
            let contact_error_block = match query.status.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error status-flash" role="alert">Please fill out all fields before sending your message.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error status-flash" role="alert">We could not save your message. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let contact_email = escape_html(&company_email());
            let body = contents
                .replace("{{CONTACT_SUCCESS_BLOCK}}", contact_success_block)
                .replace("{{CONTACT_ERROR_BLOCK}}", contact_error_block)
                .replace("{{CONTACT_EMAIL}}", &contact_email)
                .replace("{{FORM_NAME}}", &form_name)
                .replace("{{FORM_EMAIL}}", &form_email);
            let body = apply_auth_nav_link(&body, &state, &jar);
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
                    r#"<p class="auth-success status-flash" role="status">Your feedback was posted to the public forum.</p>"#
                }
                _ => "",
            };
            let feedback_error_block = match query.status.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error status-flash" role="alert">Please fill out all feedback fields.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error status-flash" role="alert">We could not save your feedback. Please try again.</p>"#
                }
                _ => "",
            };
            let open_post = query
                .feedback
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok());
            let voter_email = user_session_email(&state, &jar);
            let body = contents
                .replace("{{FEEDBACK_SUCCESS_BLOCK}}", feedback_success_block)
                .replace("{{FEEDBACK_ERROR_BLOCK}}", feedback_error_block)
                .replace(
                    "{{FEEDBACK_FORUM_CONTENT}}",
                    &render_feedback_forum(
                        &state,
                        &form_name,
                        &form_email,
                        open_post,
                        voter_email.as_deref(),
                    ),
                );
            let body = apply_auth_nav_link(&body, &state, &jar);
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

fn save_user(state: &AppState, form: &SignupForm) -> Result<u64, storage::StorageError> {
    let created_at = timestamp_now();
    let user = User {
        username: form.username.trim().to_string(),
        first_name: form.first_name.trim().to_string(),
        last_name: form.last_name.trim().to_string(),
        email: form.email.trim().to_string(),
        password: form.password.trim().to_string(),
        created_at,
    };

    state.storage.save_user(&user)?;
    Ok(created_at)
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
        return signup_redirect_with_fields(
            "password_mismatch",
            username,
            first_name,
            last_name,
            email,
        )
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
        Ok(created_at) => {
            let welcome_state = state.clone();
            let welcome_email = email.to_string();
            let welcome_first_name = first_name.to_string();
            tokio::spawn(async move {
                if let Err(error) = onboarding_emails::try_send_due_for_email(
                    &welcome_state,
                    &welcome_email,
                    &welcome_first_name,
                    created_at,
                )
                .await
                {
                    eprintln!("welcome onboarding email failed for {welcome_email}: {error}");
                }
            });
            signed_in_redirect(&state, jar, email)
        }
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
) -> Result<i64, storage::StorageError> {
    let name = form.name.trim().to_string();
    let author_username = user_id
        .and_then(|email| user_for_email(state, email).map(|user| user.username))
        .filter(|username| !username.is_empty())
        .unwrap_or_else(|| name.clone());

    let submission = FeedbackSubmission {
        id: 0,
        name,
        email: form.email.trim().to_string(),
        category: form.category.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
        user_id: user_id.map(str::to_string),
        author_username,
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
        Ok(post_id) => {
            if from_dashboard {
                Redirect::to(&format!(
                    "/home?tab=feedback&feedback={post_id}&status=feedback_sent"
                ))
            } else {
                Redirect::to(&format!("/feedback?status=sent&feedback={post_id}"))
            }
        }
        Err(error) => {
            eprintln!("feedback save failed: {error}");
            if from_dashboard {
                Redirect::to("/home?tab=feedback&status=feedback_failed")
            } else {
                Redirect::to("/feedback?status=failed")
            }
        }
    }
}

async fn feedback_vote_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<FeedbackVoteForm>,
) -> impl IntoResponse {
    let Some(email) = voter_session_email(&state, &jar) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(FeedbackVoteResponse {
                ok: false,
                error: Some("login_required"),
                feedback_id: 0,
                upvotes: 0,
                downvotes: 0,
                user_vote: None,
            }),
        )
            .into_response();
    };

    let feedback_id = match form.feedback_id.trim().parse::<i64>() {
        Ok(id) if id > 0 => id,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FeedbackVoteResponse {
                    ok: false,
                    error: Some("invalid_feedback"),
                    feedback_id: 0,
                    upvotes: 0,
                    downvotes: 0,
                    user_vote: None,
                }),
            )
                .into_response();
        }
    };

    let vote = match form.vote.trim() {
        "up" => 1i8,
        "down" => -1i8,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FeedbackVoteResponse {
                    ok: false,
                    error: Some("invalid_vote"),
                    feedback_id,
                    upvotes: 0,
                    downvotes: 0,
                    user_vote: None,
                }),
            )
                .into_response();
        }
    };

    if state
        .storage
        .get_feedback_submission(feedback_id)
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::NOT_FOUND,
            Json(FeedbackVoteResponse {
                ok: false,
                error: Some("not_found"),
                feedback_id,
                upvotes: 0,
                downvotes: 0,
                user_vote: None,
            }),
        )
            .into_response();
    }

    match state.storage.cast_feedback_vote(feedback_id, &email, vote) {
        Ok(counts) => {
            maybe_grant_purrfect_idea_reward(&state, feedback_id, counts.upvotes).await;
            (
                StatusCode::OK,
                Json(FeedbackVoteResponse {
                    ok: true,
                    error: None,
                    feedback_id,
                    upvotes: counts.upvotes,
                    downvotes: counts.downvotes,
                    user_vote: counts.user_vote,
                }),
            )
                .into_response()
        }
        Err(error) => {
            eprintln!("feedback vote failed: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FeedbackVoteResponse {
                    ok: false,
                    error: Some("server_error"),
                    feedback_id,
                    upvotes: 0,
                    downvotes: 0,
                    user_vote: None,
                }),
            )
                .into_response()
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
        return Redirect::to("/home?tab=forum&community=forum&status=forum_missing")
            .into_response();
    }

    let profile = get_or_create_profile(&state, &email).await;
    let breed_slug = resolve_forum_breed_slug(&form.breed_slug, &profile);
    let username = user_for_email(&state, &email)
        .map(|user| user.username)
        .unwrap_or_else(|| "Parent".to_string());

    match state.storage.create_forum_post(
        &email,
        &username,
        title,
        body,
        &breed_slug,
        timestamp_now(),
    ) {
        Ok(post_id) => {
            let mut url =
                format!("/home?tab=forum&community=forum&thread={post_id}&status=forum_post_sent");
            if !breed_slug.is_empty() {
                url.push_str(&format!("&breed={}", urlencoding::encode(&breed_slug)));
            }
            Redirect::to(&url).into_response()
        }
        Err(error) => {
            eprintln!("forum post failed for {email}: {error}");
            Redirect::to("/home?tab=forum&community=forum&status=forum_failed").into_response()
        }
    }
}

async fn push_vapid_public_key() -> impl IntoResponse {
    match push_notifications::vapid_public_key() {
        Some(public_key) => Json(serde_json::json!({ "public_key": public_key })).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn push_subscribe(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<push_notifications::PushSubscribeRequest>,
) -> impl IntoResponse {
    let email = match api_user_email(&state, &jar) {
        Some(email) => email,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let endpoint = body.endpoint.trim();
    let p256dh = body.p256dh.trim();
    let auth = body.auth.trim();
    if endpoint.is_empty() || p256dh.is_empty() || auth.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match state
        .storage
        .upsert_push_subscription(&email, endpoint, p256dh, auth, timestamp_now())
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(error) => {
            eprintln!("push subscribe failed for {email}: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
struct PushUnsubscribeRequest {
    endpoint: String,
}

async fn push_unsubscribe(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<PushUnsubscribeRequest>,
) -> impl IntoResponse {
    let email = match api_user_email(&state, &jar) {
        Some(email) => email,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let endpoint = body.endpoint.trim();
    if endpoint.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match state.storage.delete_push_subscription(endpoint) {
        Ok(()) => {
            eprintln!("push unsubscribed for {email}");
            StatusCode::OK.into_response()
        }
        Err(error) => {
            eprintln!("push unsubscribe failed for {email}: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn onboarding_email_prefs_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<onboarding_emails::OnboardingEmailPrefsForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    profile.onboarding_emails_enabled = form.onboarding_emails_enabled == "on";
    let activity_message = if profile.onboarding_emails_enabled {
        "Week-one onboarding emails enabled."
    } else {
        "Week-one onboarding emails turned off."
    };
    push_activity(&mut profile, activity_message);

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account&status=onboarding_emails_saved"),
        Err(_) => Redirect::to("/home?tab=account&status=error"),
    }
}

async fn notification_prefs_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<push_notifications::NotificationPrefsForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    if push_notifications::apply_notification_prefs_form(&mut profile, &form).is_err() {
        return Redirect::to("/home?tab=account&status=notification_prefs_invalid");
    }

    push_activity(&mut profile, "Notification settings updated.");

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account&status=notification_prefs_saved"),
        Err(_) => Redirect::to("/home?tab=account&status=error"),
    }
}

async fn notifications_schedule(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    let email = match api_user_email(&state, &jar) {
        Some(email) => email,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let profile = get_or_create_profile(&state, &email).await;
    Json(push_notifications::NotificationScheduleResponse {
        push_enabled: push_notifications::push_configured(),
        reminders: push_notifications::upcoming_reminders_for_profile(&profile),
    })
    .into_response()
}

async fn community_visibility_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CommunityVisibilityForm>,
) -> impl IntoResponse {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect,
    };

    let mut profile = get_or_create_profile(&state, &email).await;
    let visible = form.community_visible == "on";
    profile.community_visible = visible;
    push_activity(
        &mut profile,
        if visible {
            "Your cat is now visible in the community feed."
        } else {
            "Your cat is hidden from the community feed."
        },
    );

    match save_profile(&state, &profile).await {
        Ok(()) => Redirect::to("/home?tab=account&status=community_visibility_saved"),
        Err(_) => Redirect::to("/home?tab=account&status=error"),
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
        _ => {
            return Redirect::to("/home?tab=forum&community=forum&status=forum_invalid")
                .into_response()
        }
    };

    if body.is_empty() {
        let url =
            format!("/home?tab=forum&community=forum&thread={post_id}&status=forum_reply_missing");
        return Redirect::to(&url).into_response();
    }

    if state
        .storage
        .get_forum_post(post_id)
        .ok()
        .flatten()
        .is_none()
    {
        return Redirect::to("/home?tab=forum&community=forum&status=forum_invalid")
            .into_response();
    }

    let username = user_for_email(&state, &email)
        .map(|user| user.username)
        .unwrap_or_else(|| "Parent".to_string());

    match state
        .storage
        .create_forum_reply(post_id, &email, &username, body, timestamp_now())
    {
        Ok(()) => {
            let url =
                format!("/home?tab=forum&community=forum&thread={post_id}&status=forum_reply_sent");
            Redirect::to(&url).into_response()
        }
        Err(error) => {
            eprintln!("forum reply failed for {email}: {error}");
            let url =
                format!("/home?tab=forum&community=forum&thread={post_id}&status=forum_failed");
            Redirect::to(&url).into_response()
        }
    }
}

async fn forum_thread_redirect(Path(post_id): Path<i64>) -> Response {
    let url = format!("/home?tab=forum&community=forum&thread={post_id}");
    Redirect::temporary(&url).into_response()
}

fn forum_delete_redirect(post_id: Option<i64>, status: &str) -> Response {
    let mut url = format!("/home?tab=forum&community=forum&status={status}");
    if let Some(post_id) = post_id {
        url.push_str(&format!("&thread={post_id}"));
    }
    Redirect::to(&url).into_response()
}

async fn forum_post_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ForumDeletePostForm>,
) -> Response {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let post_id: i64 = match form.post_id.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            return Redirect::to("/home?tab=forum&community=forum&status=forum_invalid")
                .into_response()
        }
    };

    match state.storage.delete_forum_post_owned(post_id, &email) {
        Ok(ForumDeleteOutcome::Deleted) => forum_delete_redirect(None, "forum_post_deleted"),
        Ok(ForumDeleteOutcome::NotFound) => {
            Redirect::to("/home?tab=forum&community=forum&status=forum_invalid").into_response()
        }
        Ok(ForumDeleteOutcome::NotAuthorized) => {
            forum_delete_redirect(Some(post_id), "forum_delete_denied")
        }
        Err(error) => {
            eprintln!("forum post delete failed for {email}: {error}");
            forum_delete_redirect(Some(post_id), "forum_delete_failed")
        }
    }
}

async fn forum_reply_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ForumDeleteReplyForm>,
) -> Response {
    let email = match user_redirect_if_missing(&state, &jar) {
        Ok(email) => email,
        Err(redirect) => return redirect.into_response(),
    };

    let reply_id: i64 = match form.reply_id.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            return Redirect::to("/home?tab=forum&community=forum&status=forum_invalid")
                .into_response()
        }
    };
    let post_id: i64 = match form.post_id.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            return Redirect::to("/home?tab=forum&community=forum&status=forum_invalid")
                .into_response()
        }
    };

    match state.storage.delete_forum_reply_owned(reply_id, &email) {
        Ok(ForumDeleteOutcome::Deleted) => {
            forum_delete_redirect(Some(post_id), "forum_reply_deleted")
        }
        Ok(ForumDeleteOutcome::NotFound) => {
            Redirect::to("/home?tab=forum&community=forum&status=forum_invalid").into_response()
        }
        Ok(ForumDeleteOutcome::NotAuthorized) => {
            forum_delete_redirect(Some(post_id), "forum_delete_denied")
        }
        Err(error) => {
            eprintln!("forum reply delete failed for {email}: {error}");
            forum_delete_redirect(Some(post_id), "forum_delete_failed")
        }
    }
}

fn render_submission_rows(rows: &[(&str, &str, &str, &str, u64)], empty_message: &str) -> String {
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
        <a href="/home?tab=pet">HOME</a>
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
    <script src="/paw-cursor.js"></script>
  </body>
</html>"#,
        feedback_count = feedback.len(),
        contact_count = contacts.len(),
        feedback_rows = render_feedback_rows(&feedback, "No feedback submissions yet."),
        contact_rows = render_submission_rows(&contact_rows, "No contact messages yet."),
    );

    Html(body).into_response()
}

async fn admin_logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = clear_admin_session(&state, jar);
    (jar, Redirect::to("/")).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile_weeks_premium(weeks: u32, indoor: &str) -> UserProfile {
        let mut profile = test_profile_weeks(weeks, indoor);
        profile.premium_unlocked = true;
        profile
    }

    fn test_profile_weeks(weeks: u32, indoor: &str) -> UserProfile {
        UserProfile {
            email: "test@example.com".to_string(),
            paw_points: 0,
            parent_level: 1,
            parent_xp: 0,
            pet_name: "Mochi".to_string(),
            pet_breed: "Domestic Shorthair".to_string(),
            pet_color: String::new(),
            pet_mood: String::new(),
            pet_emoji: "🐱".to_string(),
            equipped_outfit: String::new(),
            owned_outfits: vec![],
            onboarding_completed: true,
            pet_age_weeks: Some(weeks),
            pet_age_years: None,
            pet_birth_date: None,
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
            care_schedule: default_care_schedule(),
            tasks: vec![],
            calendar_events: vec![],
            user_calendar_events: vec![],
            activity: vec![],
            stripe_customer_id: None,
            pet_photo_url: None,
            pet_video_url: None,
            pet_video_clip_start: None,
            pet_video_clip_duration: None,
            pet_video_zoom: None,
            pet_video_offset_x: None,
            pet_video_offset_y: None,
            deceased: false,
            deceased_at: None,
            memorial_videos: Vec::new(),
            memorial_comfort_seen: false,
            pending_purrfect_idea_ids: vec![],
            owned_decor: default_owned_decor(),
            equipped_decor: default_equipped_decor(),
            owned_tap_boosts: default_owned_tap_boosts(),
            equipped_tap_boost: default_equipped_tap_boost(),
            petting_bonus_inventory: HashMap::new(),
            active_petting_bonus: String::new(),
            petting_bonus_expires_at: 0,
            owned_breed_guides: Vec::new(),
            premium_unlocked: false,
            additional_pets: Vec::new(),
            active_pet_id: PRIMARY_PET_ID.to_string(),
            active_pet_owner_email: None,
            care_streak_days: 0,
            care_streak_last_date: None,
            best_care_streak: 0,
            claimed_streak_rewards: Vec::new(),
            community_visible: true,
            notification_prefs: push_notifications::NotificationPrefs::default(),
            notification_sent_dates: HashMap::new(),
            onboarding_emails_enabled: true,
            onboarding_emails_sent: Vec::new(),
        }
    }

    #[test]
    fn memorial_pet_gets_comfort_tasks_and_skips_daily_care() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.premium_unlocked = true;
        profile.additional_pets.push(HouseholdPet {
            id: "pet_luna".to_string(),
            pet_name: "Luna".to_string(),
            pet_breed: "Siamese".to_string(),
            pet_color: String::new(),
            pet_mood: "Happy".to_string(),
            pet_age_weeks: None,
            pet_age_years: Some(2),
            pet_birth_date: Some("2024-06-01".to_string()),
            last_vet_date: None,
            never_been_to_vet: false,
            pet_conditions: String::new(),
            pet_medications: String::new(),
            pet_indoor_outdoor: Some("indoor".to_string()),
            vaccine_history: vec![],
            pet_vaccines_unknown: false,
            care_schedule: default_care_schedule(),
            pet_photo_url: None,
            pet_video_url: None,
            pet_video_clip_start: None,
            pet_video_clip_duration: None,
            pet_video_zoom: None,
            pet_video_offset_x: None,
            pet_video_offset_y: None,
            deceased: false,
            deceased_at: None,
            memorial_videos: Vec::new(),
            memorial_comfort_seen: false,
        });
        assert!(refresh_profile_tasks(&mut profile));
        assert!(profile
            .tasks
            .iter()
            .any(|task| task.id == "feed_breakfast" && task.pet_id == "pet_luna"));

        assert!(memorial::memorialize_pet(&mut profile, "pet_luna"));
        assert!(refresh_profile_tasks(&mut profile));

        assert!(!profile.tasks.iter().any(|task| {
            task.pet_id == "pet_luna" && task.id == "feed_breakfast"
        }));
        assert!(profile.tasks.iter().any(|task| {
            task.pet_id == "pet_luna" && task.id == memorial::MEMORIAL_SELF_HUG_TASK_ID
        }));
        assert!(profile.tasks.iter().any(|task| {
            task.pet_id == "pet_luna" && task.id == memorial::MEMORIAL_PET_FOR_ANGEL_TASK_ID
        }));
    }

    #[test]
    fn parse_time_input_accepts_hh_mm() {
        assert_eq!(parse_time_input("08:30"), Some(510));
        assert_eq!(parse_time_input("21:00"), Some(1260));
        assert!(parse_time_input("25:00").is_none());
        assert!(parse_time_input("bad").is_none());
    }

    #[test]
    fn feeding_plan_matches_cat_age() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).expect("date");

        let mut kitten = test_profile_weeks(12, "indoor");
        kitten.pet_birth_date = Some("2026-03-01".to_string());
        assert_eq!(
            feeding_plan_for_profile(&kitten, today),
            FeedingPlan::FourMeals
        );

        let mut juvenile = test_profile_weeks(40, "indoor");
        juvenile.pet_birth_date = Some("2025-09-01".to_string());
        assert_eq!(
            feeding_plan_for_profile(&juvenile, today),
            FeedingPlan::ThreeMeals
        );

        let mut adult = test_profile_weeks(52, "indoor");
        adult.pet_age_weeks = None;
        adult.pet_age_years = Some(4);
        adult.pet_birth_date = Some("2022-06-05".to_string());
        assert_eq!(
            feeding_plan_for_profile(&adult, today),
            FeedingPlan::TwoMeals
        );

        let mut senior = test_profile_weeks(52, "indoor");
        senior.pet_age_weeks = None;
        senior.pet_age_years = Some(9);
        senior.pet_birth_date = Some("2017-06-05".to_string());
        assert_eq!(
            feeding_plan_for_profile(&senior, today),
            FeedingPlan::TwoMeals
        );
    }

    #[test]
    fn owned_breed_guide_adds_matching_pet_tasks_and_calendar_events() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Persian".to_string();
        profile.owned_breed_guides.push("persian".to_string());

        assert!(ensure_breed_guide_tasks(&mut profile));
        assert!(
            profile
                .tasks
                .iter()
                .any(|task| task.id.starts_with("breed_guide_persian_") && task.pet_id == PRIMARY_PET_ID)
        );

        let today = Local::now().date_naive();
        let horizon = today + Duration::days(400);
        let events = generate_breed_guide_calendar_events(&profile, today, horizon);
        assert!(events
            .iter()
            .any(|event| event.title.contains("Persian breed wellness exam")));
    }

    #[test]
    fn breed_guide_tasks_skip_when_breed_does_not_match() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Siamese".to_string();
        profile.owned_breed_guides.push("persian".to_string());

        assert!(!ensure_breed_guide_tasks(&mut profile));
        assert!(!profile
            .tasks
            .iter()
            .any(|task| task.id.starts_with("breed_guide_")));
    }

    #[test]
    fn starter_tasks_follow_age_based_feeding_schedule() {
        let mut kitten = test_profile_weeks(10, "indoor");
        kitten.pet_birth_date = Some("2026-04-01".to_string());
        let kitten_snapshot = PetSnapshot::from_primary(&kitten);
        let kitten_tasks =
            default_starter_tasks(&kitten_snapshot, &default_care_schedule());
        assert_eq!(
            kitten_tasks
                .iter()
                .filter(|task| FEEDING_TASK_IDS.contains(&task.id.as_str()))
                .count(),
            4
        );

        let mut adult = default_profile("user@example.com");
        adult.pet_name = "Mochi".to_string();
        adult.pet_breed = "Domestic Shorthair".to_string();
        adult.pet_age_years = Some(3);
        adult.pet_birth_date = Some("2023-06-05".to_string());
        adult.pet_indoor_outdoor = Some("indoor".to_string());
        let adult_snapshot = PetSnapshot::from_primary(&adult);
        let adult_tasks = default_starter_tasks(&adult_snapshot, &default_care_schedule());
        let feed_ids: Vec<_> = adult_tasks
            .iter()
            .filter(|task| FEEDING_TASK_IDS.contains(&task.id.as_str()))
            .map(|task| task.id.as_str())
            .collect();
        assert_eq!(feed_ids, vec!["feed_breakfast", "feed_dinner"]);
    }

    #[test]
    fn care_schedule_updates_task_labels() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.care_schedule.feed_time_minutes = 7 * 60 + 15;
        profile.care_schedule.water_evening_time_minutes = 20 * 60;
        assert!(refresh_profile_tasks(&mut profile));

        let feed = profile
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed task");
        assert_eq!(feed.due_label, "Daily · 7:15 AM");
        assert_eq!(feed.time_minutes, 435);

        let water_night = profile
            .tasks
            .iter()
            .find(|task| task.id == "water_bowl_night")
            .expect("evening water");
        assert_eq!(water_night.due_label, "Daily · 8:00 PM");
    }

    #[test]
    fn daily_care_events_repeat_on_calendar() {
        let profile = test_profile_weeks(52, "indoor");
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).expect("date");
        let tomorrow = today.succ_opt().expect("tomorrow");
        let events = generate_daily_care_calendar_events(&profile, today, tomorrow);

        assert_eq!(events.len(), 14);
        assert!(
            events
                .iter()
                .filter(|event| event.day == today.day() && event.month == today.month())
                .count()
                == 7
        );
        assert!(events
            .iter()
            .any(|event| event.title.contains("Feed Mochi")));
        assert!(events
            .iter()
            .any(|event| event.title.contains("Refresh litter box")));
    }

    #[test]
    fn task_list_renders_clickable_times_for_scheduled_tasks() {
        let mut profile = test_profile_weeks(52, "indoor");
        assert!(refresh_profile_tasks(&mut profile));
        let html = render_task_list(&profile);
        assert!(html.contains("task-time-btn"));
        assert!(html.contains(r#"data-task-id="feed_breakfast""#));
        assert!(html.contains(r#"data-time-minutes="480""#));
        assert!(html.contains("Weekly · anytime"));
        assert!(!html.contains(r#"data-task-id="replace_litter""#));
    }

    #[test]
    fn apply_task_time_updates_task_schedule_and_care_schedule() {
        let mut profile = test_profile_weeks(52, "indoor");
        assert!(refresh_profile_tasks(&mut profile));
        assert!(apply_task_time_to_profile(
            &mut profile,
            "feed_breakfast",
            7 * 60 + 45
        ));

        let feed = profile
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        assert_eq!(feed.due_label, "Daily · 7:45 AM");
        assert_eq!(profile.care_schedule.feed_time_minutes, 7 * 60 + 45);
        assert!(!apply_task_time_to_profile(
            &mut profile,
            "replace_litter",
            600
        ));
    }

    #[test]
    fn custom_tasks_render_with_delete_and_ten_point_reward() {
        let mut profile = test_profile_weeks(52, "indoor");
        assert!(refresh_profile_tasks(&mut profile));
        let today = Local::now().date_naive();
        profile.tasks.push(create_custom_task(
            &profile,
            PRIMARY_PET_ID,
            "Brush teeth".to_string(),
            today,
        ));
        sort_tasks_by_time(&mut profile.tasks);

        let html = render_task_list(&profile);
        assert!(html.contains("Brush teeth"));
        assert!(html.contains("task-delete-btn"));
        assert!(html.contains("+10 pts"));

        let custom_id = profile
            .tasks
            .iter()
            .find(|task| is_custom_task(&task.id))
            .expect("custom task")
            .id
            .clone();
        let removed =
            remove_custom_task(&mut profile, &custom_id, PRIMARY_PET_ID).expect("removed");
        assert_eq!(removed.title, "Brush teeth");
        assert!(profile
            .tasks
            .iter()
            .all(|task| !is_custom_task(&task.id) || task.id != custom_id));
    }

    #[test]
    fn care_tasks_are_sorted_earliest_to_latest() {
        let mut profile = test_profile_weeks(52, "indoor");
        assert!(refresh_profile_tasks(&mut profile));

        let times: Vec<u16> = profile.tasks.iter().map(|task| task.time_minutes).collect();
        assert!(
            times.windows(2).all(|pair| pair[0] <= pair[1]),
            "expected tasks in time order, got {times:?}"
        );

        assert!(apply_task_time_to_profile(
            &mut profile,
            "play_session",
            5 * 60
        ));
        let play_index = profile
            .tasks
            .iter()
            .position(|task| task.id == "play_session")
            .expect("play task");
        assert_eq!(play_index, 0, "early play time should move task to the top");
    }

    #[test]
    fn user_calendar_events_merge_with_generated_events() {
        let mut profile = test_profile_weeks(12, "indoor");
        profile.user_calendar_events.push(CalendarEvent {
            id: Some("user-event-1".to_string()),
            day: 15,
            month: 6,
            year: 2026,
            title: "Brush coat".to_string(),
            time_label: "9:00 AM".to_string(),
            time_minutes: 540,
        });

        let events =
            visible_calendar_events(&profile, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        assert!(events.iter().any(|event| event.title == "Brush coat"));
        assert!(events.iter().any(|event| event.id.is_none()));
    }

    #[test]
    fn user_calendar_events_sort_by_time_on_same_day() {
        let mut profile = test_profile_weeks(12, "indoor");
        profile.user_calendar_events = vec![
            CalendarEvent {
                id: Some("late".to_string()),
                day: 10,
                month: 6,
                year: 2026,
                title: "Evening meds".to_string(),
                time_label: "8:00 PM".to_string(),
                time_minutes: 1200,
            },
            CalendarEvent {
                id: Some("early".to_string()),
                day: 10,
                month: 6,
                year: 2026,
                title: "Morning brush".to_string(),
                time_label: "7:00 AM".to_string(),
                time_minutes: 420,
            },
            CalendarEvent {
                id: Some("mid".to_string()),
                day: 10,
                month: 6,
                year: 2026,
                title: "Lunch water".to_string(),
                time_label: "12:00 PM".to_string(),
                time_minutes: 720,
            },
        ];

        let events: Vec<_> =
            visible_calendar_events(&profile, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
                .into_iter()
                .filter(|event| {
                    event.id.is_some() && event.day == 10 && event.month == 6 && event.year == 2026
                })
                .collect();

        let titles: Vec<_> = events.iter().map(|event| event.title.as_str()).collect();
        assert_eq!(titles, vec!["Morning brush", "Lunch water", "Evening meds"]);
    }

    #[test]
    fn parent_level_breakdown_computes_xp_and_paw_points() {
        let mut profile = test_profile_weeks(12, "indoor");
        profile.parent_level = 3;
        profile.parent_xp = 40;
        profile.paw_points = 85;
        profile.tasks = vec![UserTask {
            id: "t1".to_string(),
            title: "Feed".to_string(),
            completed: true,
            due_label: "Today".to_string(),
            due_day: None,
            due_month: None,
            due_year: None,
            time_minutes: 480,
            reward: 20,
            pet_id: PRIMARY_PET_ID.to_string(),
        }];
        profile.owned_outfits = vec!["classic_collar".to_string(), "cozy_sweater".to_string()];
        profile.activity.push(ProfileActivity {
            message: "Purchased 100 paw points via Stripe Checkout.".to_string(),
            timestamp: 1,
        });

        assert_eq!(lifetime_xp(&profile), 240);
        assert_eq!(task_xp_earned(&profile), 10);
        assert_eq!(task_rewards_earned(&profile), 20);
        assert_eq!(outfit_points_spent(&profile), 50);
        assert_eq!(purchased_paw_points(&profile), 100);

        let html = render_parent_level_breakdown(&profile);
        assert!(html.contains("Parent Level 3 Breakdown"));
        assert!(html.contains("40 / 100"));
        assert!(html.contains("href=\"/home/cat-home\""));
        assert!(html.contains("85"));
        assert!(html.contains("paw-points-icon"));
        assert!(html.contains("+20"));
        assert!(html.contains("-50"));
    }

    #[test]
    fn level_progress_reports_remaining_xp() {
        let mut profile = test_profile_weeks(12, "indoor");
        profile.parent_level = 2;
        profile.parent_xp = 75;
        let (pct, text) = level_progress(&profile);
        assert_eq!(pct, 75);
        assert!(text.contains("25 XP to reach level 3"));
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
    fn account_password_section_shows_change_form_for_regular_users() {
        let html = render_account_password_section("mochi@example.com");
        assert!(html.contains("account-change-password-form"));
        assert!(html.contains("Change password"));
        assert!(html.contains("current_password"));
        assert!(html.contains("new_password"));
    }

    #[test]
    fn account_password_section_hides_change_form_for_admin() {
        let html = render_account_password_section(&admin_email());
        assert!(!html.contains("account-change-password-form"));
        assert!(html.contains("ADMIN_PASSWORD"));
    }

    #[test]
    fn format_timestamp_uses_readable_calendar_date() {
        let formatted = format_timestamp(1_700_000_000);
        assert!(!formatted.contains("day "));
        assert!(formatted.contains("2023"));
    }

    #[test]
    fn format_member_since_uses_long_month_name() {
        let formatted = format_member_since(1_700_000_000);
        assert!(formatted.contains("November"));
        assert!(formatted.contains("2023"));
    }

    #[test]
    fn normalize_pet_name_rejects_blank_and_placeholder_values() {
        assert_eq!(normalize_pet_name("  Mochi  "), Some("Mochi".to_string()));
        assert_eq!(normalize_pet_name(""), None);
        assert_eq!(normalize_pet_name("your cat"), None);
        assert_eq!(normalize_pet_name("No Pet Yet"), None);
    }

    #[test]
    fn account_pet_name_field_shows_save_form_when_pet_exists() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        let html = render_account_pet_name_field(&profile);
        assert!(html.contains("account-pet-name-change-trigger"));
        assert!(html.contains("account-pet-name-form"));
        assert!(html.contains(r#"value="Mochi""#));
        assert!(html.contains(">Mochi<"));
        assert!(html.contains(r#"aria-label="Change pet name""#));
        assert!(html.contains("Save name"));
    }

    #[test]
    fn account_pet_name_field_prompts_setup_when_missing_pet() {
        let profile = default_profile("new@example.com");
        let html = render_account_pet_name_field(&profile);
        assert!(!html.contains("account-pet-name-form"));
        assert!(html.contains("Set up your cat on the My Pet tab."));
    }

    #[test]
    fn login_prefill_cookie_round_trips_special_characters() {
        let encoded = encode_login_prefill_cookie_value("user+tag@example.com", "p@ss \"word'&<>");
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
        assert!(!events
            .iter()
            .any(|event| event.title.contains("FVRCP") && event.day == week_10.day()));
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
            "cat_name=Mochi&pet_breed=Domestic+Shorthair&pet_color=Tabby&pet_birth_date=2024-01-15&pet_indoor_outdoor=indoor&last_vet_date=&conditions=&medications={extra}"
        )
    }

    #[test]
    fn birthday_events_repeat_each_year_in_horizon() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_name = "Mochi".to_string();
        profile.pet_birth_date = Some("2024-06-15".to_string());
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);

        let today = NaiveDate::from_ymd_opt(2026, 3, 1).expect("date");
        let horizon = today + Duration::days(730);
        let events = generate_birthday_calendar_events(&profile, today, horizon);
        assert!(events.iter().any(|event| {
            event.year == 2026
                && event.month == 6
                && event.day == 15
                && event.title.contains("birthday")
        }));
        assert!(events.iter().any(|event| {
            event.year == 2027
                && event.month == 6
                && event.day == 15
                && event.title.contains("birthday")
        }));
    }

    #[test]
    fn derive_age_from_birth_handles_kittens_and_adults() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
        assert_eq!(
            derive_age_from_birth(NaiveDate::from_ymd_opt(2026, 4, 1).expect("dob"), today)
                .expect("kitten"),
            (Some(8), None)
        );
        assert_eq!(
            derive_age_from_birth(NaiveDate::from_ymd_opt(2022, 6, 1).expect("dob"), today)
                .expect("adult"),
            (None, Some(4))
        );
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
    fn symptom_check_form_deserializes_text_and_quick_picks() {
        let form: SymptomCheckForm = serde_urlencoded::from_str(
            "symptoms=vomiting&quick_symptoms=lethargy&quick_symptoms=not+eating",
        )
        .expect("form");
        assert_eq!(form.symptoms, "vomiting");
        assert_eq!(
            form.quick_symptoms,
            vec!["lethargy".to_string(), "not eating".to_string()]
        );
    }

    #[test]
    fn symptom_check_form_deserializes_single_quick_pick() {
        let form: SymptomCheckForm =
            serde_urlencoded::from_str("symptoms=&quick_symptoms=sneezing").expect("form");
        assert!(form.symptoms.is_empty());
        assert_eq!(form.quick_symptoms, vec!["sneezing".to_string()]);
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
    fn validate_pet_video_accepts_mp4_magic_bytes() {
        let mut mp4 = vec![0x00, 0x00, 0x00, 0x20, b'f', b't', b'y', b'p'];
        mp4.extend_from_slice(b"mp42");
        assert_eq!(validate_pet_video(Some("video/mp4"), &mp4), Ok("mp4"));
    }

    #[test]
    fn validate_pet_video_rejects_oversized_file() {
        let bytes = vec![0x00; MAX_PET_VIDEO_BYTES + 1];
        assert!(validate_pet_video(Some("video/mp4"), &bytes).is_err());
    }

    #[test]
    fn validate_pet_video_rejects_bad_content_type() {
        let mut mp4 = vec![0x00, 0x00, 0x00, 0x20, b'f', b't', b'y', b'p'];
        mp4.extend_from_slice(b"mp42");
        assert!(validate_pet_video(Some("application/pdf"), &mp4).is_err());
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
    fn render_pet_user_video_optional_when_uploaded() {
        let mut profile = test_profile_weeks(10, "indoor");
        profile.pet_video_url = Some("/uploads/example-playing.mp4".to_string());
        profile.pet_video_clip_start = Some(12.5);
        profile.pet_video_clip_duration = Some(4.5);
        let html = render_pet_avatar(&profile);
        assert!(html.contains("cinder-photo-toggle"));
        assert!(html.contains("Watch my kitty play!"));
        assert!(html.contains("/uploads/example-playing.mp4"));
        assert!(html.contains("pet-user-video-optional"));
        assert!(html.contains(r#"data-clip-start="12.5""#));
        assert!(html.contains(r#"data-clip-duration="4.5""#));
    }

    #[test]
    fn account_profile_shows_uploaded_cat_video() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_video_url = Some("/uploads/mochi-playing.mp4".to_string());
        profile.pet_video_clip_start = Some(3.0);
        let html = render_account_pet_photo_living(&profile);
        assert!(html.contains(r#"class="account-pet-photo-wrap account-pet-photo-toggle""#));
        assert!(html.contains(r#"role="button""#));
        assert!(html.contains("account-pet-photo-image"));
        assert!(html.contains("account-pet-video-optional"));
        assert!(html.contains("account-pet-video-player"));
        assert!(html.contains("account-pet-media-actions"));
        assert!(html.contains("Change profile photo"));
        assert!(html.contains("Change cat GIF"));
        assert!(html.contains("/uploads/mochi-playing.mp4"));
        assert!(html.contains("tap photo for playing clip"));
        assert!(html.contains("Mochi"));
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
        assert!(!events
            .iter()
            .any(|event| event.title.contains("Last vet visit")));
        assert!(events
            .iter()
            .any(|event| event.title.contains("Vet checkup reminder")));
    }

    #[test]
    fn never_been_to_vet_adds_asap_calendar_reminders_starting_today() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.never_been_to_vet = true;
        profile.last_vet_date = None;

        let today = Local::now().date_naive();
        let events = generate_vet_calendar_events(&profile, today);
        assert!(!events
            .iter()
            .any(|event| event.title.contains("Last vet visit")));
        assert!(events.iter().any(|event| {
            event.title.contains("Make vet appointment ASAP")
                && event.year == today.year() as u32
                && event.month == today.month()
                && event.day == today.day()
        }));
        let two_weeks = today + Duration::weeks(2);
        assert!(events.iter().any(|event| {
            event.title.contains("Make vet appointment ASAP")
                && event.year == two_weeks.year() as u32
                && event.month == two_weeks.month()
                && event.day == two_weeks.day()
        }));
        assert!(!events
            .iter()
            .any(|event| event.title.contains("Vet checkup reminder")));
    }

    #[test]
    fn never_been_to_vet_triggers_asap_task() {
        let mut profile = test_profile_weeks_premium(52, "indoor");
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.never_been_to_vet = true;
        profile.last_vet_date = None;
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        assert!(needs_vet_appointment_asap(&profile, today));
        profile.tasks.clear();
        assert!(refresh_profile_tasks(&mut profile));
        assert!(profile
            .tasks
            .iter()
            .any(|task| task.id == VET_APPOINTMENT_TASK_ID));
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
        let mut profile = test_profile_weeks_premium(52, "indoor");
        profile.pet_age_weeks = None;
        profile.pet_age_years = Some(2);
        profile.pet_vaccines_unknown = true;
        profile.last_vet_date = Some("2025-01-01".to_string());
        profile.never_been_to_vet = false;
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        assert!(needs_vet_appointment_asap(&profile, today));
        profile.tasks.clear();
        assert!(refresh_profile_tasks(&mut profile));
        assert!(profile
            .tasks
            .iter()
            .any(|task| task.id == VET_APPOINTMENT_TASK_ID));
    }

    #[test]
    fn vet_urgency_alert_shows_on_pet_and_calendar_tabs() {
        let mut profile = test_profile_weeks_premium(52, "indoor");
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
        let mut profile = test_profile_weeks_premium(10, "indoor");
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        profile.never_been_to_vet = false;
        profile.last_vet_date = Some("2025-01-01".to_string());
        assert!(vaccines_due_or_overdue(&profile, today));
        assert!(needs_vet_appointment_asap(&profile, today));
    }

    #[test]
    fn admin_credentials_match_configured_email_and_password() {
        assert!(is_admin_credentials(&admin_email(), &admin_password()));
        assert!(!is_admin_credentials(&admin_email(), "wrong-password"));
        assert!(!is_admin_credentials(
            "other@example.com",
            &admin_password()
        ));
    }

    #[test]
    fn user_session_survives_app_state_reopen() {
        let data_dir = std::env::temp_dir().join(format!("ww-session-reopen-{}", Uuid::new_v4()));
        let email = "session-user@example.com";

        let jar = {
            let storage = Storage::open_at(data_dir.clone()).expect("storage");
            storage
                .save_user(&User {
                    username: "SessionUser".to_string(),
                    first_name: "Session".to_string(),
                    last_name: "User".to_string(),
                    email: email.to_string(),
                    password: "TestPass1!".to_string(),
                    created_at: 1,
                })
                .expect("save user");
            let state = AppState { storage };
            create_user_session(&state, CookieJar::new(), email)
        };

        let restarted = AppState {
            storage: Storage::open_at(data_dir).expect("reopen storage"),
        };
        assert_eq!(user_session_email(&restarted, &jar).as_deref(), Some(email));
    }

    #[test]
    fn complete_sign_in_grants_admin_session_for_admin_email() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-admin-login-{}", Uuid::new_v4())),
        )
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

        let state = AppState { storage };

        let jar = complete_sign_in(&state, CookieJar::new(), &admin_email());
        assert!(admin_session_valid(&state, &jar));
        assert_eq!(
            user_session_email(&state, &jar).as_deref(),
            Some(admin_email().as_str())
        );
    }

    #[test]
    fn admin_env_password_syncs_database_hash() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-admin-sync-{}", Uuid::new_v4())),
        )
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
    fn admin_without_pet_gets_onboarding_modal() {
        let profile = admin_profile(&admin_email());
        assert!(!render_onboarding_modal(&profile).is_empty());
    }

    #[test]
    fn admin_with_pet_skips_onboarding_modal() {
        let mut profile = admin_profile(&admin_email());
        profile.onboarding_completed = true;
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(render_onboarding_modal(&profile).is_empty());
    }

    #[test]
    fn incomplete_onboarding_shows_no_pet_blurb_and_cta() {
        let profile = default_profile("user@example.com");
        assert_eq!(render_pet_blurb(&profile), "Create a pet");
        let cta = render_pet_setup_cta(&profile);
        assert!(cta.contains("Create your pet"));
        assert!(cta.contains("pet-setup-trigger"));
        assert!(user_needs_pet_setup(&profile));
    }

    #[test]
    fn admin_without_pet_gets_pet_setup_cta() {
        let profile = admin_profile(&admin_email());
        assert!(render_pet_setup_cta(&profile).contains("Create your pet"));
        assert!(user_needs_pet_setup(&profile));
    }

    #[test]
    fn admin_with_pet_skips_pet_setup_cta() {
        let mut profile = admin_profile(&admin_email());
        profile.onboarding_completed = true;
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(render_pet_setup_cta(&profile).is_empty());
        assert!(!user_needs_pet_setup(&profile));
    }

    #[test]
    fn onboarding_modal_hidden_until_opened() {
        let profile = default_profile("user@example.com");
        let modal = render_onboarding_modal(&profile);
        assert!(modal.contains("id=\"onboarding-modal\""));
        assert!(modal.contains("hidden"));
    }

    #[test]
    fn calendar_tab_shows_pet_setup_prompt_when_onboarding_incomplete() {
        let profile = default_profile("user@example.com");
        let prompt = render_calendar_pet_setup_prompt(&profile);
        assert!(prompt.contains("calendar-pet-setup-alert"));
        assert!(prompt.contains("calendar-pet-setup-trigger"));
        assert!(prompt.contains("pet-setup-trigger"));
        assert!(prompt.contains("Create your pet"));
    }

    #[test]
    fn calendar_pet_setup_prompt_hidden_after_onboarding() {
        let mut profile = default_profile("user@example.com");
        profile.onboarding_completed = true;
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(render_calendar_pet_setup_prompt(&profile).is_empty());
    }

    #[test]
    fn placeholder_pet_name_does_not_count_as_having_pet() {
        let mut profile = default_profile("user@example.com");
        profile.onboarding_completed = true;
        profile.pet_name = "No pet yet".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(!profile_has_pet(&profile));
        assert_eq!(render_pet_blurb(&profile), "Create a pet");
    }

    #[test]
    fn placeholder_profile_still_needs_pet_setup_when_flag_set() {
        let mut profile = default_profile("user@example.com");
        profile.onboarding_completed = true;
        assert!(user_needs_pet_setup(&profile));
        assert!(!render_pet_setup_cta(&profile).is_empty());
        assert!(!render_onboarding_modal(&profile).is_empty());
    }

    #[test]
    fn admin_without_pet_gets_calendar_pet_setup_prompt() {
        let profile = admin_profile(&admin_email());
        assert!(!render_calendar_pet_setup_prompt(&profile).is_empty());
    }

    #[test]
    fn completed_onboarding_shows_personalized_pet_blurb() {
        let mut profile = default_profile("user@example.com");
        profile.onboarding_completed = true;
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        let blurb = render_pet_blurb(&profile);
        assert!(blurb.contains("Mochi"));
        assert!(blurb.contains("mirrors your real cat"));
        assert!(render_pet_setup_cta(&profile).is_empty());
        assert_eq!(display_pet_name(&profile), "Mochi");
    }

    #[test]
    fn cat_home_scene_renders_room_and_starter_decor() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        let scene = render_cat_home_scene(&profile);
        assert!(scene.contains(r#"data-room="sunny_nook""#));
        assert!(scene.contains("cat-home-pet-stage"));
        assert!(scene.contains("cat-home-pet-stack"));
        assert!(scene.contains("cat-home-pet-bubble"));
        assert!(scene.contains("pet me!"));
        assert!(scene.contains("Soft Mat"));
    }

    #[test]
    fn decor_cards_show_shortfall_when_paw_points_are_insufficient() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 42;
        profile.owned_decor = default_owned_decor();
        profile.equipped_decor = default_equipped_decor();

        let cards = render_decor_cards(&profile);
        assert!(cards.contains("Cozy Hammock"));
        assert!(cards.contains("need-paw-points-trigger"));
        assert!(cards.contains(r#"data-shop-purchasable="true""#));
        assert!(cards.contains(r#"data-shop-id="hammock""#));
        assert!(cards.contains(r#"data-item-name="Cozy Hammock""#));
        assert!(cards.contains(r#"data-item-price="55""#));
        assert!(!cards.contains(r#"decor_id" value="hammock""#));
    }

    #[tokio::test]
    async fn decor_buy_hammock_redirects_when_paw_points_are_insufficient() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "decor-buy@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 42;
        profile.owned_decor = default_owned_decor();
        profile.equipped_decor = default_equipped_decor();
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/decor/buy")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie_header)
                    .body(Body::from("decor_id=hammock"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response_location(response),
            "/home/cat-home?status=need_paw_points&decor_id=hammock"
        );

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 42);
        assert!(!updated.owned_decor.iter().any(|id| id == "hammock"));
    }

    #[tokio::test]
    async fn paw_points_needed_page_renders_purchase_prompt() {
        let state = routing_test_state();
        let email = "need-points@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 42;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let response = paw_points_needed_page(
            State(state),
            jar,
            Query(NeedPawPointsQuery {
                decor_id: Some("hammock".to_string()),
                outfit_id: None,
                boost_id: None,
                petting_bonus_id: None,
                return_to: Some("cat_home".to_string()),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert!(html.contains("You don't have enough paw points"));
        assert!(html.contains("Purchase paw points"));
        assert!(html.contains("Cozy Hammock"));
        assert!(html.contains("You need <strong>13</strong> more"));
        assert!(html.contains(r#"href="/home/cat-home""#));
    }

    #[tokio::test]
    async fn decor_buy_hammock_succeeds_with_enough_paw_points() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "decor-buy-ok@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 60;
        profile.owned_decor = default_owned_decor();
        profile.equipped_decor = default_equipped_decor();
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/decor/buy")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie_header)
                    .body(Body::from("decor_id=hammock"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response_location(response),
            "/home/cat-home?status=decor_bought"
        );

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 5);
        assert!(updated.owned_decor.iter().any(|id| id == "hammock"));
        assert_eq!(
            updated.equipped_decor.get("bed"),
            Some(&"hammock".to_string())
        );
    }

    #[test]
    fn cat_home_outfit_shop_renders_slider_with_cat_home_return() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 100;
        let shop = render_cat_home_outfit_shop(&profile);
        assert!(shop.contains("cat-home-outfit-shop"));
        assert!(shop.contains("cat-home-outfit-slider"));
        assert!(shop.contains(r#"name="return_to" value="cat_home""#));
        assert!(shop.contains("Buy for"));
        assert!(shop.contains("Cozy Sweater"));
    }

    #[test]
    fn cat_home_tap_boost_shop_renders_slider_with_rewards() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 100;
        let shop = render_cat_home_tap_boost_shop(&profile);
        assert!(shop.contains("cat-home-tap-boost-shop"));
        assert!(shop.contains("cat-home-tap-boost-slider"));
        assert!(shop.contains("Level up petting"));
        assert!(shop.contains("+1 per pet"));
        assert!(shop.contains("Happy Taps"));
        assert!(shop.contains(r#"name="return_to" value="cat_home""#));
        assert!(shop.contains(r#"action="/home/tap-boosts/buy""#));
    }

    #[test]
    fn equipped_tap_boost_reward_uses_active_boost() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.equipped_tap_boost = "mega_cuddles".to_string();
        assert_eq!(equipped_tap_boost_reward(&profile), 5);
    }

    #[tokio::test]
    async fn tap_boost_buy_activates_happy_taps() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "tap-boost-buy@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 50;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/tap-boosts/buy")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie_header)
                    .body(Body::from("boost_id=happy_taps&return_to=cat_home"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response_location(response),
            "/home/cat-home?status=boost_bought"
        );

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 10);
        assert_eq!(updated.equipped_tap_boost, "happy_taps");
        assert!(updated.owned_tap_boosts.iter().any(|id| id == "happy_taps"));
    }

    #[tokio::test]
    async fn pet_pet_awards_active_tap_boost_reward() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "tap-boost-pet@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.paw_points = 10;
        profile.equipped_tap_boost = "happy_taps".to_string();
        profile.owned_tap_boosts = vec!["basic_pets".to_string(), "happy_taps".to_string()];
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/cat-home/pet-pet")
                    .header("accept", "application/json")
                    .header("cookie", cookie_header)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["tap_reward"], 2);
        assert_eq!(data["paw_points"], 12);

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 12);
    }

    #[test]
    fn cat_home_status_block_includes_outfit_flash_messages() {
        assert!(cat_home_status_block(Some("outfit_bought")).contains("Outfit purchased"));
        assert!(cat_home_status_block(Some("outfit_points")).is_empty());
        assert!(cat_home_status_block(Some("need_paw_points")).is_empty());
    }

    #[tokio::test]
    async fn cat_home_page_opens_need_paw_points_modal_for_decor_shortfall() {
        let state = routing_test_state();
        let email = "need-modal@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 42;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let response = cat_home_page(
            State(state),
            jar,
            Query(CatHomeQuery {
                status: Some("need_paw_points".to_string()),
                decor_id: Some("hammock".to_string()),
                outfit_id: None,
                boost_id: None,
                petting_bonus_id: None,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert!(html.contains("need-paw-points-modal"));
        assert!(html.contains(r#"data-auto-open="true""#));
        assert!(html.contains("Almost there!"));
        assert!(html.contains("Cozy Hammock"));
        assert!(!html.contains("Not enough paw points for that decor"));
    }

    #[tokio::test]
    async fn cat_home_page_skips_need_paw_points_modal_when_balance_is_sufficient() {
        let state = routing_test_state();
        let email = "need-modal-ok@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 120;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let response = cat_home_page(
            State(state),
            jar,
            Query(CatHomeQuery {
                status: Some("need_paw_points".to_string()),
                decor_id: Some("hammock".to_string()),
                outfit_id: None,
                boost_id: None,
                petting_bonus_id: None,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert!(!html.contains(r#"data-auto-open="true""#));
        assert!(html.contains(r#"decor_id" value="hammock""#));
        assert!(html.contains("Buy for 55 pts"));
    }

    #[tokio::test]
    async fn cat_home_page_replaces_outfit_shop_placeholder() {
        let state = routing_test_state();
        let email = "cat-home@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let response = cat_home_page(State(state), jar, Query(CatHomeQuery::default()))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert!(
            !html.contains("{{CAT_HOME_OUTFIT_SHOP}}"),
            "cat home page leaked outfit shop placeholder"
        );
        assert!(
            !html.contains("{{CAT_HOME_TAP_BOOST_SHOP}}"),
            "cat home page leaked tap boost shop placeholder"
        );
        assert!(
            !html.contains("{{PAW_POINTS_ICON}}"),
            "cat home page leaked paw points icon placeholder"
        );
        assert!(
            !html.contains("{{NEED_PAW_POINTS_MODAL}}"),
            "cat home page leaked need paw points modal placeholder"
        );
        assert!(html.contains("paw-points-icon.png"));
        assert!(html.contains("need-paw-points-modal"));
        assert!(html.contains("cat-home-outfit-shop"));
        assert!(html.contains("cat-home-outfit-slider"));
        assert!(html.contains("cat-home-tap-boost-shop"));
        assert!(
            !html.contains("{{CAT_HOME_PETTING_BONUS_SHOP}}"),
            "cat home page leaked petting bonus shop placeholder"
        );
        assert!(html.contains("cat-home-petting-bonus-shop"));
        assert!(html.contains("Petting bonuses"));
        assert!(html.contains("Love Explosion"));
        assert!(html.contains("Level up petting"));
        assert!(html.contains("Dress up"));
    }

    #[test]
    fn cat_home_petting_bonus_shop_renders_timed_multipliers() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 200;
        profile.equipped_tap_boost = "mega_cuddles".to_string();
        profile.owned_tap_boosts = vec!["basic_pets".to_string(), "mega_cuddles".to_string()];

        let shop = render_cat_home_petting_bonus_shop(&profile);
        assert!(shop.contains("10× your +5 boost · +50 per tap"));
        assert!(shop.contains("60 seconds"));
        assert!(shop.contains("Love Explosion"));
        assert!(shop.contains("+5 per pet"));
    }

    #[test]
    fn effective_tap_reward_stacks_petting_bonus_on_equipped_boost() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.equipped_tap_boost = "mega_cuddles".to_string();
        profile.active_petting_bonus = "love_explosion".to_string();
        profile.petting_bonus_expires_at = timestamp_now() + 60;

        let (total, base, multiplier) = effective_tap_reward(&profile);
        assert_eq!(base, 5);
        assert_eq!(multiplier, Some(10));
        assert_eq!(total, 50);
    }

    #[test]
    fn ensure_tap_boost_state_repairs_invalid_equipped_boost() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.owned_tap_boosts = vec!["basic_pets".to_string(), "happy_taps".to_string()];
        profile.equipped_tap_boost = "missing_boost".to_string();

        assert!(ensure_tap_boost_state(&mut profile));
        assert_eq!(profile.equipped_tap_boost, "happy_taps");
    }

    #[tokio::test]
    async fn petting_bonus_buy_adds_inventory() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "petting-bonus-buy@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.paw_points = 200;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/petting-bonuses/buy")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie_header)
                    .body(Body::from("bonus_id=love_explosion&return_to=cat_home"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response_location(response),
            "/home/cat-home?status=petting_bonus_bought"
        );

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 50);
        assert_eq!(petting_bonus_inventory_count(&updated, "love_explosion"), 1);
    }

    #[tokio::test]
    async fn pet_pet_awards_petting_bonus_multiplier() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "petting-bonus-pet@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.paw_points = 10;
        profile.equipped_tap_boost = "happy_taps".to_string();
        profile.owned_tap_boosts = vec!["basic_pets".to_string(), "happy_taps".to_string()];
        profile.active_petting_bonus = "love_explosion".to_string();
        profile.petting_bonus_expires_at = timestamp_now() + 60;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/cat-home/pet-pet")
                    .header("accept", "application/json")
                    .header("cookie", cookie_header)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["tap_reward"], 20);
        assert_eq!(data["tap_boost_base"], 2);
        assert_eq!(data["tap_multiplier"], 10);
        assert_eq!(data["paw_points"], 30);
    }

    #[tokio::test]
    async fn pet_name_submit_persists_new_name() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "pet-name@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_name = "Mochi".to_string();
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/pet-name")
                    .header("accept", "application/json")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie_header)
                    .body(Body::from("pet_name=Pippin"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["status"], "done");
        assert_eq!(data["pet_name"], "Pippin");

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.pet_name, "Pippin");
    }

    #[tokio::test]
    async fn pet_pet_awards_one_paw_point_per_request() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "pet-pet@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.paw_points = 10;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);

        for expected in [11_u32, 12] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/home/cat-home/pet-pet")
                        .header("accept", "application/json")
                        .header("cookie", cookie_header.clone())
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");

            assert_eq!(response.status(), StatusCode::OK);
            let body = response_html(response).await;
            let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
            assert_eq!(data["ok"], true);
            assert_eq!(data["paw_points"], expected);
        }

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 12);
    }

    #[test]
    fn pet_check_cta_links_to_cat_home_after_onboarding() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Domestic Shorthair".to_string();
        let cta = render_pet_check_cta(&profile);
        assert!(cta.contains("/home/cat-home"));
        assert!(cta.contains("Check on your cat"));
        assert!(render_cat_home_nav_link(&profile).contains("/home/cat-home"));
    }

    #[test]
    fn pet_check_cta_prompts_setup_before_onboarding() {
        let profile = default_profile("user@example.com");
        let cta = render_pet_check_cta(&profile);
        assert!(cta.contains("virtual home"));
        assert!(cta.contains("pet-setup-trigger"));
        assert!(render_cat_home_nav_link(&profile).is_empty());
    }

    #[test]
    fn admin_account_skips_vet_appointment_task() {
        let profile = admin_profile(&admin_email());
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("date");
        assert!(!needs_vet_appointment_asap(&profile, today));
    }

    #[test]
    fn admin_with_pet_gets_starter_care_tasks() {
        let mut profile = admin_profile(&admin_email());
        profile.pet_name = "Cinder".to_string();
        profile.pet_breed = "Maine Coon".to_string();
        profile.pet_birth_date = Some("2021-06-07".to_string());
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(refresh_profile_tasks(&mut profile));
        let html = render_task_list(&profile);
        assert!(html.contains("Morning feeding"));
        assert!(html.contains("+15 pts"));
        assert!(!profile
            .tasks
            .iter()
            .any(|task| task.id == VET_APPOINTMENT_TASK_ID));
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
    fn health_tab_shows_symptom_checker() {
        let profile = test_profile_weeks_premium(10, "indoor");
        let html = render_health_tab(&profile);
        assert!(html.contains("symptom-checker-form"));
        assert!(html.contains("Not a vet"));
        assert!(html.contains("action=\"/home/health/symptoms\""));
    }

    #[test]
    fn health_tab_shows_financial_hardship_resources() {
        let profile = test_profile_weeks_premium(10, "indoor");
        let html = render_health_tab(&profile);
        assert!(html.contains("financial-hardship-card"));
        assert!(html.contains("Experiencing financial hardship?"));
        assert!(html.contains("CareCredit"));
        assert!(html.contains("Scratchpay"));
        assert!(html.contains("Trupanion"));
        assert!(html.contains("shelter-locator-form"));
    }

    #[test]
    fn health_tab_shows_symptom_checker_for_free_user() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Persian".to_string();
        let html = render_health_tab(&profile);
        assert!(html.contains("symptom-checker-form"));
        assert!(html.contains("Not a vet"));
    }

    #[test]
    fn health_tab_shows_vet_notes_form() {
        let mut profile = test_profile_weeks_premium(10, "indoor");
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
    fn health_tab_shows_vet_visit_form_when_pet_exists() {
        let profile = test_profile_weeks_premium(10, "indoor");
        let html = render_health_tab(&profile);
        assert!(html.contains("Add veterinary information"));
        assert!(html.contains("health-vet-disclosure"));
        assert!(html.contains("action=\"/home/vet-visit\""));
        assert!(html.contains("health-vaccine-rows"));
        assert!(html.contains("health-vet-note"));
    }

    #[test]
    fn health_tab_shows_unlocked_breed_guide_for_premium_user() {
        let mut profile = test_profile_weeks_premium(52, "indoor");
        profile.pet_breed = "Persian".to_string();
        let html = render_health_tab(&profile);
        assert!(html.contains("breed-guide-card-owned"));
        assert!(html.contains("Persian care guide"));
        assert!(html.contains(r#"href="/home/breed-guide/persian""#));
        assert!(!html.contains("breed-guide-card-locked"));
    }

    #[test]
    fn health_tab_shows_locked_breed_guide_for_free_user() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Persian".to_string();
        let html = render_health_tab(&profile);
        assert!(html.contains("breed-guide-card-locked"));
        assert!(html.contains("Persian care guide"));
        assert!(html.contains(r#"href="/home/breed-guide/persian""#));
        assert!(
            html.contains(r#"action="/home/breed-guides/checkout""#)
                || html.contains("STRIPE_SECRET_KEY")
        );
    }

    #[test]
    fn health_tab_shows_unlocked_breed_guide_when_owned() {
        let mut profile = test_profile_weeks_premium(52, "indoor");
        profile.pet_breed = "Siamese".to_string();
        profile.owned_breed_guides = vec!["siamese".to_string()];
        let html = render_health_tab(&profile);
        assert!(html.contains("breed-guide-card-owned"));
        assert!(html.contains(r#"href="/home/breed-guide/siamese""#));
        assert!(!html.contains("breed-guide-card-locked"));
    }

    #[test]
    fn health_tab_shows_premium_upsell_for_free_user() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_breed = "Persian".to_string();
        let html = render_health_tab(&profile);
        assert!(html.contains("premium-upsell-card-compact"));
        assert!(html.contains("WhiskerWatch Plus"));
        assert!(html.contains("breed-guide-card-locked"));
        assert!(html.contains("/home/breed-guides"));
        assert!(!html.contains("action=\"/home/vet-visit\""));
    }

    #[test]
    fn pet_health_info_free_tier_shows_basic_profile_only() {
        let profile = test_profile_weeks(52, "indoor");
        let html = render_pet_health_info(&profile);
        assert!(html.contains("Domestic Shorthair"));
        assert!(html.contains("WhiskerWatch Plus"));
        assert!(!html.contains("Last vet appointment"));
        assert!(!html.contains("Vaccine history"));
    }

    #[test]
    fn merge_calendar_skips_vet_events_for_free_user() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.never_been_to_vet = true;
        let today = Local::now().date_naive();
        let events = merge_calendar_events(&profile, today);
        assert!(!events
            .iter()
            .any(|event| event.title.contains("vet") || event.title.contains("Vet")));
    }

    #[test]
    fn free_user_does_not_get_vet_urgency_alert() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.pet_vaccines_unknown = true;
        assert!(render_vet_urgency_alert(&profile, "pet-tab-vet-alert").is_empty());
    }

    #[test]
    #[test]
    fn additional_pets_get_separate_tasks_and_switcher() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.premium_unlocked = true;
        profile.additional_pets.push(HouseholdPet {
            id: "pet_test2".to_string(),
            pet_name: "Luna".to_string(),
            pet_breed: "Siamese".to_string(),
            pet_color: "Seal point".to_string(),
            pet_mood: "Happy".to_string(),
            pet_age_weeks: None,
            pet_age_years: Some(2),
            pet_birth_date: Some("2024-06-01".to_string()),
            last_vet_date: None,
            never_been_to_vet: false,
            pet_conditions: String::new(),
            pet_medications: String::new(),
            pet_indoor_outdoor: Some("indoor".to_string()),
            vaccine_history: vec![],
            pet_vaccines_unknown: false,
            care_schedule: default_care_schedule(),
            pet_photo_url: None,
            pet_video_url: None,
            pet_video_clip_start: None,
            pet_video_clip_duration: None,
            pet_video_zoom: None,
            pet_video_offset_x: None,
            pet_video_offset_y: None,
            deceased: false,
            deceased_at: None,
            memorial_videos: Vec::new(),
            memorial_comfort_seen: false,
        });
        assert!(refresh_profile_tasks(&mut profile));

        let primary_feeds = profile
            .tasks
            .iter()
            .filter(|task| task.id == "feed_breakfast" && task.pet_id == PRIMARY_PET_ID)
            .count();
        let luna_feeds = profile
            .tasks
            .iter()
            .filter(|task| task.id == "feed_breakfast" && task.pet_id == "pet_test2")
            .count();
        assert_eq!(primary_feeds, 1);
        assert_eq!(luna_feeds, 1);

        profile.active_pet_id = "pet_test2".to_string();
        let html = render_task_list(&profile);
        assert!(html.contains("feed_breakfast"));
        assert!(!html.contains(&format!("pet_id\" value=\"{PRIMARY_PET_ID}\"")));

        let switcher = render_pet_switcher(&profile);
        assert!(switcher.contains("Mochi"));
        assert!(switcher.contains("Luna"));
        assert!(switcher.contains("pet_test2"));
    }

    #[test]
    fn multi_pet_section_shows_add_cat_button_not_inline_form() {
        let html = entitlements::render_multi_pet_section(
            true,
            "premium@example.com",
            true,
            "Mochi",
            0,
            "",
            true,
        );
        assert!(html.contains("add-cat-trigger"));
        assert!(html.contains("Add another cat"));
        assert!(html.contains("Single kitty household"));
        assert!(!html.contains("Multi-kitty household"));
        assert!(!html.contains("additional_pet_name"));
        assert!(render_add_cat_onboarding_modal().contains("add_cat_name"));
        assert!(render_add_cat_onboarding_modal().contains("add_pet"));
    }

    #[test]
    fn premium_user_can_add_unlimited_additional_pets() {
        let profile = test_profile_weeks_premium(52, "indoor");
        assert!(entitlements::can_add_pet(
            profile.premium_unlocked,
            &profile.email,
            true,
            0,
        ));
        assert!(entitlements::can_add_pet(
            profile.premium_unlocked,
            &profile.email,
            true,
            12,
        ));
    }

    #[test]
    fn free_user_cannot_add_additional_pet() {
        let profile = test_profile_weeks(52, "indoor");
        assert!(!entitlements::can_add_pet(
            profile.premium_unlocked,
            &profile.email,
            true,
            profile.additional_pets.len(),
        ));
    }

    #[test]
    fn care_streak_increments_and_milestones() {
        let mut profile = test_profile_weeks(52, "indoor");
        let day1 = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
        let day2 = NaiveDate::from_ymd_opt(2026, 6, 2).expect("date");
        let day3 = NaiveDate::from_ymd_opt(2026, 6, 3).expect("date");

        assert_eq!(share_cards::update_care_streak(&mut profile, day1), None);
        assert_eq!(profile.care_streak_days, 1);
        assert_eq!(share_cards::update_care_streak(&mut profile, day2), None);
        assert_eq!(profile.care_streak_days, 2);
        assert_eq!(share_cards::update_care_streak(&mut profile, day3), Some(3));
        assert_eq!(profile.best_care_streak, 3);
    }

    #[test]
    fn level_share_headline_uses_pet_name() {
        let profile = test_profile_weeks(52, "indoor");
        let offer = share_cards::create_share_offer(
            &profile,
            share_cards::ShareCardKind::LevelUp(10),
            "http://localhost:3000",
            1_700_000_000,
        )
        .expect("offer");
        assert_eq!(offer.headline, "My cat Mochi hit level 10! 🐾");
        assert!(offer.url.contains("/share/"));
    }

    #[tokio::test]
    async fn dashboard_replaces_care_streak_chip_placeholder() {
        let state = routing_test_state();
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = "streak-chip@example.com".to_string();
        profile.care_streak_days = 5;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), &profile.email);
        let response = dashboard_page(State(state), jar, Query(empty_dashboard_query()))
            .await
            .into_response();
        let html = response_html(response).await;
        assert_no_unreplaced_dashboard_placeholders(&html);
        assert!(!html.contains("{{CARE_STREAK_CHIP}}"));
        assert!(!html.contains("{{STREAK_CARD_SECTION}}"));
        assert!(html.contains("care-streak-chip"));
        assert!(html.contains("care-streak-card"));
        assert!(html.contains("5 days"));
    }

    #[tokio::test]
    async fn dashboard_replaces_streak_card_section_when_streak_is_zero() {
        let state = routing_test_state();
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = "streak-zero@example.com".to_string();
        profile.care_streak_days = 0;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), &profile.email);
        let response = dashboard_page(State(state), jar, Query(empty_dashboard_query()))
            .await
            .into_response();
        let html = response_html(response).await;
        assert_no_unreplaced_dashboard_placeholders(&html);
        assert!(!html.contains("{{STREAK_CARD_SECTION}}"));
        assert!(html.contains("care-streak-card--empty"));
        assert!(html.contains("Start today"));
    }

    #[tokio::test]
    async fn dashboard_replaces_account_tab_sections() {
        let state = routing_test_state();
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = "account-sections@example.com".to_string();
        profile.premium_unlocked = false;
        profile.community_visible = true;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), &profile.email);
        let response = dashboard_page(State(state), jar, Query(empty_dashboard_query()))
            .await
            .into_response();
        let html = response_html(response).await;
        assert_no_unreplaced_dashboard_placeholders(&html);
        assert!(!html.contains("{{ACCOUNT_PREMIUM_SECTION}}"));
        assert!(!html.contains("{{COMMUNITY_VISIBILITY_SECTION}}"));
        assert!(!html.contains("{{ACCOUNT_NOTIFICATIONS_SECTION}}"));
        assert!(!html.contains("{{ACCOUNT_ONBOARDING_EMAILS_SECTION}}"));
        assert!(!html.contains("{{FRIENDS_AND_SHARING_SECTION}}"));
        assert!(!html.contains("{{FRIENDS_TAB_LABEL}}"));
        assert!(html.contains(r#"data-tab="friends""#));
        assert!(html.contains("panel-friends"));
        assert!(html.contains("premium-upsell-card"));
        assert!(html.contains("community-visibility-card"));
        assert!(html.contains("friends-sharing-card"));
        assert!(html.contains("pet-sharing-card"));
        assert!(html.contains("push-notifications-card"));
        assert!(html.contains("onboarding-emails-card"));
    }

    #[test]
    fn care_streak_chip_renders_current_streak() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.care_streak_days = 7;
        let html = streak_rewards::render_care_streak_chip(&profile);
        assert!(html.contains("7 days"));
        assert!(html.contains("care-streak-chip"));
        assert!(html.contains(r#"href="/home/streak""#));
    }

    #[test]
    fn breed_catalog_includes_all_categories() {
        let html = breeds::render_catalog_html("setup=pet");
        assert!(html.contains("Premium care guide"));
        assert!(html.contains("Long-Haired Breeds"));
        assert!(html.contains("Short-Haired Breeds"));
        assert!(html.contains("Unique / Specialty Breeds"));
        assert!(html.contains("Colorpoint Breeds (Siamese-derived)"));
        assert!(html.contains("Domestic Longhair"));
        assert!(html.contains("mixed ancestry, fluffy coat, independent and adaptable"));
        assert!(html.contains("Persian"));
        assert!(html.contains("flat face, silky coat, calm and gentle"));
        assert!(html.contains("Snowshoe"));
        assert!(html.contains(r#"href="/home?setup=pet&amp;breed=Maine%20Coon""#));
    }

    #[test]
    fn onboarding_modal_uses_breed_picker_input() {
        let profile = default_profile("user@example.com");
        let modal = render_onboarding_modal(&profile);
        assert!(modal.contains(r#"id="pet_breed""#));
        assert!(modal.contains("breed-picker-input"));
        assert!(modal.contains("readonly"));
        assert!(!modal.contains("cat-breeds"));
        assert!(modal.contains("birth-date-picker"));
        assert!(modal.contains(r#"id="pet_birth_month""#));
        assert!(!modal.contains(r#"type="date""#));
        assert!(modal.contains(r#"name="pet_photo""#));
        assert!(modal.contains("required"));
        assert!(modal.contains("pet-color-picker"));
        assert!(modal.contains(r#"id="pet_color_select""#));
        assert!(modal.contains(r#"name="pet_color""#));
    }

    #[test]
    fn replace_litter_reward_syncs_to_latest_default() {
        let mut profile = default_profile("user@example.com");
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(refresh_profile_tasks(&mut profile));
        if let Some(task) = profile
            .tasks
            .iter()
            .find(|task| task.id == "replace_litter")
        {
            assert_eq!(task.reward, 50);
        } else {
            panic!("replace_litter task missing");
        }

        if let Some(task) = profile
            .tasks
            .iter_mut()
            .find(|task| task.id == "replace_litter")
        {
            task.reward = 25;
        }
        assert!(refresh_profile_tasks(&mut profile));
        assert_eq!(
            profile
                .tasks
                .iter()
                .find(|task| task.id == "replace_litter")
                .map(|task| task.reward),
            Some(50)
        );
    }

    #[test]
    fn starter_care_tasks_appear_once_pet_is_created() {
        let mut profile = default_profile("user@example.com");
        assert!(render_task_list(&profile).is_empty());

        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());

        assert!(refresh_profile_tasks(&mut profile));
        let html = render_task_list(&profile);
        assert!(html.contains("Morning feeding"));
        assert!(html.contains("+15 pts"));
        assert!(html.contains("15-minute play session"));
        assert!(html.contains("+20 pts"));
    }

    #[test]
    fn daily_tasks_reset_after_midnight() {
        let mut profile = test_profile_weeks(52, "indoor");
        let today = Local::now().date_naive();
        let yesterday = today.pred_opt().expect("yesterday");

        profile.tasks = vec![
            UserTask {
                id: "feed_breakfast".to_string(),
                title: "Morning feeding".to_string(),
                completed: true,
                due_label: "Daily · 8:00 AM".to_string(),
                due_day: Some(yesterday.day()),
                due_month: Some(yesterday.month()),
                due_year: Some(yesterday.year() as u32),
                time_minutes: 480,
                reward: 15,
                pet_id: PRIMARY_PET_ID.to_string(),
            },
            UserTask {
                id: "play_session".to_string(),
                title: "15-minute play session".to_string(),
                completed: true,
                due_label: "Today · 6:00 PM".to_string(),
                due_day: Some(yesterday.day()),
                due_month: Some(yesterday.month()),
                due_year: Some(yesterday.year() as u32),
                time_minutes: 1080,
                reward: 20,
                pet_id: PRIMARY_PET_ID.to_string(),
            },
            UserTask {
                id: "replace_litter".to_string(),
                title: "Replace litter".to_string(),
                completed: true,
                due_label: "Weekly · anytime".to_string(),
                due_day: Some(yesterday.day()),
                due_month: Some(yesterday.month()),
                due_year: Some(yesterday.year() as u32),
                time_minutes: 600,
                reward: 50,
                pet_id: PRIMARY_PET_ID.to_string(),
            },
        ];

        assert!(refresh_profile_tasks(&mut profile));

        let feed = profile
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        assert!(!feed.completed);
        assert_eq!(feed.due_day, Some(today.day()));

        let play = profile
            .tasks
            .iter()
            .find(|task| task.id == "play_session")
            .expect("play");
        assert!(!play.completed);
        assert_eq!(play.due_day, Some(today.day()));

        let litter = profile
            .tasks
            .iter()
            .find(|task| task.id == "replace_litter")
            .expect("litter");
        assert!(litter.completed);
        assert_eq!(litter.due_day, Some(yesterday.day()));
    }

    #[test]
    fn refresh_backfills_due_dates_without_clearing_same_day_completion() {
        let mut profile = test_profile_weeks(52, "indoor");
        let today = Local::now().date_naive();
        profile.tasks = vec![UserTask {
            id: "feed_breakfast".to_string(),
            title: "Morning feeding".to_string(),
            completed: true,
            due_label: "Daily · 8:00 AM".to_string(),
            due_day: None,
            due_month: None,
            due_year: None,
            time_minutes: 480,
            reward: 15,
            pet_id: PRIMARY_PET_ID.to_string(),
        }];

        assert!(refresh_profile_tasks(&mut profile));
        let feed = profile
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        assert!(feed.completed);
        assert_eq!(feed.due_day, Some(today.day()));
        assert_eq!(feed.due_month, Some(today.month()));
        assert_eq!(feed.due_year, Some(today.year() as u32));
    }

    #[tokio::test]
    async fn task_toggle_reopen_deducts_paw_points_after_profile_refresh() {
        let state = routing_test_state();
        let email = "toggle-reopen@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        assert!(refresh_profile_tasks(&mut profile));
        let reward = profile
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed")
            .reward;
        profile.paw_points = 100 + reward;
        let task = profile
            .tasks
            .iter_mut()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        task.completed = true;
        task.due_day = None;
        task.due_month = None;
        task.due_year = None;
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, "application/json".parse().expect("accept header"));
        let response = task_toggle(
            State(state.clone()),
            jar,
            headers,
            Form(TaskToggleForm {
                task_id: "feed_breakfast".to_string(),
                pet_id: PRIMARY_PET_ID.to_string(),
                pet_owner: String::new(),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["status"], "reopened");
        assert_eq!(data["paw_points"], 100);

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        assert_eq!(updated.paw_points, 100);
        let feed = updated
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        assert!(!feed.completed);
    }

    #[test]
    fn reopening_task_deducts_paw_points_without_going_negative() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.paw_points = 100;
        profile.parent_xp = 20;
        profile.tasks = vec![UserTask {
            id: "feed_breakfast".to_string(),
            title: "Morning feeding".to_string(),
            completed: true,
            due_label: "Daily · 8:00 AM".to_string(),
            due_day: None,
            due_month: None,
            due_year: None,
            time_minutes: 480,
            reward: 15,
            pet_id: PRIMARY_PET_ID.to_string(),
        }];

        let (_title, reward) = reopen_completed_task(&mut profile, 0);
        assert_eq!(reward, 15);
        assert!(!profile.tasks[0].completed);
        assert_eq!(profile.paw_points, 85);
        assert_eq!(profile.parent_xp, 13);

        profile.paw_points = 10;
        profile.tasks[0].completed = true;
        let (_title, _) = reopen_completed_task(&mut profile, 0);
        assert_eq!(profile.paw_points, 0);
    }

    #[test]
    fn reopening_task_reverses_level_up_from_completion() {
        let mut profile = test_profile_weeks(52, "indoor");
        profile.parent_level = 2;
        profile.parent_xp = 5;
        profile.paw_points = 40;
        profile.tasks = vec![UserTask {
            id: "replace_litter".to_string(),
            title: "Replace litter".to_string(),
            completed: true,
            due_label: "Weekly · anytime".to_string(),
            due_day: None,
            due_month: None,
            due_year: None,
            time_minutes: 600,
            reward: 50,
            pet_id: PRIMARY_PET_ID.to_string(),
        }];

        let (_title, _) = reopen_completed_task(&mut profile, 0);
        assert_eq!(profile.paw_points, 0);
        assert_eq!(profile.parent_level, 1);
        assert_eq!(profile.parent_xp, 80);
    }

    #[test]
    fn tasks_tab_shows_pet_setup_prompt_when_onboarding_incomplete() {
        let profile = default_profile("user@example.com");
        let prompt = render_tasks_tab_setup_prompt(&profile);
        assert!(prompt.contains("tasks-tab-setup-alert"));
        assert!(prompt.contains("tasks-tab-setup-trigger"));
        assert!(prompt.contains("pet-setup-trigger"));
        assert!(prompt.contains("Create your pet"));
    }

    #[test]
    fn tasks_tab_pet_setup_prompt_hidden_after_onboarding() {
        let mut profile = default_profile("user@example.com");
        profile.onboarding_completed = true;
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        assert!(render_tasks_tab_setup_prompt(&profile).is_empty());
    }

    #[test]
    fn health_tab_prompts_pet_setup_without_cat() {
        let profile = default_profile("user@example.com");
        let html = render_health_tab(&profile);
        assert!(html.contains("health-tab-setup-alert"));
        assert!(html.contains("health-tab-setup-trigger"));
        assert!(html.contains("pet-setup-trigger"));
        assert!(html.contains("Create your pet"));
        assert!(!html.contains("action=\"/home/vet-visit\""));
    }

    #[test]
    fn admin_dashboard_nav_link_only_when_admin_session() {
        let storage =
            Storage::open_at(std::env::temp_dir().join(format!("ww-admin-nav-{}", Uuid::new_v4())))
                .expect("storage");
        let state = AppState { storage };
        let jar = CookieJar::new();
        assert_eq!(admin_dashboard_nav_link(&state, &jar), "");

        let jar = create_admin_session(&state, jar);
        assert!(admin_dashboard_nav_link(&state, &jar).contains("/admin"));
    }

    #[tokio::test]
    async fn forum_post_redirect_uses_see_other_not_temporary() {
        let state = routing_test_state();
        let jar = create_user_session(&state, CookieJar::new(), "forum-user@example.com");
        let response = forum_post_submit(
            State(state),
            jar,
            Form(ForumPostForm {
                title: "How often to brush?".to_string(),
                body: "Longhair cat hates brushing.".to_string(),
                breed_slug: String::new(),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response_location(response);
        assert!(location.contains("tab=forum"));
        assert!(location.contains("status=forum_post_sent"));
        assert!(location.contains("thread="));
    }

    #[test]
    fn forum_tab_renders_ask_form_and_threads() {
        let storage =
            Storage::open_at(std::env::temp_dir().join(format!("ww-forum-tab-{}", Uuid::new_v4())))
                .expect("storage");
        let state = AppState { storage };

        let post_id = state
            .storage
            .create_forum_post(
                "user@test.local",
                "catmom",
                "Best brush for longhair?",
                "My cat hates brushing.",
                "persian",
                1_700_000_000,
            )
            .expect("create post");

        let profile = test_profile_weeks(52, "indoor");
        let html = render_dashboard_forum_tab(
            &state,
            &profile,
            Some(post_id),
            "user@test.local",
            "forum",
            None,
        );
        assert!(html.contains("Community"));
        assert!(html.contains("Ask a question"));
        assert!(html.contains("Best brush for longhair?"));
        assert!(html.contains(&format!(r#"data-post-id="{post_id}""#)));
        assert!(html.contains("Post reply"));
        assert!(html.contains(r#"aria-label="Delete question""#));
        assert!(html.contains("forum-delete-minus"));
        assert!(html.contains("forum-breed-badge"));

        let other_view = render_dashboard_forum_tab(
            &state,
            &profile,
            Some(post_id),
            "other@test.local",
            "forum",
            None,
        );
        assert!(!other_view.contains(r#"aria-label="Delete question""#));
    }

    #[test]
    fn community_cat_feed_lists_other_visible_pets() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-community-feed-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };

        let mut viewer = test_profile_weeks(52, "indoor");
        viewer.email = "viewer@test.local".to_string();
        viewer.pet_name = "Pepper".to_string();
        viewer.community_visible = true;
        viewer.pet_photo_url = Some("/uploads/pepper.jpg".to_string());
        viewer.pet_video_url = Some("/uploads/pepper-playing.mp4".to_string());
        state.storage.save_profile(&viewer).expect("save viewer");

        let mut mochi = test_profile_weeks(52, "indoor");
        mochi.email = "mochi@test.local".to_string();
        mochi.pet_name = "Mochi".to_string();
        mochi.pet_breed = "Persian".to_string();
        mochi.parent_level = 10;
        mochi.care_streak_days = 7;
        mochi.community_visible = true;
        state.storage.save_profile(&mochi).expect("save mochi");

        let mut hidden = test_profile_weeks(52, "indoor");
        hidden.email = "hidden@test.local".to_string();
        hidden.pet_name = "Shadow".to_string();
        hidden.community_visible = false;
        state.storage.save_profile(&hidden).expect("save hidden");

        let html =
            render_dashboard_forum_tab(&state, &viewer, None, "viewer@test.local", "cats", None);
        assert!(html.contains("Community cats"));
        assert!(html.contains("Mochi"));
        assert!(html.contains("Pepper"));
        assert!(html.contains("community-cat-you-badge"));
        assert!(html.contains("/uploads/pepper.jpg"));
        assert!(html.contains("/uploads/pepper-playing.mp4"));
        assert!(html.contains("community-cat-media-toggle"));
        assert!(html.contains("Parent level 10"));
        assert!(!html.contains("Shadow"));
    }

    #[test]
    fn community_friends_posts_tab_shows_only_friend_questions() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-community-friends-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };

        let owner = User {
            username: "owner".to_string(),
            first_name: "Owner".to_string(),
            last_name: "One".to_string(),
            email: "owner@test.local".to_string(),
            password: "secret".to_string(),
            created_at: 1,
        };
        let friend = User {
            username: "friend".to_string(),
            first_name: "Friend".to_string(),
            last_name: "Two".to_string(),
            email: "friend@test.local".to_string(),
            password: "secret".to_string(),
            created_at: 1,
        };
        let stranger = User {
            username: "stranger".to_string(),
            first_name: "Str".to_string(),
            last_name: "Anger".to_string(),
            email: "stranger@test.local".to_string(),
            password: "secret".to_string(),
            created_at: 1,
        };
        state.storage.save_user(&owner).expect("save owner");
        state.storage.save_user(&friend).expect("save friend");
        state.storage.save_user(&stranger).expect("save stranger");

        state
            .storage
            .create_friend_request("owner@test.local", "friend@test.local", 1)
            .expect("friend request");
        let incoming = state
            .storage
            .list_incoming_friend_requests("friend@test.local")
            .expect("incoming");
        state
            .storage
            .respond_friend_request(&incoming[0].id, "friend@test.local", true)
            .expect("accept friend");

        state
            .storage
            .create_forum_post(
                "friend@test.local",
                "friend",
                "Friend grooming tip",
                "How often do you brush?",
                "persian",
                1_700_000_000,
            )
            .expect("friend post");
        state
            .storage
            .create_forum_post(
                "stranger@test.local",
                "stranger",
                "Stranger grooming tip",
                "Ignore me.",
                "persian",
                1_700_000_001,
            )
            .expect("stranger post");

        let profile = test_profile_weeks(52, "indoor");
        let empty_friends = render_dashboard_forum_tab(
            &state,
            &profile,
            None,
            "loner@test.local",
            "friends",
            None,
        );
        assert!(empty_friends.contains("Friends' posts"));
        assert!(empty_friends.contains("community-friends-cta"));
        assert!(empty_friends.contains("Find cat friends on the Friends tab"));

        let html = render_dashboard_forum_tab(
            &state,
            &profile,
            None,
            "owner@test.local",
            "friends",
            None,
        );
        assert!(html.contains("Friends' posts"));
        assert!(html.contains("Friend grooming tip"));
        assert!(!html.contains("Stranger grooming tip"));
    }

    #[test]
    fn dashboard_admin_nav_placeholders_are_replaced() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-admin-nav-template-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };
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
    fn feedback_forum_renders_public_posts() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-feedback-forum-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };
        state
            .storage
            .save_feedback(&FeedbackSubmission {
                id: 0,
                name: "Cat Mom".to_string(),
                email: "catmom@example.com".to_string(),
                category: "idea".to_string(),
                message: "Add a treat counter".to_string(),
                submitted_at: 1_700_000_000,
                user_id: Some("catmom@example.com".to_string()),
                author_username: "catmom".to_string(),
            })
            .expect("save feedback");

        let html = render_feedback_forum(
            &state,
            "Cat Mom",
            "catmom@example.com",
            None,
            Some("catmom@example.com"),
        );
        assert!(html.contains("Feedback Forum"));
        assert!(html.contains("Community feedback"));
        assert!(html.contains("Add a treat counter"));
        assert!(html.contains("data-feedback-id="));
        assert!(html.contains("Post to forum"));
        assert!(html.contains("feedback-vote-btn"));
        assert!(html.contains("feedback-vote-up"));
    }

    #[test]
    fn purrfect_idea_reward_grants_points_once() {
        let storage =
            Storage::open_at(std::env::temp_dir().join(format!("ww-purrfect-{}", Uuid::new_v4())))
                .expect("storage");
        let state = AppState {
            storage: storage.clone(),
        };

        let author_email = "author@example.com";
        let author_profile = default_profile(author_email);
        state
            .storage
            .save_profile(&author_profile)
            .expect("save author profile");

        let post_id = state
            .storage
            .save_feedback(&FeedbackSubmission {
                id: 0,
                name: "Author".to_string(),
                email: author_email.to_string(),
                category: "idea".to_string(),
                message: "Purrfect idea".to_string(),
                submitted_at: 1_700_000_000,
                user_id: Some(author_email.to_string()),
                author_username: "Author".to_string(),
            })
            .expect("save feedback");

        for index in 0..5 {
            state
                .storage
                .cast_feedback_vote(post_id, &format!("voter{index}@example.com"), 1)
                .expect("upvote");
        }

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            maybe_grant_purrfect_idea_reward(&state, post_id, 5).await;
            maybe_grant_purrfect_idea_reward(&state, post_id, 5).await;
        });

        let updated = state
            .storage
            .load_profile(author_email)
            .expect("load profile")
            .expect("author profile");
        assert_eq!(updated.paw_points, PURRFECT_IDEA_REWARD);
        assert_eq!(updated.pending_purrfect_idea_ids, vec![post_id]);
        assert!(state
            .storage
            .feedback_reward_granted(post_id)
            .expect("reward flag"));
    }

    #[test]
    fn admin_feedback_list_renders_submissions_with_user_id() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-admin-feedback-{}", Uuid::new_v4())),
        )
        .expect("storage");
        storage
            .save_feedback(&FeedbackSubmission {
                id: 0,
                name: "Cat Mom".to_string(),
                email: "catmom@example.com".to_string(),
                category: "idea".to_string(),
                message: "Add a treat counter".to_string(),
                submitted_at: 1_700_000_000,
                user_id: Some("catmom@example.com".to_string()),
                author_username: "catmom".to_string(),
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
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-admin-gate-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };
        let jar = CookieJar::new();
        assert!(!admin_session_valid(&state, &jar));

        let jar = create_admin_session(&state, jar);
        assert!(admin_session_valid(&state, &jar));
    }

    fn routing_test_state() -> AppState {
        let storage =
            Storage::open_at(std::env::temp_dir().join(format!("ww-routing-{}", Uuid::new_v4())))
                .expect("storage");
        AppState { storage }
    }

    fn empty_dashboard_query() -> DashboardQuery {
        DashboardQuery {
            status: None,
            session_id: None,
            vet_followup: None,
            thread: None,
            feedback: None,
            cal_day: None,
            cal_month: None,
            cal_year: None,
            community: None,
            breed: None,
            add_cat: None,
            pet: None,
            pet_owner: None,
        }
    }

    #[test]
    fn resolve_calendar_view_uses_query_or_defaults() {
        assert_eq!(resolve_calendar_view(Some("8"), Some("2027")), (8, 2027));
        assert_eq!(
            resolve_calendar_view(Some("13"), Some("2027")),
            (current_calendar_month(), current_calendar_year())
        );
        assert_eq!(
            resolve_calendar_view(Some("0"), Some("2200")),
            (current_calendar_month(), current_calendar_year())
        );
    }

    fn response_location(response: Response) -> String {
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string()
    }

    #[tokio::test]
    async fn public_root_serves_marketing_homepage() {
        let state = routing_test_state();
        let response = index_page(State(state), CookieJar::new())
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let html = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(html.contains("Watch over your cat's health"));
        assert!(html.contains("id=\"features\""));
        assert!(!html.contains("Log In to WhiskerWatch"));
        assert_marketing_top_nav(&html, "/");
    }

    #[test]
    fn apply_auth_nav_link_replaces_login_and_legacy_placeholder() {
        let state = routing_test_state();
        let html = apply_auth_nav_link(
            "<nav>{{AUTH_NAV_LINK}}<a href=\"/login\">LOG IN</a></nav>",
            &state,
            &CookieJar::new(),
        );
        assert!(!html.contains("{{"));
        assert!(html.contains(r#"<a href="/login">LOG IN</a>"#));
    }

    #[tokio::test]
    async fn public_nav_routes_return_expected_status() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let cases = [
            ("/", StatusCode::OK),
            ("/login", StatusCode::OK),
            ("/signup", StatusCode::OK),
            ("/forgot-password", StatusCode::OK),
            ("/contact", StatusCode::OK),
            ("/feedback", StatusCode::OK),
            ("/breeds", StatusCode::OK),
            ("/breeds/persian", StatusCode::OK),
            ("/breeds/not-a-real-breed", StatusCode::NOT_FOUND),
            ("/sitemap.xml", StatusCode::OK),
            ("/robots.txt", StatusCode::OK),
            ("/home", StatusCode::SEE_OTHER),
            ("/index.html", StatusCode::PERMANENT_REDIRECT),
        ];

        for (path, expected) in cases {
            let state = routing_test_state();
            let uploads = state.storage.data_dir().join("uploads");
            let _ = std::fs::create_dir_all(&uploads);
            let app = build_app(state, uploads);
            let response = app
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), expected, "path {path}");
        }
    }

    #[tokio::test]
    async fn static_assets_served_when_cwd_is_not_project_root() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let nested = manifest_dir.join("target").join("static-asset-cwd-test");
        std::fs::create_dir_all(&nested).expect("nested cwd");
        std::env::set_current_dir(&nested).expect("chdir");

        let cases = [
            ("/styles.css", "text/css"),
            ("/images/logo.png", "image/png"),
        ];
        for (path, expected_type) in cases {
            let state = routing_test_state();
            let uploads = state.storage.data_dir().join("uploads");
            std::fs::create_dir_all(&uploads).expect("uploads");
            let app = build_app(state, uploads);
            let response = app
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "path {path}");
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            assert!(
                content_type.contains(expected_type),
                "path {path} content-type {content_type}"
            );
        }

        std::env::set_current_dir(&manifest_dir).expect("restore cwd");
        let _ = std::fs::remove_dir(nested);
    }

    #[tokio::test]
    async fn signed_in_root_redirects_to_dashboard() {
        let state = routing_test_state();
        let email = "guest@example.com".to_string();
        let jar = create_user_session(&state, CookieJar::new(), &email);
        let response = index_page(State(state), jar).await.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response_location(response), "/home");
    }

    #[tokio::test]
    async fn dashboard_without_session_redirects_to_public_home() {
        let state = routing_test_state();
        let response = dashboard_page(
            State(state),
            CookieJar::new(),
            Query(empty_dashboard_query()),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response_location(response), "/");
    }

    #[tokio::test]
    async fn logged_in_home_replaces_pet_tab_placeholders() {
        let state = routing_test_state();
        let jar = create_user_session(&state, CookieJar::new(), "user@example.com");
        let response = dashboard_page(State(state), jar, Query(empty_dashboard_query()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert_no_unreplaced_dashboard_placeholders(&html);
        assert!(!html.contains("{{PAW_POINTS_ICON}}"));
        assert!(html.contains("paw-points-icon.png"));
        assert!(html.contains(r#"data-tab="pet""#));
        assert!(html.contains("My Pet"));
        assert!(html.contains("Create a pet"));
        assert!(html.contains("Create your pet"));
        assert!(html.contains("calendar-pet-setup-alert"));
        assert!(html.contains("tasks-tab-setup-alert"));
        assert!(html.contains(r#"name="pet_photo""#));
        assert!(html.contains("Cat profile photo"));
        assert!(html.contains("pet-photo-fieldset-required"));
    }

    #[tokio::test]
    async fn task_toggle_accepts_urlencoded_json_request() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "toggle-http@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        assert!(refresh_profile_tasks(&mut profile));
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/tasks/toggle")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("accept", "application/json")
                    .header("cookie", cookie_header)
                    .body(Body::from("task_id=feed_breakfast"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["status"], "completed");

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        let feed = updated
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        assert!(feed.completed);
    }

    #[tokio::test]
    async fn task_toggle_accepts_admin_session_without_user_cookie() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = admin_email();
        ensure_admin_user_account(&state).expect("admin profile");
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        profile.pet_name = "Admin Cat".to_string();
        profile.pet_breed = "Maine Coon".to_string();
        profile.pet_age_years = Some(4);
        profile.pet_age_weeks = None;
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        profile.onboarding_completed = true;
        assert!(refresh_profile_tasks(&mut profile));
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_admin_session(&state, CookieJar::new());
        let cookie = jar
            .get(ADMIN_SESSION_COOKIE)
            .expect("admin session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/tasks/toggle")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("accept", "application/json")
                    .header("cookie", cookie_header)
                    .body(Body::from("task_id=feed_breakfast"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["status"], "completed");
    }

    #[tokio::test]
    async fn task_toggle_json_request_without_session_returns_unauthorized() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state, uploads);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/tasks/toggle")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("accept", "application/json")
                    .body(Body::from("task_id=feed_breakfast"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], false);
        assert_eq!(data["status"], "auth");
    }

    #[tokio::test]
    async fn task_time_update_accepts_urlencoded_json_request() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::util::ServiceExt;

        let state = routing_test_state();
        let email = "task-time-http@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        assert!(refresh_profile_tasks(&mut profile));
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let cookie = jar
            .get(USER_SESSION_COOKIE)
            .expect("session cookie should be set");
        let cookie_header = format!("{}={}", cookie.name(), cookie.value());

        let uploads = state.storage.data_dir().join("uploads");
        let _ = std::fs::create_dir_all(&uploads);
        let app = build_app(state.clone(), uploads);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/home/tasks/time")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("accept", "application/json")
                    .header("cookie", cookie_header)
                    .body(Body::from("task_id=feed_breakfast&task_time=09%3A30"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_html(response).await;
        let data: serde_json::Value = serde_json::from_str(&body).expect("json response");
        assert_eq!(data["ok"], true);
        assert_eq!(data["status"], "time_updated");

        let updated = state
            .storage
            .load_profile(email)
            .expect("load profile")
            .expect("profile");
        let feed = updated
            .tasks
            .iter()
            .find(|task| task.id == "feed_breakfast")
            .expect("feed");
        assert_eq!(feed.time_minutes, 9 * 60 + 30);
        assert_eq!(updated.care_schedule.feed_time_minutes, 9 * 60 + 30);
    }

    #[tokio::test]
    async fn logged_in_home_with_pet_includes_task_time_editor() {
        let state = routing_test_state();
        let email = "task-time-user@example.com";
        let mut profile = test_profile_weeks(52, "indoor");
        profile.email = email.to_string();
        assert!(refresh_profile_tasks(&mut profile));
        state.storage.save_profile(&profile).expect("save profile");

        let jar = create_user_session(&state, CookieJar::new(), email);
        let response = dashboard_page(State(state), jar, Query(empty_dashboard_query()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert!(html.contains(r#"id="task-time-modal""#));
        assert!(html.contains("task-time-btn"));
        assert!(html.contains(r#"data-task-id="feed_breakfast""#));
    }

    #[tokio::test]
    async fn logged_in_home_admin_session_replaces_all_placeholders() {
        let state = routing_test_state();
        let email = admin_email();
        let jar = create_user_session(&state, CookieJar::new(), &email);
        let jar = create_admin_session(&state, jar);
        let response = dashboard_page(State(state), jar, Query(empty_dashboard_query()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let html = response_html(response).await;
        assert_no_unreplaced_dashboard_placeholders(&html);
        assert!(html.contains(r#"data-tab="pet""#));
        assert!(html.contains("My Pet"));
        assert!(html.contains(r#"<a href="/admin">ADMIN</a>"#));
    }

    fn assert_no_unreplaced_dashboard_placeholders(html: &str) {
        assert!(
            !html.contains("{{"),
            "/home leaked template placeholders: {}",
            unreplaced_dashboard_placeholders(html)
        );
    }

    fn unreplaced_dashboard_placeholders(html: &str) -> String {
        let mut found = Vec::new();
        let mut rest = html;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            if let Some(end) = after.find("}}") {
                let mut token = String::from("{{");
                token.push_str(&after[..end]);
                token.push_str("}}");
                found.push(token);
                rest = &after[end + 2..];
            } else {
                break;
            }
        }
        found.join(", ")
    }

    #[tokio::test]
    async fn user_logout_redirects_to_public_home() {
        let state = routing_test_state();
        let response = user_logout(State(state), CookieJar::new())
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response_location(response), "/");
    }

    #[tokio::test]
    async fn admin_logout_redirects_to_public_home() {
        let state = routing_test_state();
        let jar = create_admin_session(&state, CookieJar::new());
        let response = admin_logout(State(state), jar).await.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response_location(response), "/");
    }

    fn assert_public_home_nav(html: &str, page: &str) {
        assert!(
            html.contains(r#"<a href="/">HOME</a>"#),
            "{page} HOME should link to the public index"
        );
        assert!(
            !html.contains(r#"<a href="/home">HOME</a>"#),
            "{page} HOME must not link to the auth-only dashboard"
        );
    }

    fn assert_marketing_top_nav(html: &str, page: &str) {
        for (label, href) in [
            ("HOME", r#"<a href="/">HOME</a>"#),
            ("FEATURES", r#"<a href="/#features">FEATURES</a>"#),
            ("LOG IN", r#"<a href="/login">LOG IN</a>"#),
            ("FEEDBACK", r#"<a href="/feedback">FEEDBACK</a>"#),
            ("CONTACT", r#"<a href="/contact">CONTACT</a>"#),
        ] {
            assert!(html.contains(href), "{page} nav missing {label} -> {href}");
        }
        assert!(
            !html.contains("{{"),
            "{page} must not leak template placeholders"
        );
    }

    async fn response_html(response: Response) -> String {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        String::from_utf8(body.to_vec()).expect("utf8")
    }

    #[test]
    fn auth_templates_home_links_to_public_index() {
        for name in [
            "login.html",
            "signup.html",
            "forgot-password.html",
            "reset-password.html",
        ] {
            let path = storage::path_in_project(format!("templates/{name}"));
            let html = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("could not read {}: {error}", path.display());
            });
            assert_public_home_nav(&html, name);
        }
    }

    #[tokio::test]
    async fn auth_pages_serve_public_home_nav_link() {
        let state = routing_test_state();

        let login = login_page(
            State(state.clone()),
            CookieJar::new(),
            Query(LoginQuery::default()),
        )
        .await
        .into_response();
        assert_eq!(login.status(), StatusCode::OK);
        let login_html = response_html(login).await;
        assert_public_home_nav(&login_html, "login");
        assert_marketing_top_nav(&login_html, "login");

        let signup = signup_page(
            State(state.clone()),
            CookieJar::new(),
            Query(SignupQuery::default()),
        )
        .await
        .into_response();
        assert_eq!(signup.status(), StatusCode::OK);
        assert_public_home_nav(&response_html(signup).await, "signup");

        let forgot = forgot_password_page(
            State(state.clone()),
            CookieJar::new(),
            Query(ForgotPasswordQuery::default()),
        )
        .await
        .into_response();
        assert_eq!(forgot.status(), StatusCode::OK);
        assert_public_home_nav(&response_html(forgot).await, "forgot-password");

        let contact = contact_page(
            State(state.clone()),
            CookieJar::new(),
            Query(ContactQuery::default()),
        )
        .await
        .into_response();
        assert_eq!(contact.status(), StatusCode::OK);
        let contact_html = response_html(contact).await;
        assert!(!contact_html.contains("{{"));
        assert!(contact_html.contains(r#"<a href="/contact">CONTACT</a>"#));

        let feedback = feedback_page(
            State(state),
            CookieJar::new(),
            Query(FeedbackQuery::default()),
        )
        .await
        .into_response();
        assert_eq!(feedback.status(), StatusCode::OK);
        let feedback_html = response_html(feedback).await;
        assert!(!feedback_html.contains("{{"));
        assert!(feedback_html.contains(r#"<a href="/feedback">FEEDBACK</a>"#));
    }
}

async fn serve_static_no_cache(filename: &'static str, content_type: &'static str) -> Response {
    let path = storage::static_dir().join(filename);
    match fs::read(&path).await {
        Ok(body) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            body,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn build_app(state: AppState, uploads_dir: std::path::PathBuf) -> Router {
    Router::new()
        .route("/", get(index_page))
        .route("/breeds", get(public_breeds_index_page))
        .route("/breeds/{slug}", get(public_breed_guide_page))
        .route("/sitemap.xml", get(sitemap_page))
        .route("/robots.txt", get(robots_page))
        .route("/share/{token}", get(share_card_page))
        .route("/index.html", get(|| async { Redirect::permanent("/") }))
        .route("/home", get(dashboard_page))
        .route("/home/breeds", get(breed_select_page))
        .route("/home/breed-guides", get(breed_guides_shop_page))
        .route("/home/breed-guide/{slug}", get(breed_guide_page))
        .route("/home/breed-guides/checkout", post(breed_guide_checkout))
        .route("/home/premium/checkout", post(premium_checkout))
        .route("/home/pets/add", post(add_pet_submit))
        .route("/home/onboarding", post(onboarding_submit))
        .route("/home/pet-name", post(pet_name_submit))
        .route("/home/password", post(password_change_submit))
        .route("/home/pet-photo", post(pet_photo_submit))
        .route("/home/pet-video", post(pet_video_submit))
        .route("/home/pets/memorialize", post(memorialize_pet_submit))
        .route("/home/pets/memorial-comfort", post(memorial_comfort_submit))
        .route("/home/pets/memorial-video", post(memorial_video_submit))
        .route("/home/vet-visit", post(vet_visit_submit))
        .route("/home/vet-notes", post(vet_notes_submit))
        .route("/home/health/symptoms", post(symptom_check_submit))
        .route("/home/health/shelters", post(shelter_search_submit))
        .route("/home/outfits/buy", post(outfit_buy))
        .route("/home/outfits/equip", post(outfit_equip))
        .route("/home/tasks/toggle", post(task_toggle))
        .route("/home/tasks/add", post(task_add))
        .route("/home/tasks/delete", post(task_delete))
        .route("/home/tasks/time", post(task_time_update))
        .route("/home/streak", get(streak_keep_going_page))
        .route("/home/streak/claim", post(streak_reward_claim_submit))
        .route("/home/friends/request", post(friend_request_submit))
        .route("/home/friends/respond", post(friend_respond_submit))
        .route("/home/pets/share", post(pet_share_submit))
        .route("/home/pets/share/respond", post(pet_share_respond_submit))
        .route("/home/pets/share/revoke", post(pet_share_revoke_submit))
        .route("/home/calendar/event/new", get(calendar_event_form_page))
        .route("/home/calendar/event", post(calendar_event_add))
        .route("/home/cat-home", get(cat_home_page))
        .route("/home/paw-points", get(paw_points_balance))
        .route("/home/cat-home/pet-pet", post(pet_pet))
        .route("/home/decor/buy", post(decor_buy))
        .route("/home/decor/equip", post(decor_equip))
        .route("/home/tap-boosts/buy", post(tap_boost_buy))
        .route("/home/tap-boosts/equip", post(tap_boost_equip))
        .route("/home/petting-bonuses/buy", post(petting_bonus_buy))
        .route(
            "/home/petting-bonuses/activate",
            post(petting_bonus_activate),
        )
        .route(
            "/home/community/visibility",
            post(community_visibility_submit),
        )
        .route("/push/vapid-public-key", get(push_vapid_public_key))
        .route("/home/push/subscribe", post(push_subscribe))
        .route("/home/push/unsubscribe", post(push_unsubscribe))
        .route(
            "/home/notifications/preferences",
            post(notification_prefs_submit),
        )
        .route("/home/notifications/schedule", get(notifications_schedule))
        .route(
            "/home/onboarding-emails/preferences",
            post(onboarding_email_prefs_submit),
        )
        .route("/home/forum/post", post(forum_post_submit))
        .route("/home/forum/post/delete", post(forum_post_delete))
        .route("/home/forum/reply", post(forum_reply_submit))
        .route("/home/forum/reply/delete", post(forum_reply_delete))
        .route("/home/forum/{id}", get(forum_thread_redirect))
        .route("/home/paw-points/needed", get(paw_points_needed_page))
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
        .route("/feedback/vote", post(feedback_vote_submit))
        .route("/admin", get(admin_page))
        .route("/admin/logout", post(admin_logout))
        .route(
            "/login.html",
            get(|| async { Redirect::permanent("/login") }),
        )
        .route(
            "/signup.html",
            get(|| async { Redirect::permanent("/signup") }),
        )
        .route(
            "/contact.html",
            get(|| async { Redirect::permanent("/contact") }),
        )
        .route(
            "/feedback.html",
            get(|| async { Redirect::permanent("/feedback") }),
        )
        .route(
            "/styles.css",
            get(|| serve_static_no_cache("styles.css", "text/css; charset=utf-8")),
        )
        .route(
            "/dashboard.js",
            get(|| serve_static_no_cache("dashboard.js", "application/javascript; charset=utf-8")),
        )
        .route(
            "/alerts.js",
            get(|| serve_static_no_cache("alerts.js", "application/javascript; charset=utf-8")),
        )
        .route(
            "/paw-cursor.js",
            get(|| serve_static_no_cache("paw-cursor.js", "application/javascript; charset=utf-8")),
        )
        .route(
            "/feedback-forum.js",
            get(|| {
                serve_static_no_cache("feedback-forum.js", "application/javascript; charset=utf-8")
            }),
        )
        .route(
            "/calendar-event-form.js",
            get(|| {
                serve_static_no_cache(
                    "calendar-event-form.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/push-notifications.js",
            get(|| {
                serve_static_no_cache(
                    "push-notifications.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/sw.js",
            get(|| serve_static_no_cache("sw.js", "application/javascript; charset=utf-8")),
        )
        .route(
            "/pet-setup-draft.js",
            get(|| {
                serve_static_no_cache(
                    "pet-setup-draft.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/pet-color-picker.js",
            get(|| {
                serve_static_no_cache(
                    "pet-color-picker.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/pet-photo-framer.js",
            get(|| {
                serve_static_no_cache(
                    "pet-photo-framer.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/pet-video-framer.js",
            get(|| {
                serve_static_no_cache(
                    "pet-video-framer.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/breed-select-loading.js",
            get(|| {
                serve_static_no_cache(
                    "breed-select-loading.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/shop-affordance.js",
            get(|| {
                serve_static_no_cache(
                    "shop-affordance.js",
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/memorial.js",
            get(|| serve_static_no_cache("memorial.js", "application/javascript; charset=utf-8")),
        )
        .route(
            "/cinder-pet.js",
            get(|| serve_static_no_cache("cinder-pet.js", "application/javascript; charset=utf-8")),
        )
        .nest_service("/uploads", ServeDir::new(uploads_dir))
        .nest_service(
            "/images",
            ServeDir::new(storage::static_dir().join("images")),
        )
        .fallback_service(ServeDir::new(storage::static_dir()))
        .layer(DefaultBodyLimit::max(MAX_PET_VIDEO_BYTES))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let storage = Storage::open().unwrap_or_else(|error| {
        panic!("failed to open storage: {error:?}");
    });
    let uploads_dir = storage.data_dir().join("uploads");
    if let Err(error) = std::fs::create_dir_all(&uploads_dir) {
        eprintln!(
            "warning: could not create uploads directory {}: {error}",
            uploads_dir.display()
        );
    }
    let db_path = storage.db_path();
    eprintln!(
        "Using data directory: {} (database: {})",
        storage.data_dir().display(),
        db_path.display()
    );
    match storage.persisted_counts() {
        Ok((users, forum_posts, forum_replies, feedback, contacts)) => {
            eprintln!(
                "SQLite contains {users} users, {forum_posts} forum posts, {forum_replies} forum replies, {feedback} feedback entries, {contacts} contact messages"
            );
        }
        Err(error) => eprintln!("warning: could not read SQLite counts: {error}"),
    }
    let data_dir = storage.data_dir();
    let data_dir_env = std::env::var("DATA_DIR").ok();
    if data_dir_env
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        eprintln!(
            "WARNING: DATA_DIR is not set. User accounts, pet profiles, and paw points will be lost on redeploy."
        );
        eprintln!(
            "Tip: set DATA_DIR to a fixed absolute path (e.g. /data on Render with a persistent disk)."
        );
    } else if std::env::var("RENDER").ok().as_deref() == Some("true")
        && data_dir != PathBuf::from("/data")
    {
        eprintln!(
            "WARNING: Running on Render but DATA_DIR is {} (expected /data with a persistent disk). Profile data may not survive redeploys.",
            data_dir.display()
        );
    }

    let state = AppState { storage };
    push_notifications::spawn_dispatcher(state.clone());
    onboarding_emails::spawn_dispatcher(state.clone());

    let app = build_app(state, uploads_dir);

    let address = listen_address();
    let listener = TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind to {address}: {error}"));

    println!("WhiskerWatch running at http://{address}");
    println!(
        "Admin login: {} / (see ADMIN_PASSWORD env var)",
        admin_email()
    );
    axum::serve(listener, app)
        .await
        .expect("server failed unexpectedly");
}
