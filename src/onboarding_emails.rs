use crate::email_delivery::{send_email, OutboundEmail};
use crate::UserProfile;
use serde::Deserialize;

const SECONDS_PER_HOUR: u64 = 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnboardingEmailStep {
    pub id: &'static str,
    pub delay_hours: u64,
}

pub const ONBOARDING_SEQUENCE: &[OnboardingEmailStep] = &[
    OnboardingEmailStep {
        id: "welcome",
        delay_hours: 0,
    },
    OnboardingEmailStep {
        id: "day1_tasks",
        delay_hours: 24,
    },
    OnboardingEmailStep {
        id: "day3_cat_home",
        delay_hours: 72,
    },
    OnboardingEmailStep {
        id: "day5_health",
        delay_hours: 120,
    },
    OnboardingEmailStep {
        id: "day7_stick",
        delay_hours: 168,
    },
];

#[derive(Deserialize)]
pub struct OnboardingEmailPrefsForm {
    #[serde(default)]
    pub onboarding_emails_enabled: String,
}

pub fn due_onboarding_email_ids(
    signup_timestamp: u64,
    now: u64,
    already_sent: &[String],
    enabled: bool,
) -> Vec<&'static str> {
    if !enabled {
        return Vec::new();
    }

    let age_secs = now.saturating_sub(signup_timestamp);
    let sent: std::collections::HashSet<&str> = already_sent.iter().map(String::as_str).collect();

    ONBOARDING_SEQUENCE
        .iter()
        .filter(|step| {
            !sent.contains(step.id) && age_secs >= step.delay_hours.saturating_mul(SECONDS_PER_HOUR)
        })
        .map(|step| step.id)
        .collect()
}

fn display_first_name(first_name: &str) -> String {
    let trimmed = first_name.trim();
    if trimmed.is_empty() {
        "there".to_string()
    } else {
        trimmed.to_string()
    }
}

fn pet_label(profile: &UserProfile) -> String {
    let name = profile.pet_name.trim();
    if name.is_empty()
        || name.eq_ignore_ascii_case("your cat")
        || name.eq_ignore_ascii_case("add your cat's details")
        || name.eq_ignore_ascii_case("no pet yet")
    {
        "your cat".to_string()
    } else {
        name.to_string()
    }
}

fn app_url(path: &str) -> String {
    let base = crate::public_base_url();
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

fn email_shell(
    first_name: &str,
    headline: &str,
    body_html: &str,
    cta_label: &str,
    cta_url: &str,
) -> OutboundEmail {
    let greeting = display_first_name(first_name);
    let html_body = format!(
        r#"<!DOCTYPE html>
<html>
  <body style="margin:0;padding:0;background:#fff7fb;font-family:Georgia,'Times New Roman',serif;color:#3d2a36;">
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#fff7fb;padding:24px 12px;">
      <tr>
        <td align="center">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px;background:#ffffff;border-radius:18px;padding:28px 24px;border:1px solid #f2d9e4;">
            <tr><td style="font-size:28px;line-height:1.2;color:#c45c8a;padding-bottom:8px;">WhiskerWatch 🐾</td></tr>
            <tr><td style="font-size:20px;line-height:1.35;color:#3d2a36;padding-bottom:12px;">Hi {greeting}, {headline}</td></tr>
            <tr><td style="font-size:16px;line-height:1.6;color:#5a4550;padding-bottom:20px;">{body_html}</td></tr>
            <tr><td>
              <a href="{cta_url}" style="display:inline-block;background:#e8899e;color:#ffffff;text-decoration:none;padding:12px 20px;border-radius:999px;font-size:16px;font-weight:bold;">{cta_label}</a>
            </td></tr>
            <tr><td style="font-size:13px;line-height:1.5;color:#8a7480;padding-top:24px;">You are receiving this because you joined WhiskerWatch. Turn these tips off anytime on your Account tab.</td></tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#,
        greeting = greeting,
        headline = headline,
        body_html = body_html,
        cta_label = cta_label,
        cta_url = cta_url,
    );

    let text_body = format!(
        "Hi {greeting},\n\n{headline}\n\n{}\n\n{cta_label}: {cta_url}\n\n— WhiskerWatch",
        strip_html_tags(body_html),
        cta_label = cta_label,
        cta_url = cta_url,
    );

    OutboundEmail {
        to: String::new(),
        subject: String::new(),
        html_body,
        text_body,
    }
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_email(step_id: &str, first_name: &str, profile: &UserProfile) -> OutboundEmail {
    let pet = pet_label(profile);
    let has_pet = crate::profile_has_pet(profile);

    let (subject, headline, body_html, cta_label, cta_path) = match step_id {
        "welcome" => (
            "Welcome to WhiskerWatch — let's meet your cat",
            "welcome aboard!",
            if has_pet {
                format!(
                    "<p>Thanks for joining WhiskerWatch. {pet} is already on your dashboard — explore care tasks, paw points, and your personalized pet tab.</p><p>Your first week is all about building a simple daily rhythm. We will send a few short guides to help you get the most out of every tab.</p>"
                )
            } else {
                "<p>Thanks for joining WhiskerWatch. The best first step is telling us about your cat so we can personalize feeding reminders, care tasks, and health tips.</p><p>It only takes a minute — add a name, breed, and age to unlock your dashboard.</p>".to_string()
            },
            if has_pet { "Open your dashboard" } else { "Set up your cat" },
            "/home",
        ),
        "day1_tasks" => (
            "Day 1: earn paw points with care tasks",
            "ready for your first care win?",
            format!(
                "<p>Care tasks are the heart of WhiskerWatch. Complete feeding, water, play, and litter routines for {pet} to earn paw points and parent XP.</p><p>Tip: tap any task time to adjust it to your real schedule. Finish one task today to start your care streak.</p>"
            ),
            "View care tasks",
            "/home?tab=tasks",
        ),
        "day3_cat_home" => (
            "Day 3: visit Cat Home and the outfit shop",
            "time to play in Cat Home!",
            format!(
                "<p>Cat Home is where {pet} comes to life. Pet your cat for bonus paw points, decorate their room, and spend points in the outfit shop.</p><p>Try a daily check-in on the My Pet tab, then hop to Cat Home to see your cat's mood update.</p>"
            ),
            "Visit Cat Home",
            "/home/cat-home",
        ),
        "day5_health" => (
            "Day 5: track health and build your streak",
            "keep the momentum going.",
            format!(
                "<p>Log vet visits and notes in the Health tab so {pet}'s care history stays in one place. WhiskerWatch Plus unlocks breed guides and smarter vet reminders.</p><p>Your care streak counts daily task completions — hit 7 days for a shareable milestone card.</p>"
            ),
            "Open Health tab",
            "/home?tab=health",
        ),
        "day7_stick" => (
            "Day 7: you are building a great habit",
            "one week in — nice work!",
            format!(
                "<p>You have been caring for {pet} for a week. Enable push notifications on your Account tab for task reminders and morning check-ins.</p><p>Explore the Community tab to see other cats and ask breed questions. Small daily wins beat perfect weeks — keep showing up.</p>"
            ),
            "Open your dashboard",
            "/home?tab=account",
        ),
        _ => (
            "WhiskerWatch tips for your cat",
            "a quick tip from WhiskerWatch.",
            "<p>Jump back into your dashboard to keep your care routine going.</p>".to_string(),
            "Open dashboard",
            "/home",
        ),
    };

    let mut email = email_shell(
        first_name,
        headline,
        &body_html,
        cta_label,
        &app_url(cta_path),
    );
    email.subject = subject.to_string();
    email
}

pub fn render_account_onboarding_emails_section(profile: &UserProfile) -> String {
    let checked = if profile.onboarding_emails_enabled {
        " checked"
    } else {
        ""
    };

    format!(
        r##"<article class="dashboard-card onboarding-emails-card">
  <h2>Onboarding tips by email</h2>
  <p class="field-hint">We send a short welcome series during your first week to help you discover care tasks, Cat Home, health tracking, and more.</p>
  <form class="login-form onboarding-email-prefs-form" action="/home/onboarding-emails/preferences" method="post">
    <label class="checkbox-pill"><input type="checkbox" name="onboarding_emails_enabled" value="on"{checked} /> Send week-one onboarding emails</label>
    <button type="submit" class="download-btn login-submit">Save email preferences</button>
  </form>
</article>"##,
        checked = checked,
    )
}

pub async fn try_send_due_for_email(
    state: &crate::AppState,
    email: &str,
    first_name: &str,
    signup_timestamp: u64,
) -> Result<(), String> {
    if crate::is_admin_account(email) {
        return Ok(());
    }

    let mut profile = match state.storage.load_profile(email) {
        Ok(Some(profile)) => profile,
        Ok(None) => crate::default_profile(email),
        Err(error) => return Err(format!("{error}")),
    };

    let now = crate::timestamp_now();
    let due = due_onboarding_email_ids(
        signup_timestamp,
        now,
        &profile.onboarding_emails_sent,
        profile.onboarding_emails_enabled,
    );

    if due.is_empty() {
        return Ok(());
    }

    let mut changed = false;
    for step_id in due {
        let mut outbound = build_email(step_id, first_name, &profile);
        outbound.to = email.to_string();
        send_email(&outbound).await?;
        profile.onboarding_emails_sent.push(step_id.to_string());
        changed = true;
        eprintln!("onboarding email sent: {step_id} -> {email}");
    }

    if changed {
        state
            .storage
            .save_profile(&profile)
            .map_err(|error| format!("{error}"))?;
    }

    Ok(())
}

pub fn spawn_dispatcher(state: crate::AppState) {
    if !crate::email_delivery::smtp_configured() {
        eprintln!(
            "Onboarding email dispatcher running in dev-log mode (set SMTP_HOST to deliver real email)."
        );
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15 * 60));
        loop {
            interval.tick().await;
            if let Err(error) = dispatch_all(&state).await {
                eprintln!("onboarding email dispatch error: {error}");
            }
        }
    });
}

async fn dispatch_all(state: &crate::AppState) -> Result<(), String> {
    let users = state
        .storage
        .load_users()
        .map_err(|error| format!("{error}"))?;
    let now = crate::timestamp_now();

    for user in users {
        if crate::is_admin_account(&user.email) {
            continue;
        }

        let due = {
            let profile = match state.storage.load_profile(&user.email) {
                Ok(Some(profile)) => profile,
                Ok(None) => crate::default_profile(&user.email),
                Err(error) => {
                    eprintln!(
                        "onboarding email profile load failed for {}: {error}",
                        user.email
                    );
                    continue;
                }
            };

            due_onboarding_email_ids(
                user.created_at,
                now,
                &profile.onboarding_emails_sent,
                profile.onboarding_emails_enabled,
            )
        };

        if due.is_empty() {
            continue;
        }

        if let Err(error) =
            try_send_due_for_email(state, &user.email, &user.first_name, user.created_at).await
        {
            eprintln!("onboarding email failed for {}: {error}", user.email);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_email_due_immediately() {
        let due = due_onboarding_email_ids(1_000, 1_500, &[], true);
        assert_eq!(due, vec!["welcome"]);
    }

    #[test]
    fn day1_not_due_before_24_hours() {
        let due =
            due_onboarding_email_ids(0, 23 * SECONDS_PER_HOUR, &["welcome".to_string()], true);
        assert!(due.is_empty());
    }

    #[test]
    fn day1_due_after_24_hours() {
        let due =
            due_onboarding_email_ids(0, 24 * SECONDS_PER_HOUR, &["welcome".to_string()], true);
        assert_eq!(due, vec!["day1_tasks"]);
    }

    #[test]
    fn skips_disabled_and_already_sent() {
        assert!(due_onboarding_email_ids(0, 10_000_000, &[], false).is_empty());
        let due = due_onboarding_email_ids(
            0,
            10_000_000,
            &[
                "welcome".to_string(),
                "day1_tasks".to_string(),
                "day3_cat_home".to_string(),
                "day5_health".to_string(),
                "day7_stick".to_string(),
            ],
            true,
        );
        assert!(due.is_empty());
    }

    #[test]
    fn build_welcome_email_mentions_pet_setup_when_missing() {
        let profile = crate::default_profile("new@example.com");
        let email = build_email("welcome", "Alex", &profile);
        assert!(email.subject.contains("Welcome"));
        assert!(email.html_body.contains("telling us about your cat"));
    }
}
