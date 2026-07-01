use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{vet_care, PetSnapshot, UserProfile};

const RECHECK_COOLDOWN_DAYS: i64 = 7;
const MAX_HISTORY_PER_PET: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PetWeightRecord {
    pub weight_lbs: f32,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HomeHealthCheckEntry {
    pub checked_at: String,
    pub weight_lbs: Option<f32>,
    pub appetite: String,
    pub energy: String,
    pub litter_habits: String,
    pub drinking: String,
    pub grooming_mood: String,
    pub discomfort: bool,
    pub outcome: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Reassuring,
    Monitor,
    VetSoon,
}

impl CheckOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reassuring => "reassuring",
            Self::Monitor => "monitor",
            Self::VetSoon => "vet_soon",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Reassuring => "Looks reassuring today",
            Self::Monitor => "Watch closely",
            Self::VetSoon => "Call your vet soon",
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            Self::Reassuring => "home-health-outcome-reassuring",
            Self::Monitor => "home-health-outcome-monitor",
            Self::VetSoon => "home-health-outcome-vet-soon",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HomeHealthCheckResult {
    pub outcome: CheckOutcome,
    pub summary: String,
    pub detail: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct HomeHealthCheckForm {
    pub pet_id: String,
    pub weight_lbs: Option<String>,
    pub appetite: String,
    pub energy: String,
    pub litter_habits: String,
    pub drinking: String,
    pub grooming_mood: String,
    pub discomfort: Option<String>,
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
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

fn pet_name(snapshot: &PetSnapshot) -> String {
    let name = snapshot.pet_name.trim();
    if name.is_empty() {
        "your cat".to_string()
    } else {
        name.to_string()
    }
}

pub fn checkup_overdue(snapshot: &PetSnapshot, today: NaiveDate) -> bool {
    let plan = vet_care::analyze(snapshot, today);
    plan.overdue_items.iter().any(|item| {
        let label = item.label.to_lowercase();
        label.contains("wellness")
            || label.contains("medication")
            || label.contains("follow")
    })
}

fn days_since_last_check(profile: &UserProfile, pet_id: &str, today: NaiveDate) -> Option<i64> {
    profile
        .home_health_checks
        .get(pet_id)
        .and_then(|entries| entries.last())
        .and_then(|entry| parse_date(&entry.checked_at))
        .map(|date| (today - date).num_days())
}

pub fn should_offer_check(snapshot: &PetSnapshot, profile: &UserProfile, today: NaiveDate) -> bool {
    if snapshot.deceased || !checkup_overdue(snapshot, today) {
        return false;
    }
    days_since_last_check(profile, &snapshot.id, today)
        .is_none_or(|days| days >= RECHECK_COOLDOWN_DAYS)
}

fn parse_weight(value: Option<&str>) -> Option<f32> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let parsed = raw.parse::<f32>().ok()?;
    if parsed <= 0.0 || parsed > 45.0 {
        return None;
    }
    Some(parsed)
}

pub fn evaluate(
    form: &HomeHealthCheckForm,
    previous_weight: Option<f32>,
    pet_name: &str,
) -> Result<HomeHealthCheckResult, &'static str> {
    let weight = parse_weight(form.weight_lbs.as_deref());
    if form.weight_lbs.as_deref().is_some_and(|value| !value.trim().is_empty()) && weight.is_none()
    {
        return Err("invalid_weight");
    }

    let mut concern = 0u8;
    let mut flags = Vec::new();

    match form.appetite.as_str() {
        "none" => {
            concern += 3;
            flags.push("Not eating".to_string());
        }
        "less" => {
            concern += 1;
            flags.push("Eating less than usual".to_string());
        }
        "more" => {
            concern += 1;
            flags.push("Eating more than usual".to_string());
        }
        _ => {}
    }

    match form.energy.as_str() {
        "low" => {
            concern += 2;
            flags.push("Lower energy".to_string());
        }
        "high" => {
            concern += 1;
            flags.push("Very restless or hyper".to_string());
        }
        _ => {}
    }

    match form.litter_habits.as_str() {
        "straining" => {
            concern += 4;
            flags.push("Straining or crying in the litter box".to_string());
        }
        "accidents" => {
            concern += 3;
            flags.push("Litter box accidents".to_string());
        }
        "less" => {
            concern += 2;
            flags.push("Using the litter box less".to_string());
        }
        "more" => {
            concern += 1;
            flags.push("More litter box trips".to_string());
        }
        _ => {}
    }

    match form.drinking.as_str() {
        "more" => {
            concern += 2;
            flags.push("Drinking more than usual".to_string());
        }
        "less" => {
            concern += 2;
            flags.push("Drinking less than usual".to_string());
        }
        _ => {}
    }

    match form.grooming_mood.as_str() {
        "hiding" => {
            concern += 2;
            flags.push("Hiding more than usual".to_string());
        }
        "less_grooming" => {
            concern += 2;
            flags.push("Grooming less than usual".to_string());
        }
        _ => {}
    }

    if form.discomfort.as_deref() == Some("on") {
        concern += 4;
        flags.push("Pain, limping, or vomiting today".to_string());
    }

    if let (Some(prev), Some(current)) = (previous_weight, weight) {
        if prev > 0.0 {
            let change_pct = ((current - prev) / prev) * 100.0;
            if change_pct <= -10.0 {
                concern += 2;
                flags.push("Noticeable weight loss since last check".to_string());
            } else if change_pct >= 15.0 {
                concern += 1;
                flags.push("Weight gain since last check".to_string());
            }
        }
    }

    let outcome = if concern >= 5 {
        CheckOutcome::VetSoon
    } else if concern >= 2 {
        CheckOutcome::Monitor
    } else {
        CheckOutcome::Reassuring
    };

    let summary = match outcome {
        CheckOutcome::Reassuring => format!(
            "{pet_name} looks reassuring on today's home check — nothing you reported sounds urgent."
        ),
        CheckOutcome::Monitor => format!(
            "Mostly okay, but keep a close eye on {pet_name} and book that overdue vet visit soon."
        ),
        CheckOutcome::VetSoon => format!(
            "A few answers suggest calling your vet about {pet_name}, even before the overdue checkup."
        ),
    };

    let detail = if flags.is_empty() {
        "Appetite, energy, litter habits, and mood all sounded normal. This is not a substitute for a real exam — still schedule the overdue wellness visit when you can.".to_string()
    } else {
        format!(
            "We noticed: {}. A home check cannot rule out illness — contact your vet if anything worsens.",
            flags.join("; ")
        )
    };

    Ok(HomeHealthCheckResult {
        outcome,
        summary,
        detail,
        flags,
    })
}

pub fn save_check(
    profile: &mut UserProfile,
    pet_id: &str,
    form: &HomeHealthCheckForm,
    result: &HomeHealthCheckResult,
    today: NaiveDate,
) -> Result<(), &'static str> {
    if !crate::pet_id_exists(profile, pet_id) {
        return Err("invalid_pet");
    }

    let weight = parse_weight(form.weight_lbs.as_deref());
    let entry = HomeHealthCheckEntry {
        checked_at: today.format("%Y-%m-%d").to_string(),
        weight_lbs: weight,
        appetite: form.appetite.clone(),
        energy: form.energy.clone(),
        litter_habits: form.litter_habits.clone(),
        drinking: form.drinking.clone(),
        grooming_mood: form.grooming_mood.clone(),
        discomfort: form.discomfort.as_deref() == Some("on"),
        outcome: result.outcome.as_str().to_string(),
        summary: result.summary.clone(),
        detail: result.detail.clone(),
    };

    if let Some(weight_lbs) = weight {
        profile.pet_weights.insert(
            pet_id.to_string(),
            PetWeightRecord {
                weight_lbs,
                recorded_at: entry.checked_at.clone(),
            },
        );
    }

    let history = profile
        .home_health_checks
        .entry(pet_id.to_string())
        .or_default();
    history.push(entry);
    if history.len() > MAX_HISTORY_PER_PET {
        let overflow = history.len() - MAX_HISTORY_PER_PET;
        history.drain(0..overflow);
    }

    Ok(())
}

fn render_radio_group(
    name: &str,
    legend: &str,
    options: &[(&str, &str)],
    default_value: &str,
    wide_grid: bool,
) -> String {
    let field_id = name.replace('_', "-");
    let grid_class = if wide_grid {
        "home-health-radio-grid home-health-radio-grid--wide"
    } else {
        "home-health-radio-grid"
    };
    let options_html = options
        .iter()
        .map(|(value, label)| {
            let checked = if *value == default_value {
                " checked"
            } else {
                ""
            };
            format!(
                r#"<label class="radio-pill home-health-radio"><input type="radio" name="{name}" value="{value}"{checked} required /> {label}</label>"#,
                name = escape_html_attr(name),
                value = escape_html_attr(value),
                checked = checked,
                label = escape_html(label),
            )
        })
        .collect::<String>();

    format!(
        r#"<fieldset class="home-health-fieldset home-health-question" id="{field_id}-fieldset">
  <legend class="home-health-question-label">{legend}</legend>
  <div class="{grid_class}">{options_html}</div>
</fieldset>"#,
        field_id = field_id,
        legend = escape_html(legend),
        grid_class = grid_class,
        options_html = options_html,
    )
}

fn render_last_check_summary(entry: &HomeHealthCheckEntry, pet_label: &str) -> String {
    let outcome = match entry.outcome.as_str() {
        "vet_soon" => CheckOutcome::VetSoon,
        "monitor" => CheckOutcome::Monitor,
        _ => CheckOutcome::Reassuring,
    };
  format!(
        r#"<div class="home-health-recent {outcome_class}">
  <p class="home-health-recent-kicker">Last home check · {date}</p>
  <p class="home-health-recent-title">{title}</p>
  <p class="home-health-recent-copy">{summary}</p>
  <p class="home-health-recent-note">Another check unlocks in a few days, or sooner if {pet_label} seems off.</p>
</div>"#,
        outcome_class = outcome.css_class(),
        date = escape_html(&entry.checked_at),
        title = escape_html(outcome.label()),
        summary = escape_html(&entry.summary),
        pet_label = escape_html(pet_label),
    )
}

pub fn render_section(snapshot: &PetSnapshot, profile: &UserProfile, today: NaiveDate) -> String {
    if snapshot.deceased {
        return String::new();
    }

    let name = pet_name(snapshot);
    let pet_id = escape_html_attr(&snapshot.id);
    let overdue = checkup_overdue(snapshot, today);

    if let Some(days) = days_since_last_check(profile, &snapshot.id, today) {
        if days < RECHECK_COOLDOWN_DAYS {
            if let Some(entry) = profile.home_health_checks.get(&snapshot.id).and_then(|h| h.last())
            {
                return format!(
                    r#"<div class="home-health-check-block">
  <div class="home-health-check-header">
    <span class="home-health-check-badge" aria-hidden="true">🩺</span>
    <div class="home-health-check-copy">
      <h3 class="home-health-check-title">Quick home health check</h3>
      <p class="home-health-check-lead">Here&apos;s how {name} looked at home on your last check.</p>
    </div>
  </div>
  {recent}
</div>"#,
                    name = escape_html(&name),
                    recent = render_last_check_summary(entry, &name),
                );
            }
        }
    }

    if !overdue {
        return String::new();
    }

    let weight_hint = profile
        .pet_weights
        .get(&snapshot.id)
        .map(|record| {
            format!(
                r#"<p class="field-hint home-health-weight-hint">Last weight: <strong>{weight} lbs</strong> (recorded {date})</p>"#,
                weight = record.weight_lbs,
                date = escape_html(&record.recorded_at),
            )
        })
        .unwrap_or_else(|| {
            r#"<p class="field-hint home-health-weight-hint">Weigh on a home scale if you can — even a rough estimate helps track trends.</p>"#.to_string()
        });

    let appetite = render_radio_group(
        "appetite",
        "Appetite today",
        &[
            ("normal", "Normal"),
            ("less", "Eating less"),
            ("more", "Eating more"),
            ("none", "Not eating"),
        ],
        "normal",
        false,
    );
    let energy = render_radio_group(
        "energy",
        "Energy",
        &[
            ("normal", "Normal"),
            ("low", "Lower than usual"),
            ("high", "Very restless"),
        ],
        "normal",
        false,
    );
    let litter = render_radio_group(
        "litter_habits",
        "Litter box",
        &[
            ("normal", "Normal"),
            ("more", "More trips"),
            ("less", "Less than usual"),
            ("straining", "Straining / crying"),
            ("accidents", "Accidents outside box"),
        ],
        "normal",
        true,
    );
    let drinking = render_radio_group(
        "drinking",
        "Drinking",
        &[
            ("normal", "Normal"),
            ("more", "Drinking more"),
            ("less", "Drinking less"),
        ],
        "normal",
        false,
    );
    let grooming = render_radio_group(
        "grooming_mood",
        "Mood & grooming",
        &[
            ("normal", "Normal"),
            ("hiding", "Hiding more"),
            ("less_grooming", "Grooming less"),
        ],
        "normal",
        false,
    );

    format!(
        r#"<div class="home-health-check-block">
  <div class="home-health-check-header">
    <span class="home-health-check-badge" aria-hidden="true">🩺</span>
    <div class="home-health-check-copy">
      <h3 class="home-health-check-title">Quick home health check</h3>
      <p class="home-health-check-lead">A vet visit is overdue — answer a few basics so you know {name} still seems okay at home.</p>
      <p class="home-health-check-disclaimer">Not a diagnosis — just a cozy between-visit check-in.</p>
    </div>
  </div>
  <form class="login-form home-health-check-form" action="/home/health/check" method="post">
    <input type="hidden" name="pet_id" value="{pet_id}" />
    <div class="home-health-weight-group">
      <label for="home-health-weight-{pet_id}">Weight (lbs, optional)</label>
      <input id="home-health-weight-{pet_id}" name="weight_lbs" type="number" inputmode="decimal" min="1" max="45" step="0.1" placeholder="e.g. 9.4" />
      {weight_hint}
    </div>
    {appetite}
    {energy}
    {litter}
    {drinking}
    {grooming}
    <label class="checkbox-pill home-health-discomfort">
      <input type="checkbox" name="discomfort" value="on" />
      Limping, pain, or vomiting today
    </label>
    <button type="submit" class="download-btn home-health-check-submit">Save check for {name} 🐾</button>
  </form>
</div>"#,
        name = escape_html(&name),
        pet_id = pet_id,
        weight_hint = weight_hint,
        appetite = appetite,
        energy = energy,
        litter = litter,
        drinking = drinking,
        grooming = grooming,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_care_schedule, default_profile, PetSnapshot, PRIMARY_PET_ID};

    fn snapshot(name: &str) -> PetSnapshot {
        PetSnapshot {
            id: PRIMARY_PET_ID.to_string(),
            pet_name: name.to_string(),
            pet_breed: "Domestic Shorthair".to_string(),
            pet_color: "Tabby".to_string(),
            pet_mood: "Happy".to_string(),
            pet_age_weeks: None,
            pet_age_years: Some(4),
            pet_birth_date: Some("2022-01-01".to_string()),
            last_vet_date: Some("2024-01-01".to_string()),
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
            memorial_videos: vec![],
            memorial_comfort_seen: false,
        }
    }

    #[test]
    fn reassuring_when_all_normal() {
        let form = HomeHealthCheckForm {
            pet_id: PRIMARY_PET_ID.to_string(),
            weight_lbs: Some("10".to_string()),
            appetite: "normal".to_string(),
            energy: "normal".to_string(),
            litter_habits: "normal".to_string(),
            drinking: "normal".to_string(),
            grooming_mood: "normal".to_string(),
            discomfort: None,
        };
        let result = evaluate(&form, None, "Mochi").expect("ok");
        assert_eq!(result.outcome, CheckOutcome::Reassuring);
    }

    #[test]
    fn vet_soon_for_straining_and_not_eating() {
        let form = HomeHealthCheckForm {
            pet_id: PRIMARY_PET_ID.to_string(),
            weight_lbs: None,
            appetite: "none".to_string(),
            energy: "low".to_string(),
            litter_habits: "straining".to_string(),
            drinking: "normal".to_string(),
            grooming_mood: "hiding".to_string(),
            discomfort: Some("on".to_string()),
        };
        let result = evaluate(&form, None, "Mochi").expect("ok");
        assert_eq!(result.outcome, CheckOutcome::VetSoon);
    }

    #[test]
    fn renders_form_when_checkup_overdue() {
        let profile = default_profile("test@example.com");
        let snap = snapshot("Mochi");
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
        assert!(checkup_overdue(&snap, today));
        let html = render_section(&snap, &profile, today);
        assert!(html.contains("Quick home health check"));
        assert!(html.contains("weight_lbs"));
    }
}
