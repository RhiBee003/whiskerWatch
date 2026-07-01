use chrono::{Datelike, Duration, NaiveDate};

use crate::{
    breed_guides, breed_health, generate_vaccine_calendar_events_for_snapshot,
    pet_snapshot, vet_reminder_interval_for_snapshot, CalendarEvent,
    PetSnapshot, UserProfile,
};

const UPCOMING_WINDOW_DAYS: i64 = 45;
const DUE_SOON_WINDOW_DAYS: i64 = 14;
const CHRONIC_CARE_INTERVAL_DAYS: i64 = 182;

const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VetCareUrgency {
    ActionNeeded,
    DueSoon,
    OnTrack,
}

#[derive(Debug, Clone)]
pub struct VetCareItem {
    pub label: String,
    pub due_date: NaiveDate,
    pub overdue: bool,
}

#[derive(Debug, Clone)]
pub struct VetCarePlan {
    pub urgency: VetCareUrgency,
    pub needs_appointment: bool,
    pub headline: String,
    pub detail: String,
    pub visit_summary: String,
    pub context_tips: Vec<String>,
    pub overdue_items: Vec<VetCareItem>,
    pub upcoming_items: Vec<VetCareItem>,
    pub wellness_interval_label: String,
}

fn parse_vet_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}

fn pet_display_name(snapshot: &PetSnapshot) -> String {
    let name = snapshot.pet_name.trim();
    if name.is_empty() {
        "your cat".to_string()
    } else {
        name.to_string()
    }
}

fn format_short_date(date: NaiveDate) -> String {
    format!(
        "{} {}",
        MONTHS_SHORT
            .get(date.month0() as usize)
            .copied()
            .unwrap_or("???"),
        date.day()
    )
}

fn format_relative_due(today: NaiveDate, due: NaiveDate, overdue: bool) -> String {
    let days = (due - today).num_days();
    if days == 0 {
        return "due today".to_string();
    }
    if overdue {
        let n = days.unsigned_abs();
        if n == 1 {
            "1 day overdue".to_string()
        } else {
            format!("{n} days overdue")
        }
    } else if days == 1 {
        "in 1 day".to_string()
    } else {
        format!("in {days} days")
    }
}

fn is_outdoor(snapshot: &PetSnapshot) -> bool {
    snapshot
        .pet_indoor_outdoor
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("outdoor"))
}

fn has_chronic_care(snapshot: &PetSnapshot) -> bool {
    !snapshot.pet_conditions.trim().is_empty() || !snapshot.pet_medications.trim().is_empty()
}

fn wellness_exam_label(snapshot: &PetSnapshot) -> String {
    if snapshot.pet_age_weeks.is_some_and(|weeks| weeks < 16) {
        "Kitten wellness check".to_string()
    } else if snapshot.pet_age_years.is_some_and(|years| years >= 10) {
        "Senior wellness exam".to_string()
    } else if let Some(guide) = breed_guides::guide_for_breed_name(&snapshot.pet_breed) {
        if breed_guides::wellness_exam_interval_months(&guide) <= 6 {
            format!("{} wellness exam", guide.breed_name)
        } else {
            "Wellness exam".to_string()
        }
    } else {
        "Wellness exam".to_string()
    }
}

fn wellness_interval_label(snapshot: &PetSnapshot) -> String {
    let days = vet_reminder_interval_for_snapshot(snapshot).num_days();
    if days <= 30 {
        "every 4 weeks".to_string()
    } else if days <= 200 {
        "every 6 months".to_string()
    } else {
        "every 12 months".to_string()
    }
}

fn visit_summary(snapshot: &PetSnapshot, today: NaiveDate) -> String {
    if snapshot.never_been_to_vet {
        return "No vet visits recorded yet.".to_string();
    }

    let last = snapshot
        .last_vet_date
        .as_deref()
        .and_then(parse_vet_date)
        .map(format_short_date);

    let next_wellness = next_wellness_due(snapshot, today).map(|due| {
        let relative = format_relative_due(today, due, due <= today);
        format!("{} ({relative})", format_short_date(due))
    });

    match (last, next_wellness) {
        (Some(last), Some(next)) => format!("Last visit: {last} · Next wellness: {next}"),
        (Some(last), None) => format!("Last visit: {last}"),
        (None, Some(next)) => format!("Next wellness: {next}"),
        (None, None) => String::new(),
    }
}

fn collect_context_tips(snapshot: &PetSnapshot) -> Vec<String> {
    let mut tips = Vec::new();
    let name = pet_display_name(snapshot);

    if is_outdoor(snapshot) {
        tips.push(format!(
            "{name} goes outside — ask about FeLV boosters, parasite prevention, and wound checks."
        ));
    }

    if snapshot.pet_age_years.is_some_and(|years| years >= 10) {
        tips.push(
            "Senior cats benefit from bloodwork, dental checks, and steady weight monitoring."
                .to_string(),
        );
    }

    if let Some(breed) = breed_health::resolve_breed(&snapshot.pet_breed) {
        if breed.brachycephalic {
            tips.push(format!(
                "{} is brachycephalic — mention breathing, eye, and dental concerns.",
                breed.name
            ));
        }
        if breed.folded_ear {
            tips.push(
                "Folded-ear breeds need ear and joint checks at wellness visits.".to_string(),
            );
        }
        if breed.hairless {
            tips.push("Hairless breeds need regular skin exams and warmth checks.".to_string());
        }
        if breed.large_breed {
            tips.push(format!(
                "Large breeds like {} may need heart and joint screening as they age.",
                breed.name
            ));
        }
    }

    if has_chronic_care(snapshot) {
        tips.push(
            "Bring the medication list and ask your vet to review the current treatment plan."
                .to_string(),
        );
    }

    if let Some(guide) = breed_guides::guide_for_breed_name(&snapshot.pet_breed) {
        if breed_guides::wellness_exam_interval_months(&guide) <= 6
            && !snapshot.pet_age_years.is_some_and(|years| years >= 10)
        {
            tips.push(format!(
                "{} cats often benefit from wellness exams every 6 months.",
                guide.breed_name
            ));
        }
    }

    tips.truncate(3);
    tips
}

fn event_date(event: &CalendarEvent) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(event.year as i32, event.month, event.day)
}

fn vaccine_label_from_event_title(title: &str) -> String {
    title
        .split('—')
        .next()
        .unwrap_or(title)
        .trim()
        .to_string()
}

fn next_wellness_due(snapshot: &PetSnapshot, today: NaiveDate) -> Option<NaiveDate> {
    if snapshot.never_been_to_vet {
        return Some(today);
    }

    let anchor = snapshot
        .last_vet_date
        .as_deref()
        .and_then(parse_vet_date)
        .unwrap_or(today);
    Some(anchor + vet_reminder_interval_for_snapshot(snapshot))
}

fn push_care_item(
    overdue: &mut Vec<VetCareItem>,
    upcoming: &mut Vec<VetCareItem>,
    label: String,
    due: NaiveDate,
    today: NaiveDate,
) {
    if due <= today {
        if !overdue.iter().any(|item: &VetCareItem| item.label == label) {
            overdue.push(VetCareItem {
                label,
                due_date: due,
                overdue: true,
            });
        }
    } else if due <= today + Duration::days(UPCOMING_WINDOW_DAYS)
        && !upcoming.iter().any(|item: &VetCareItem| item.label == label)
    {
        upcoming.push(VetCareItem {
            label,
            due_date: due,
            overdue: false,
        });
    }
}

fn collect_vaccine_items(snapshot: &PetSnapshot, today: NaiveDate) -> (Vec<VetCareItem>, Vec<VetCareItem>) {
    let mut overdue = Vec::new();
    let mut upcoming = Vec::new();
    let horizon = today + Duration::days(UPCOMING_WINDOW_DAYS);
    let events = generate_vaccine_calendar_events_for_snapshot(snapshot, today);

    for event in events {
        let Some(date) = event_date(&event) else {
            continue;
        };
        let label = vaccine_label_from_event_title(&event.title);
        if label.is_empty() {
            continue;
        }

        if date <= today {
            if overdue.iter().any(|item: &VetCareItem| item.label == label) {
                continue;
            }
            overdue.push(VetCareItem {
                label,
                due_date: date,
                overdue: true,
            });
        } else if date <= horizon
            && !upcoming.iter().any(|item: &VetCareItem| item.label == label)
        {
            upcoming.push(VetCareItem {
                label,
                due_date: date,
                overdue: false,
            });
        }
    }

    overdue.sort_by_key(|item| item.due_date);
    upcoming.sort_by_key(|item| item.due_date);
    (overdue, upcoming)
}

fn push_wellness_item(
    overdue: &mut Vec<VetCareItem>,
    upcoming: &mut Vec<VetCareItem>,
    snapshot: &PetSnapshot,
    today: NaiveDate,
) {
    if snapshot.never_been_to_vet || snapshot.pet_vaccines_unknown {
        return;
    }

    let Some(due) = next_wellness_due(snapshot, today) else {
        return;
    };

    push_care_item(
        overdue,
        upcoming,
        wellness_exam_label(snapshot),
        due,
        today,
    );
}

fn push_chronic_care_item(
    overdue: &mut Vec<VetCareItem>,
    upcoming: &mut Vec<VetCareItem>,
    snapshot: &PetSnapshot,
    today: NaiveDate,
) {
    if !has_chronic_care(snapshot)
        || snapshot.never_been_to_vet
        || snapshot.pet_vaccines_unknown
        || snapshot.last_vet_date.is_none()
    {
        return;
    }

    let Some(anchor) = snapshot
        .last_vet_date
        .as_deref()
        .and_then(parse_vet_date)
    else {
        return;
    };

    let due = anchor + Duration::days(CHRONIC_CARE_INTERVAL_DAYS);
    push_care_item(
        overdue,
        upcoming,
        "Medication & condition check-in".to_string(),
        due,
        today,
    );
}

pub fn analyze(snapshot: &PetSnapshot, today: NaiveDate) -> VetCarePlan {
    analyze_with_followup(snapshot, today, false)
}

pub fn analyze_with_followup(
    snapshot: &PetSnapshot,
    today: NaiveDate,
    vet_followup_pending: bool,
) -> VetCarePlan {
    let pet_name = pet_display_name(snapshot);
    let wellness_interval_label = wellness_interval_label(snapshot);
    let visit_summary = visit_summary(snapshot, today);
    let context_tips = collect_context_tips(snapshot);
    let (mut overdue_items, mut upcoming_items) = collect_vaccine_items(snapshot, today);
    push_wellness_item(
        &mut overdue_items,
        &mut upcoming_items,
        snapshot,
        today,
    );
    push_chronic_care_item(
        &mut overdue_items,
        &mut upcoming_items,
        snapshot,
        today,
    );

    if vet_followup_pending {
        push_care_item(
            &mut overdue_items,
            &mut upcoming_items,
            "Vet follow-up".to_string(),
            today,
            today,
        );
    }

    overdue_items.sort_by_key(|item| item.due_date);

    let needs_appointment = snapshot.never_been_to_vet
        || snapshot.pet_vaccines_unknown
        || snapshot.last_vet_date.is_none()
        || vet_followup_pending
        || !overdue_items.is_empty();

    let due_soon = !needs_appointment
        && upcoming_items.iter().any(|item| {
            item.due_date <= today + Duration::days(DUE_SOON_WINDOW_DAYS)
        });

    let urgency = if needs_appointment {
        VetCareUrgency::ActionNeeded
    } else if due_soon {
        VetCareUrgency::DueSoon
    } else {
        VetCareUrgency::OnTrack
    };

    let (headline, detail) = if vet_followup_pending && overdue_items.len() <= 1 {
        (
            format!("Follow up on {pet_name}'s last vet visit"),
            format!(
                "You noted a follow-up from your last appointment — book when your vet recommended."
            ),
        )
    } else if snapshot.never_been_to_vet {
        (
            format!("First vet visit for {pet_name}"),
            format!(
                "{pet_name} hasn't had a vet visit recorded yet — book an exam and start vaccine tracking."
            ),
        )
    } else if snapshot.pet_vaccines_unknown {
        (
            format!("Confirm vaccines for {pet_name}"),
            format!(
                "We don't know {pet_name}'s vaccine history — a vet visit will help build an accurate care plan."
            ),
        )
    } else if snapshot.last_vet_date.is_none() {
        (
            format!("Log {pet_name}'s last vet visit"),
            format!(
                "Add your last appointment date so WhiskerWatch can schedule smarter checkup reminders ({wellness_interval_label})."
            ),
        )
    } else if let Some(item) = overdue_items.first() {
        let others = overdue_items.len().saturating_sub(1);
        let extra = if others > 0 {
            format!(" (+{others} more)")
        } else {
            String::new()
        };
        let relative = format_relative_due(today, item.due_date, true);
        (
            format!("Care due for {pet_name}"),
            format!(
                "{} was {relative} — schedule a vet visit soon.{extra}",
                item.label,
            ),
        )
    } else if due_soon {
        let item = upcoming_items
            .first()
            .expect("due soon implies upcoming item");
        let relative = format_relative_due(today, item.due_date, false);
        (
            format!("Coming up for {pet_name}"),
            format!(
                "{} is {relative} — good time to book ahead.",
                item.label,
            ),
        )
    } else if let Some(item) = upcoming_items.first() {
        let relative = format_relative_due(today, item.due_date, false);
        (
            format!("{pet_name} is on track"),
            format!(
                "Next up: {} {relative}. Wellness checks are recommended {wellness_interval_label}.",
                item.label,
            ),
        )
    } else {
        (
            format!("{pet_name} is on track"),
            format!(
                "No vaccines or wellness visits due right now. We recommend wellness checks {wellness_interval_label}."
            ),
        )
    };

    VetCarePlan {
        urgency,
        needs_appointment,
        headline,
        detail,
        visit_summary,
        context_tips,
        overdue_items,
        upcoming_items,
        wellness_interval_label,
    }
}

pub fn needs_appointment(snapshot: &PetSnapshot, today: NaiveDate) -> bool {
    analyze(snapshot, today).needs_appointment
}

pub fn task_title(snapshot: &PetSnapshot, today: NaiveDate) -> String {
    let plan = analyze(snapshot, today);
    let name = pet_display_name(snapshot);

    if let Some(item) = plan.overdue_items.first() {
        if item.label == "Vet follow-up" {
            return format!("Book vet follow-up for {name}");
        }
        return format!("Schedule {} for {name}", item.label.to_lowercase());
    }
    if snapshot.never_been_to_vet {
        return format!("Book first vet visit for {name}");
    }
    if snapshot.pet_vaccines_unknown {
        return format!("Confirm vaccines at vet for {name}");
    }
    if snapshot.last_vet_date.is_none() {
        return format!("Log vet visit for {name}");
    }

    format!("Schedule vet visit for {name}")
}

pub fn task_due_label(snapshot: &PetSnapshot, today: NaiveDate) -> String {
    let plan = analyze(snapshot, today);

    if let Some(item) = plan.overdue_items.first() {
        let days = (today - item.due_date).num_days();
        if days > 30 {
            return "Overdue · schedule now".to_string();
        }
        if days > 7 {
            return "Overdue · book soon".to_string();
        }
        return "Due now · book appointment".to_string();
    }

    if snapshot.never_been_to_vet || snapshot.pet_vaccines_unknown || snapshot.last_vet_date.is_none()
    {
        return "Due now · book appointment".to_string();
    }

    if plan.urgency == VetCareUrgency::DueSoon {
        return "Due soon · plan ahead".to_string();
    }

    "When ready".to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_item_list(items: &[VetCareItem], today: NaiveDate, class_name: &str) -> String {
    if items.is_empty() {
        return String::new();
    }

    let rows = items
        .iter()
        .map(|item| {
            let relative = format_relative_due(today, item.due_date, item.overdue);
            format!(
                r#"<li class="vet-care-plan-item"><span class="vet-care-plan-item-label">{label}</span><span class="vet-care-plan-item-date">{date}<span class="vet-care-plan-item-relative">{relative}</span></span></li>"#,
                label = escape_html(&item.label),
                date = escape_html(&format_short_date(item.due_date)),
                relative = escape_html(&relative),
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(r#"<ul class="vet-care-plan-list {class_name}" role="list">{rows}</ul>"#)
}

fn render_context_tips(tips: &[String]) -> String {
    if tips.is_empty() {
        return String::new();
    }

    let rows = tips
        .iter()
        .map(|tip| {
            format!(
                r#"<li class="vet-care-plan-tip">{tip}</li>"#,
                tip = escape_html(tip)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<div class="vet-care-plan-tips"><h3 class="vet-care-plan-group-title">What to mention</h3><ul class="vet-care-plan-tip-list" role="list">{rows}</ul></div>"#
    )
}

pub fn render_health_plan_cards(
    profile: &UserProfile,
    today: NaiveDate,
    vet_followup_pending: bool,
    extra_html_for_pet: impl Fn(&PetSnapshot) -> String,
) -> String {
    let active_pet_id = profile.active_pet_id.as_str();
    let Some(snapshot) = pet_snapshot(profile, active_pet_id) else {
        return String::new();
    };
    if snapshot.deceased {
        return String::new();
    }

    let followup = vet_followup_pending;
    let mut card = render_health_plan_card(&snapshot, today, followup);
    let extra = extra_html_for_pet(&snapshot);
    if !extra.is_empty() {
        card = card.replace("</article>", &format!("{extra}</article>"));
    }
    card
}

pub fn render_health_plan_card(
    snapshot: &PetSnapshot,
    today: NaiveDate,
    vet_followup_pending: bool,
) -> String {
    let plan = analyze_with_followup(snapshot, today, vet_followup_pending);
    let headline = escape_html(&plan.headline);
    let detail = escape_html(&plan.detail);
    let visit_summary = if plan.visit_summary.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="vet-care-plan-visit-summary">{summary}</p>"#,
            summary = escape_html(&plan.visit_summary)
        )
    };
    let tips_html = render_context_tips(&plan.context_tips);
    let urgency_class = match plan.urgency {
        VetCareUrgency::ActionNeeded => "vet-care-plan-action",
        VetCareUrgency::DueSoon => "vet-care-plan-due-soon",
        VetCareUrgency::OnTrack => "vet-care-plan-on-track",
    };

    let overdue_html = render_item_list(&plan.overdue_items, today, "vet-care-plan-list-overdue");
    let upcoming_html =
        render_item_list(&plan.upcoming_items, today, "vet-care-plan-list-upcoming");
    let lists = if overdue_html.is_empty() && upcoming_html.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="vet-care-plan-lists">{overdue}{upcoming}</div>"#,
            overdue = if overdue_html.is_empty() {
                String::new()
            } else {
                format!(r#"<div class="vet-care-plan-group"><h3 class="vet-care-plan-group-title">Due now</h3>{overdue_html}</div>"#)
            },
            upcoming = if upcoming_html.is_empty() {
                String::new()
            } else {
                format!(r#"<div class="vet-care-plan-group"><h3 class="vet-care-plan-group-title">Coming up</h3>{upcoming_html}</div>"#)
            },
        )
    };

    let cta = if plan.needs_appointment {
        r##"<p class="vet-care-plan-cta"><a href="#health-vet-disclosure" class="download-btn vet-care-plan-btn">Record a vet visit</a></p>"##
    } else {
        r#"<p class="vet-care-plan-footnote">WhiskerWatch adjusts reminders for age, breed, lifestyle, and vaccine history.</p>"#
    };

    format!(
        r#"<article class="dashboard-card vet-care-plan-card {urgency_class}">
  <div class="vet-care-plan-header">
    <span class="vet-care-plan-badge" aria-hidden="true">🏥</span>
    <div class="vet-care-plan-copy">
      <h2 class="vet-care-plan-title">Smart vet care</h2>
      <p class="vet-care-plan-headline">{headline}</p>
    </div>
  </div>
  <p class="vet-care-plan-detail">{detail}</p>
  {visit_summary}
  {lists}
  {tips_html}
  {cta}
</article>"#,
        urgency_class = urgency_class,
        headline = headline,
        detail = detail,
        visit_summary = visit_summary,
        lists = lists,
        tips_html = tips_html,
        cta = cta,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_care_schedule, PetSnapshot, PRIMARY_PET_ID};

    fn snapshot_with_breed(breed: &str, years: u32, last_vet: &str) -> PetSnapshot {
        PetSnapshot {
            id: PRIMARY_PET_ID.to_string(),
            pet_name: "Mochi".to_string(),
            pet_breed: breed.to_string(),
            pet_color: "Calico".to_string(),
            pet_mood: "Happy".to_string(),
            pet_age_weeks: None,
            pet_age_years: Some(years),
            pet_birth_date: Some("2022-01-01".to_string()),
            last_vet_date: Some(last_vet.to_string()),
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
    fn persian_gets_six_month_wellness_interval() {
        let snapshot = snapshot_with_breed("Persian", 3, "2025-01-01");
        assert_eq!(wellness_interval_label(&snapshot), "every 6 months");
    }

    #[test]
    fn overdue_item_uses_relative_wording() {
        let snapshot = snapshot_with_breed("Domestic Shorthair", 3, "2024-01-01");
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
        let plan = analyze(&snapshot, today);
        assert!(plan.detail.contains("days overdue") || plan.detail.contains("day overdue"));
    }

    #[test]
    fn outdoor_cat_gets_context_tip() {
        let mut snapshot = snapshot_with_breed("Domestic Shorthair", 2, "2025-12-01");
        snapshot.pet_indoor_outdoor = Some("outdoor".to_string());
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
        let plan = analyze(&snapshot, today);
        assert!(plan
            .context_tips
            .iter()
            .any(|tip| tip.contains("FeLV")));
    }

    #[test]
    fn chronic_care_adds_check_in_item() {
        let mut snapshot = snapshot_with_breed("Domestic Shorthair", 4, "2025-01-01");
        snapshot.pet_medications = "Methimazole".to_string();
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
        let plan = analyze(&snapshot, today);
        assert!(plan
            .overdue_items
            .iter()
            .any(|item| item.label.contains("Medication")));
    }
}
