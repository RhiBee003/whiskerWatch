use chrono::{Datelike, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::UserProfile;

pub const DEFAULT_CHECKIN_MINUTES: u16 = 9 * 60;
const VET_ALERT_MINUTES: u16 = 10 * 60;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NotificationPrefs {
    #[serde(default = "default_pref_true")]
    pub enabled: bool,
    #[serde(default = "default_pref_true")]
    pub task_reminders: bool,
    #[serde(default = "default_pref_true")]
    pub vet_alerts: bool,
    #[serde(default = "default_pref_true")]
    pub daily_checkin: bool,
    #[serde(default = "default_checkin_minutes")]
    pub daily_checkin_minutes: u16,
}

fn default_pref_true() -> bool {
    true
}

fn default_checkin_minutes() -> u16 {
    DEFAULT_CHECKIN_MINUTES
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            task_reminders: true,
            vet_alerts: true,
            daily_checkin: true,
            daily_checkin_minutes: DEFAULT_CHECKIN_MINUTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingNotification {
    pub tag: String,
    pub title: String,
    pub body: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct ScheduledReminder {
    pub at: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub tag: String,
}

#[derive(Serialize)]
pub struct NotificationScheduleResponse {
    pub push_enabled: bool,
    pub reminders: Vec<ScheduledReminder>,
}

#[derive(Deserialize)]
pub struct PushSubscribeRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Deserialize)]
pub struct NotificationPrefsForm {
    #[serde(default)]
    pub enabled: String,
    #[serde(default)]
    pub task_reminders: String,
    #[serde(default)]
    pub vet_alerts: String,
    #[serde(default)]
    pub daily_checkin: String,
    pub daily_checkin_minutes: String,
}

pub fn push_configured() -> bool {
    std::env::var("VAPID_PUBLIC_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        && std::env::var("VAPID_PRIVATE_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
}

pub fn vapid_public_key() -> Option<String> {
    std::env::var("VAPID_PUBLIC_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn vapid_subject() -> String {
    std::env::var("VAPID_SUBJECT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("mailto:{}", crate::company_email()))
}

fn today_key() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

fn already_sent(profile: &UserProfile, tag: &str) -> bool {
    profile
        .notification_sent_dates
        .get(tag)
        .is_some_and(|date| date == &today_key())
}

fn mark_sent(profile: &mut UserProfile, tag: &str) {
    profile
        .notification_sent_dates
        .insert(tag.to_string(), today_key());
}

pub fn prune_old_notification_dates(profile: &mut UserProfile) {
    let today = today_key();
    profile
        .notification_sent_dates
        .retain(|_, date| date == &today);
    let _ = today;
}

fn current_minutes() -> u16 {
    let now = Local::now();
    (now.hour() as u16) * 60 + now.minute() as u16
}

fn minutes_match_target(current: u16, target: u16) -> bool {
    current == target
}

fn any_daily_task_completed_today(profile: &UserProfile, today: NaiveDate) -> bool {
    profile.tasks.iter().any(|task| {
        task.completed
            && crate::is_daily_reset_task(task)
            && task
                .due_year
                .is_some_and(|year| year == today.year() as u32)
            && task
                .due_month
                .is_some_and(|month| month == today.month())
            && task.due_day.is_some_and(|day| day == today.day())
    })
}

fn pet_display_name(profile: &UserProfile) -> String {
    let name = profile.pet_name.trim();
    if name.is_empty()
        || name.eq_ignore_ascii_case("your cat")
        || name.eq_ignore_ascii_case("no pet yet")
    {
        "your cat".to_string()
    } else {
        name.to_string()
    }
}

pub fn pending_notifications_for_profile(profile: &UserProfile) -> Vec<PendingNotification> {
    if !profile.notification_prefs.enabled || !crate::profile_has_pet(profile) {
        return Vec::new();
    }

    let today = Local::now().date_naive();
    let now_minutes = current_minutes();
    let pet = pet_display_name(profile);
    let mut pending = Vec::new();

    if profile.notification_prefs.task_reminders {
        for task in &profile.tasks {
            if task.completed {
                continue;
            }
            if !crate::is_daily_reset_task(task) && task.id != crate::VET_APPOINTMENT_TASK_ID {
                continue;
            }
            if !minutes_match_target(now_minutes, task.time_minutes) {
                continue;
            }

            let tag = format!("task:{}:{}", task.id, today_key());
            if already_sent(profile, &tag) {
                continue;
            }

            pending.push(PendingNotification {
                tag,
                title: format!("Care task for {pet}"),
                body: format!("Time for {} — earn paw points when you finish.", task.title),
                url: "/home?tab=tasks".to_string(),
            });
        }
    }

    if profile.notification_prefs.vet_alerts
        && crate::entitlements::can_access_health_records(
            profile.premium_unlocked,
            &profile.email,
        )
        && crate::needs_vet_appointment_asap(profile, today)
        && minutes_match_target(now_minutes, VET_ALERT_MINUTES)
    {
        let tag = format!("vet:alert:{}", today_key());
        if !already_sent(profile, &tag) {
            pending.push(PendingNotification {
                tag,
                title: "Vet visit reminder".to_string(),
                body: format!(
                    "{pet} may need a vet checkup soon. Log a visit in your Health tab."
                ),
                url: "/home?tab=health".to_string(),
            });
        }
    }

    if profile.notification_prefs.daily_checkin
        && minutes_match_target(now_minutes, profile.notification_prefs.daily_checkin_minutes)
        && !any_daily_task_completed_today(profile, today)
    {
        let tag = format!("daily:checkin:{}", today_key());
        if !already_sent(profile, &tag) {
            pending.push(PendingNotification {
                tag,
                title: format!("Good morning! {pet} is waiting"),
                body: "Complete a care task today to keep your streak and earn paw points.".to_string(),
                url: "/home?tab=tasks".to_string(),
            });
        }
    }

    pending
}

pub fn upcoming_reminders_for_profile(profile: &UserProfile) -> Vec<ScheduledReminder> {
    if !profile.notification_prefs.enabled || !crate::profile_has_pet(profile) {
        return Vec::new();
    }

    let today = Local::now().date_naive();
    let now = Local::now();
    let now_minutes = current_minutes();
    let pet = pet_display_name(profile);
    let mut reminders = Vec::new();

    let push_one = |reminders: &mut Vec<ScheduledReminder>,
                    minutes: u16,
                    tag: &str,
                    title: String,
                    body: String,
                    url: &str| {
        if minutes <= now_minutes {
            return;
        }
        if already_sent(profile, tag) {
            return;
        }
        let hour = minutes / 60;
        let minute = minutes % 60;
        let at = today
            .and_hms_opt(hour as u32, minute as u32, 0)
            .map(|naive| {
                naive
                    .and_local_timezone(Local)
                    .latest()
                    .unwrap_or(now)
                    .to_rfc3339()
            })
            .unwrap_or_else(|| now.to_rfc3339());
        reminders.push(ScheduledReminder {
            at,
            title,
            body,
            url: url.to_string(),
            tag: tag.to_string(),
        });
    };

    if profile.notification_prefs.task_reminders {
        for task in &profile.tasks {
            if task.completed {
                continue;
            }
            if !crate::is_daily_reset_task(task) && task.id != crate::VET_APPOINTMENT_TASK_ID {
                continue;
            }
            let tag = format!("task:{}:{}", task.id, today_key());
            push_one(
                &mut reminders,
                task.time_minutes,
                &tag,
                format!("Care task for {pet}"),
                format!("Time for {} — earn paw points when you finish.", task.title),
                "/home?tab=tasks",
            );
        }
    }

    if profile.notification_prefs.vet_alerts
        && crate::entitlements::can_access_health_records(
            profile.premium_unlocked,
            &profile.email,
        )
        && crate::needs_vet_appointment_asap(profile, today)
    {
        let tag = format!("vet:alert:{}", today_key());
        push_one(
            &mut reminders,
            VET_ALERT_MINUTES,
            &tag,
            "Vet visit reminder".to_string(),
            format!("{pet} may need a vet checkup soon."),
            "/home?tab=health",
        );
    }

    if profile.notification_prefs.daily_checkin
        && !any_daily_task_completed_today(profile, today)
    {
        let tag = format!("daily:checkin:{}", today_key());
        push_one(
            &mut reminders,
            profile.notification_prefs.daily_checkin_minutes,
            &tag,
            format!("Good morning! {pet} is waiting"),
            "Complete a care task today to keep your streak and earn paw points.".to_string(),
            "/home?tab=tasks",
        );
    }

    reminders.sort_by(|left, right| left.at.cmp(&right.at));
    reminders
}

pub fn render_account_notifications_section(profile: &UserProfile) -> String {
    let push_ready = push_configured();
    let status = if push_ready {
        "Enable browser notifications to get task reminders, vet alerts, and daily check-ins even when WhiskerWatch is closed."
    } else {
        "Push delivery requires VAPID keys on the server. You can still get in-browser reminders while this tab is open."
    };

    let enabled = checkbox(profile.notification_prefs.enabled);
    let task_reminders = checkbox(profile.notification_prefs.task_reminders);
    let vet_alerts = checkbox(profile.notification_prefs.vet_alerts);
    let daily_checkin = checkbox(profile.notification_prefs.daily_checkin);
    let checkin_time = format_time_value(profile.notification_prefs.daily_checkin_minutes);

    format!(
        r##"<article class="dashboard-card push-notifications-card" id="push-notifications-card">
  <h2>Push notifications</h2>
  <p class="field-hint">{status}</p>
  <div class="push-enable-row">
    <button type="button" class="download-btn" id="push-enable-btn">Enable notifications</button>
    <span class="push-status-pill" id="push-status-pill" hidden></span>
  </div>
  <form class="login-form notification-prefs-form" action="/home/notifications/preferences" method="post">
    <label class="checkbox-pill"><input type="checkbox" name="enabled" value="on"{enabled} /> Send notifications</label>
    <label class="checkbox-pill"><input type="checkbox" name="task_reminders" value="on"{task_reminders} /> Task reminders at scheduled times</label>
    <label class="checkbox-pill"><input type="checkbox" name="vet_alerts" value="on"{vet_alerts} /> Vet appointment alerts (WhiskerWatch Plus)</label>
    <label class="checkbox-pill"><input type="checkbox" name="daily_checkin" value="on"{daily_checkin} /> Daily morning check-in</label>
    <label for="daily_checkin_minutes">Daily check-in time</label>
    <input id="daily_checkin_minutes" name="daily_checkin_minutes" type="time" value="{checkin_time}" />
    <button type="submit" class="download-btn login-submit">Save notification settings</button>
  </form>
</article>"##,
        status = status,
        enabled = enabled,
        task_reminders = task_reminders,
        vet_alerts = vet_alerts,
        daily_checkin = daily_checkin,
        checkin_time = checkin_time,
    )
}

fn checkbox(enabled: bool) -> &'static str {
    if enabled {
        " checked"
    } else {
        ""
    }
}

fn format_time_value(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

pub fn parse_checkin_minutes_input(value: &str) -> Option<u16> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(DEFAULT_CHECKIN_MINUTES);
    }
    let (hour, minute) = trimmed.split_once(':')?;
    let hour: u32 = hour.parse().ok()?;
    let minute: u32 = minute.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour as u16) * 60 + minute as u16)
}

pub fn apply_notification_prefs_form(
    profile: &mut UserProfile,
    form: &NotificationPrefsForm,
) -> Result<(), ()> {
    let checkin = parse_checkin_minutes_input(&form.daily_checkin_minutes).ok_or(())?;
    profile.notification_prefs = NotificationPrefs {
        enabled: form.enabled == "on",
        task_reminders: form.task_reminders == "on",
        vet_alerts: form.vet_alerts == "on",
        daily_checkin: form.daily_checkin == "on",
        daily_checkin_minutes: checkin,
    };
    Ok(())
}

pub async fn send_web_push(
    client: &web_push::IsahcWebPushClient,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    notification: &PendingNotification,
) -> Result<(), String> {
    use web_push::{ContentEncoding, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder};

    if !push_configured() {
        return Err("push not configured".to_string());
    }

    let private_key = std::env::var("VAPID_PRIVATE_KEY").map_err(|_| "missing private key")?;
    let subscription = web_push::SubscriptionInfo::new(endpoint, p256dh, auth);

    let mut sig_builder = VapidSignatureBuilder::from_pem(
        Cursor::new(private_key.as_bytes()),
        &subscription,
    )
    .map_err(|e| format!("{e:?}"))?;
    sig_builder.add_claim("sub", vapid_subject());
    let vapid_signature = sig_builder.build().map_err(|e| format!("{e:?}"))?;

    let payload = serde_json::json!({
        "title": notification.title,
        "body": notification.body,
        "url": notification.url,
        "tag": notification.tag,
    });
    let payload_bytes = payload.to_string();

    let mut builder = WebPushMessageBuilder::new(&subscription);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload_bytes.as_bytes());
    builder.set_vapid_signature(vapid_signature);
    if !notification.tag.is_empty() {
        let topic: String = notification.tag.chars().take(32).collect();
        builder.set_topic(topic);
    }

    client
        .send(builder.build().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;

    Ok(())
}

pub fn spawn_dispatcher(state: crate::AppState) {
    if !push_configured() {
        eprintln!(
            "Push dispatcher disabled: set VAPID_PUBLIC_KEY and VAPID_PRIVATE_KEY to enable web push."
        );
        return;
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(error) = dispatch_all(&state).await {
                eprintln!("push dispatch error: {error}");
            }
        }
    });
}

async fn dispatch_all(state: &crate::AppState) -> Result<(), String> {
    let client = web_push::IsahcWebPushClient::new().map_err(|e| format!("{e:?}"))?;
    let subscriptions = state
        .storage
        .list_push_subscriptions()
        .map_err(|e| format!("{e:?}"))?;

    for subscription in subscriptions {
        let mut profile = match state.storage.load_profile(&subscription.email) {
            Ok(Some(profile)) => profile,
            _ => continue,
        };

        prune_old_notification_dates(&mut profile);
        let pending = pending_notifications_for_profile(&profile);
        if pending.is_empty() {
            continue;
        }

        let mut changed = false;
        for notification in pending {
            match send_web_push(
                &client,
                &subscription.endpoint,
                &subscription.p256dh,
                &subscription.auth,
                &notification,
            )
            .await
            {
                Ok(()) => {
                    mark_sent(&mut profile, &notification.tag);
                    changed = true;
                }
                Err(error) => {
                    if error.contains("404") || error.contains("410") {
                        let _ = state
                            .storage
                            .delete_push_subscription(&subscription.endpoint);
                    }
                    eprintln!(
                        "push to {} failed: {error}",
                        subscription.email
                    );
                }
            }
        }

        if changed {
            let _ = state.storage.save_profile(&profile);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_profile;

    #[test]
    fn pending_task_reminder_fires_at_task_time() {
        let mut profile = default_profile("user@example.com");
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Persian".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        profile.tasks.push(crate::UserTask {
            id: "feed_breakfast".to_string(),
            title: "Morning feeding".to_string(),
            completed: false,
            due_label: "Today · 8:00 AM".to_string(),
            due_day: Some(Local::now().day()),
            due_month: Some(Local::now().month()),
            due_year: Some(Local::now().year() as u32),
            time_minutes: current_minutes(),
            reward: 10,
        });

        let pending = pending_notifications_for_profile(&profile);
        assert_eq!(pending.len(), 1);
        assert!(pending[0].body.contains("Morning feeding"));
    }

    #[test]
    fn skips_task_reminder_when_already_completed() {
        let mut profile = default_profile("user@example.com");
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Persian".to_string();
        profile.pet_age_years = Some(2);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        profile.tasks.push(crate::UserTask {
            id: "feed_breakfast".to_string(),
            title: "Morning feeding".to_string(),
            completed: true,
            due_label: "Today · 8:00 AM".to_string(),
            due_day: Some(Local::now().day()),
            due_month: Some(Local::now().month()),
            due_year: Some(Local::now().year() as u32),
            time_minutes: current_minutes(),
            reward: 10,
        });

        assert!(pending_notifications_for_profile(&profile).is_empty());
    }

    #[test]
    fn parse_checkin_minutes_from_time_input() {
        assert_eq!(parse_checkin_minutes_input("09:30"), Some(570));
        assert!(parse_checkin_minutes_input("25:00").is_none());
    }
}
