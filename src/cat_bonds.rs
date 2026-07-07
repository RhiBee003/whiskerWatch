use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{pet_id_exists, playdates, push_activity, save_profile, AppState, UserProfile};

pub const BOND_SCORE_MIN: i32 = 0;
pub const BOND_SCORE_MAX: i32 = 100;
const DAILY_BOND_FULL_REWARD_LIMIT: u32 = 8;

#[derive(Debug, Clone, Copy)]
pub struct BondActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub emoji: &'static str,
    pub bond_delta: i32,
    pub paw_points: u32,
    pub parent_xp: u32,
}

pub const BOND_ACTIONS: &[BondActionDef] = &[
    BondActionDef {
        id: "pet",
        label: "Gentle pet",
        emoji: "🐾",
        bond_delta: 4,
        paw_points: 2,
        parent_xp: 10,
    },
    BondActionDef {
        id: "play",
        label: "Playtime",
        emoji: "🧶",
        bond_delta: 6,
        paw_points: 4,
        parent_xp: 15,
    },
    BondActionDef {
        id: "cuddle",
        label: "Cozy cuddle",
        emoji: "💕",
        bond_delta: 8,
        paw_points: 6,
        parent_xp: 22,
    },
];

#[derive(Debug, Deserialize)]
pub struct BondInteractRequest {
    pub pet_id: String,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct BondInteractResponse {
    pub ok: bool,
    pub message: String,
    pub bond_score: i32,
    pub bond_label: String,
    pub bond_emoji: String,
    pub paw_points: u32,
    pub paw_points_earned: u32,
    pub parent_xp: u32,
    pub parent_level: u32,
    pub parent_xp_earned: u32,
    pub leveled_up: bool,
    pub new_parent_level: Option<u32>,
    pub diminished_rewards: bool,
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

fn daily_bond_key(pet_id: &str, today: NaiveDate) -> String {
    format!("{pet_id}|{}", today.format("%Y-%m-%d"))
}

pub fn bond_score(profile: &UserProfile, pet_id: &str) -> i32 {
    profile
        .parent_cat_bonds
        .get(pet_id)
        .copied()
        .unwrap_or(0)
        .clamp(BOND_SCORE_MIN, BOND_SCORE_MAX)
}

pub fn adjust_bond(profile: &mut UserProfile, pet_id: &str, delta: i32) -> i32 {
    let entry = profile
        .parent_cat_bonds
        .entry(pet_id.to_string())
        .or_insert(0);
    *entry = (*entry + delta).clamp(BOND_SCORE_MIN, BOND_SCORE_MAX);
    *entry
}

pub fn parent_xp_for_next_level(level: u32) -> u32 {
    level.saturating_mul(80).saturating_add(20)
}

pub fn grant_parent_xp(profile: &mut UserProfile, amount: u32) -> Option<u32> {
    if amount == 0 {
        return None;
    }
    profile.parent_xp = profile.parent_xp.saturating_add(amount);
    let mut leveled_to = None;
    loop {
        let needed = parent_xp_for_next_level(profile.parent_level);
        if profile.parent_xp >= needed {
            profile.parent_xp -= needed;
            profile.parent_level = profile.parent_level.saturating_add(1);
            leveled_to = Some(profile.parent_level);
        } else {
            break;
        }
    }
    leveled_to
}

fn bond_action_by_id(action_id: &str) -> Option<&'static BondActionDef> {
    BOND_ACTIONS.iter().find(|action| action.id == action_id)
}

fn bond_daily_count(profile: &UserProfile, pet_id: &str, today: NaiveDate) -> u32 {
    profile
        .cat_bond_daily_counts
        .get(&daily_bond_key(pet_id, today))
        .copied()
        .unwrap_or(0)
}

fn increment_bond_daily_count(profile: &mut UserProfile, pet_id: &str, today: NaiveDate) {
    let key = daily_bond_key(pet_id, today);
    let entry = profile.cat_bond_daily_counts.entry(key).or_insert(0);
    *entry = entry.saturating_add(1);
}

fn bond_message(action: &BondActionDef, pet_name: &str) -> String {
    match action.id {
        "pet" => format!("You gave {pet_name} the sweetest pets."),
        "play" => format!("Playtime with {pet_name} was a hit!"),
        "cuddle" => format!("{pet_name} melted into a cozy cuddle."),
        _ => format!("Quality time with {pet_name}!"),
    }
}

pub fn list_owned_pet_bonds(profile: &UserProfile) -> Vec<(String, String, i32)> {
    let mut bonds = Vec::new();
    let primary_id = crate::PRIMARY_PET_ID.to_string();
    bonds.push((
        primary_id.clone(),
        profile.pet_name.clone(),
        bond_score(profile, &primary_id),
    ));
    for pet in &profile.additional_pets {
        if pet.deceased {
            continue;
        }
        bonds.push((
            pet.id.clone(),
            pet.pet_name.clone(),
            bond_score(profile, &pet.id),
        ));
    }
    bonds
}

fn render_bond_row(pet_id: &str, pet_name: &str, score: i32) -> String {
    let (label, emoji) = playdates::friendship_tier(score);
    let percent = playdates::friendship_tier_progress_percent(score);
    let level_display = playdates::format_friendship_level_display(score);
    format!(
        r#"<li class="cat-home-bond-row" data-bond-pet-id="{pet_id}">
  <div class="cat-home-bond-meta">
    <span class="cat-home-bond-name">{name}</span>
    <span class="cat-home-bond-role">Your cat</span>
  </div>
  <div class="cat-home-bond-meter" role="meter" aria-valuenow="{score}" aria-valuemin="{min}" aria-valuemax="{max}" aria-label="Bond with {name}: {label} ({level_display})">
    <div class="cat-home-bond-meter-fill" style="width: {percent}%"></div>
  </div>
  <p class="cat-home-bond-tier">{emoji} {label} · {level_display}</p>
</li>"#,
        pet_id = escape_html_attr(pet_id),
        name = escape_html(pet_name),
        score = score,
        min = BOND_SCORE_MIN,
        max = BOND_SCORE_MAX,
        label = escape_html(label),
        level_display = escape_html(&level_display),
        percent = percent,
        emoji = emoji,
    )
}

pub fn render_parent_bonds_panel(profile: &UserProfile) -> String {
    let bonds = list_owned_pet_bonds(profile);
    if bonds.is_empty() {
        return String::new();
    }

    let rows = bonds
        .iter()
        .map(|(pet_id, pet_name, score)| render_bond_row(pet_id, pet_name, *score))
        .collect::<String>();

    format!(
        r#"<section class="cat-home-bonds-panel" aria-labelledby="cat-home-bonds-title">
  <div class="cat-home-bonds-header">
    <h3 id="cat-home-bonds-title">Your bond with each cat</h3>
    <p class="field-hint cat-home-bonds-lead">Tap the cat you&apos;re playing as to pet, play, or cuddle. Bars fill toward the next bond level.</p>
  </div>
  <ul class="cat-home-bonds-list">{rows}</ul>
</section>"#
    )
}

pub async fn apply_bond_interaction(
    state: &AppState,
    viewer_email: &str,
    request: &BondInteractRequest,
    today: NaiveDate,
) -> Result<BondInteractResponse, &'static str> {
    let pet_id = request.pet_id.trim();
    if pet_id.is_empty() {
        return Err("invalid");
    }

    let action = bond_action_by_id(request.action.trim()).ok_or("invalid_action")?;

    let mut profile = crate::get_or_create_profile(state, viewer_email).await;
    if !pet_id_exists(&profile, pet_id) {
        return Err("invalid_pet");
    }

    let pet_name = crate::pet_display_name(&profile, pet_id);
    let daily_count = bond_daily_count(&profile, pet_id, today);
    let diminished = daily_count >= DAILY_BOND_FULL_REWARD_LIMIT;
    let reward_scale = if diminished { 0.5 } else { 1.0 };

    let paw_points_earned = ((action.paw_points as f32) * reward_scale).round() as u32;
    let parent_xp_earned = ((action.parent_xp as f32) * reward_scale).round() as u32;
    let bond_delta = if diminished {
        (action.bond_delta as f32 * 0.5).round() as i32
    } else {
        action.bond_delta
    };

    let bond_score = adjust_bond(&mut profile, pet_id, bond_delta);
    profile.paw_points = profile.paw_points.saturating_add(paw_points_earned);
    let new_parent_level = grant_parent_xp(&mut profile, parent_xp_earned);
    increment_bond_daily_count(&mut profile, pet_id, today);
    if profile.active_pet_id != pet_id {
        crate::set_active_pet(&mut profile, pet_id);
    }

    let (bond_label, bond_emoji) = playdates::friendship_tier(bond_score);
    let mut message = bond_message(action, &pet_name);
    if diminished {
        message.push_str(" (cozy bonus — full rewards return tomorrow.)");
    }

    let activity = if let Some(level) = new_parent_level {
        format!(
            "{message} +{paw_points_earned} paw points, +{parent_xp_earned} parent XP. Parent level {level}!"
        )
    } else {
        format!("{message} +{paw_points_earned} paw points, +{parent_xp_earned} parent XP.")
    };
    push_activity(&mut profile, &activity);

    save_profile(state, &profile).await.map_err(|_| "error")?;

    Ok(BondInteractResponse {
        ok: true,
        message,
        bond_score,
        bond_label: bond_label.to_string(),
        bond_emoji: bond_emoji.to_string(),
        paw_points: profile.paw_points,
        paw_points_earned,
        parent_xp: profile.parent_xp,
        parent_level: profile.parent_level,
        parent_xp_earned,
        leveled_up: new_parent_level.is_some(),
        new_parent_level,
        diminished_rewards: diminished,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_profile, PRIMARY_PET_ID};

    #[test]
    fn bond_actions_have_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for action in BOND_ACTIONS {
            assert!(ids.insert(action.id));
        }
    }

    #[test]
    fn grant_parent_xp_levels_up_when_threshold_met() {
        let mut profile = default_profile("test@example.com");
        profile.parent_level = 1;
        profile.parent_xp = 0;
        let leveled = grant_parent_xp(&mut profile, parent_xp_for_next_level(1));
        assert_eq!(leveled, Some(2));
        assert_eq!(profile.parent_level, 2);
        assert_eq!(profile.parent_xp, 0);
    }

    #[test]
    fn bond_panel_lists_primary_cat() {
        let profile = default_profile("test@example.com");
        let html = render_parent_bonds_panel(&profile);
        assert!(html.contains("cat-home-bonds-panel"));
        assert!(html.contains(&profile.pet_name));
    }

    #[test]
    fn adjust_bond_clamps_score() {
        let mut profile = default_profile("test@example.com");
        assert_eq!(
            adjust_bond(&mut profile, PRIMARY_PET_ID, 120),
            BOND_SCORE_MAX
        );
        assert_eq!(bond_score(&profile, PRIMARY_PET_ID), BOND_SCORE_MAX);
    }
}
