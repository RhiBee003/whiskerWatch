use crate::{
    escape_html, escape_html_attr, pet_snapshot, push_activity, sharing, AppState, PetSnapshot,
    UserProfile,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCat {
    pub pet_id: String,
    pub owner_email: String,
    pub pet_name: String,
    pub pet_color: String,
    pub pet_photo_url: Option<String>,
    pub is_owned: bool,
    pub owner_label: String,
    #[serde(default)]
    pub is_npc: bool,
    #[serde(default)]
    pub is_birthday_cat: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PlaydateActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub emoji: &'static str,
    pub delta: i32,
}

pub const CAT_PLAYDATE_ACTIONS: &[PlaydateActionDef] = &[
    PlaydateActionDef {
        id: "sniff",
        label: "Curious sniff",
        emoji: "👃",
        delta: 4,
    },
    PlaydateActionDef {
        id: "hiss",
        label: "Dramatic hiss",
        emoji: "😾",
        delta: -9,
    },
    PlaydateActionDef {
        id: "friendly_brawl",
        label: "Friendly brawl",
        emoji: "🥊",
        delta: 6,
    },
    PlaydateActionDef {
        id: "groom",
        label: "Gentle groom",
        emoji: "💅",
        delta: 10,
    },
    PlaydateActionDef {
        id: "chirp",
        label: "Friendly chirp",
        emoji: "🐾",
        delta: 5,
    },
    PlaydateActionDef {
        id: "side_eye",
        label: "Side eye",
        emoji: "🙄",
        delta: -4,
    },
];

pub const PROP_PLAYDATE_ACTION: PlaydateActionDef = PlaydateActionDef {
    id: "play_together",
    label: "Play with other cat",
    emoji: "🎉",
    delta: 14,
};

#[derive(Debug, Deserialize)]
pub struct PlaydateInteractRequest {
    pub actor_pet_id: String,
    pub actor_owner: String,
    pub target_pet_id: String,
    pub target_owner: String,
    pub action: String,
    #[serde(default)]
    pub prop_slot: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlaydateInteractResponse {
    pub ok: bool,
    pub message: String,
    pub friendship_score: i32,
    pub friendship_label: String,
    pub friendship_emoji: String,
    pub actor_pet_id: String,
    pub target_pet_id: String,
    pub backfired: bool,
}

pub fn friendship_key(
    owner_a: &str,
    pet_a: &str,
    owner_b: &str,
    pet_b: &str,
) -> String {
    let left = (
        sharing::normalize_email(owner_a),
        pet_a.trim().to_string(),
    );
    let right = (
        sharing::normalize_email(owner_b),
        pet_b.trim().to_string(),
    );
    if left <= right {
        format!("{}|{}::{}|{}", left.0, left.1, right.0, right.1)
    } else {
        format!("{}|{}::{}|{}", right.0, right.1, left.0, left.1)
    }
}

pub fn friendship_score(profile: &UserProfile, owner_a: &str, pet_a: &str, owner_b: &str, pet_b: &str) -> i32 {
    profile
        .cat_friendships
        .get(&friendship_key(owner_a, pet_a, owner_b, pet_b))
        .copied()
        .unwrap_or(0)
}

pub fn friendship_tier(score: i32) -> (&'static str, &'static str) {
    match score {
        ..=-20 => ("Frenemies", "💢"),
        -19..=-1 => ("Wary", "😾"),
        0..=9 => ("Strangers", "😐"),
        10..=29 => ("Curious", "👀"),
        30..=54 => ("Acquaintances", "🐾"),
        55..=79 => ("Buddies", "💛"),
        _ => ("Besties", "💖"),
    }
}

pub const FRIENDSHIP_SCORE_MIN: i32 = -50;
pub const FRIENDSHIP_SCORE_MAX: i32 = 100;

pub fn friendship_progress_percent(score: i32) -> u32 {
    let clamped = score.clamp(FRIENDSHIP_SCORE_MIN, FRIENDSHIP_SCORE_MAX);
    let span = (FRIENDSHIP_SCORE_MAX - FRIENDSHIP_SCORE_MIN) as u32;
    (((clamped - FRIENDSHIP_SCORE_MIN) as u32) * 100) / span
}

pub fn friendship_tier_floor(score: i32) -> i32 {
    match score {
        ..=-20 => FRIENDSHIP_SCORE_MIN,
        -19..=-1 => -19,
        0..=9 => 0,
        10..=29 => 10,
        30..=54 => 30,
        55..=79 => 55,
        _ => 80,
    }
}

pub fn friendship_next_level_target(score: i32) -> i32 {
    match score {
        ..=-20 => -19,
        -19..=-1 => 0,
        0..=9 => 10,
        10..=29 => 30,
        30..=54 => 55,
        55..=79 => 80,
        _ => FRIENDSHIP_SCORE_MAX,
    }
}

fn friendship_progress_within_tier(score: i32, floor: i32, ceiling: i32) -> u32 {
    if ceiling <= floor {
        return 100;
    }
    let clamped = score.clamp(floor, ceiling);
    (((clamped - floor) as u32) * 100) / ((ceiling - floor) as u32)
}

pub fn friendship_tier_progress_percent(score: i32) -> u32 {
    let floor = friendship_tier_floor(score);
    let target = friendship_next_level_target(score);
    friendship_progress_within_tier(score, floor, target)
}

pub fn format_friendship_level_display(score: i32) -> String {
    let target = friendship_next_level_target(score);
    format!("{score} / {target}")
}

fn pet_is_alive(snapshot: &PetSnapshot) -> bool {
    !snapshot.deceased
}

pub fn list_scene_cats(state: &AppState, viewer: &UserProfile) -> Vec<SceneCat> {
    let mut cats = Vec::new();
    let mut seen = HashSet::new();

    for (pet_id, _pet_name) in sharing::pet_summaries_for_profile(viewer) {
        let Some(snapshot) = pet_snapshot(viewer, &pet_id) else {
            continue;
        };
        if !pet_is_alive(&snapshot) {
            continue;
        }
        let dedupe = format!("{}|{}", sharing::normalize_email(&viewer.email), pet_id);
        if !seen.insert(dedupe) {
            continue;
        }
        cats.push(SceneCat {
            pet_id,
            owner_email: viewer.email.clone(),
            pet_name: snapshot.pet_name,
            pet_color: snapshot.pet_color,
            pet_photo_url: snapshot.pet_photo_url,
            is_owned: true,
            owner_label: "You".to_string(),
            is_npc: false,
            is_birthday_cat: false,
        });
    }

    for friend_email in sharing::accepted_friend_emails(state, &viewer.email) {
        let Some(friend_profile) = sharing::load_profile_by_email(state, &friend_email) else {
            continue;
        };
        let owner_label = sharing::user_label(state, &friend_email);
        for (pet_id, _) in sharing::pet_summaries_for_profile(&friend_profile) {
            let Some(snapshot) = pet_snapshot(&friend_profile, &pet_id) else {
                continue;
            };
            if !pet_is_alive(&snapshot) {
                continue;
            }
            let dedupe = format!("{}|{}", sharing::normalize_email(&friend_email), pet_id);
            if !seen.insert(dedupe) {
                continue;
            }
            cats.push(SceneCat {
                pet_id,
                owner_email: friend_email.clone(),
                pet_name: snapshot.pet_name,
                pet_color: snapshot.pet_color,
                pet_photo_url: snapshot.pet_photo_url,
                is_owned: false,
                owner_label: owner_label.clone(),
                is_npc: false,
                is_birthday_cat: false,
            });
        }
    }

    cats
}

fn cat_allowed_in_scene(state: &AppState, viewer: &UserProfile, owner_email: &str, pet_id: &str) -> bool {
    let today = chrono::Local::now().date_naive();
    if let Some(cats) = crate::birthday_party::list_party_scene_cats(state, viewer, today) {
        return cats
            .iter()
            .any(|cat| cat.pet_id == pet_id && cat.owner_email.eq_ignore_ascii_case(owner_email));
    }
    list_scene_cats(state, viewer)
        .iter()
        .any(|cat| cat.pet_id == pet_id && cat.owner_email.eq_ignore_ascii_case(owner_email))
}

pub fn action_by_id(action_id: &str) -> Option<&'static PlaydateActionDef> {
    if action_id == PROP_PLAYDATE_ACTION.id {
        return Some(&PROP_PLAYDATE_ACTION);
    }
    CAT_PLAYDATE_ACTIONS.iter().find(|action| action.id == action_id)
}

/// Cats need Buddies-level trust before bold friendly moves go well.
pub const BUDDIES_FRIENDSHIP_THRESHOLD: i32 = 55;

pub fn action_is_overly_friendly(action_id: &str) -> bool {
    matches!(action_id, "groom" | "friendly_brawl" | "play_together")
}

pub fn effective_friendship_delta(current_score: i32, action: &PlaydateActionDef) -> i32 {
    if current_score < BUDDIES_FRIENDSHIP_THRESHOLD
        && action_is_overly_friendly(action.id)
        && action.delta > 0
    {
        -action.delta
    } else {
        action.delta
    }
}

pub fn playdate_action_backfired(current_score: i32, action: &PlaydateActionDef) -> bool {
    effective_friendship_delta(current_score, action) < 0 && action.delta > 0
}

fn format_playdate_message(
    action: &PlaydateActionDef,
    actor_name: &str,
    target_name: &str,
    prop_label: Option<&str>,
) -> String {
    match action.id {
        "sniff" => format!("{actor_name} sniffed {target_name}."),
        "hiss" => format!("{actor_name} hissed at {target_name}!"),
        "friendly_brawl" => format!("{actor_name} and {target_name} had a friendly brawl!"),
        "groom" => format!("{actor_name} groomed {target_name} sweetly."),
        "chirp" => format!("{actor_name} chirped hello to {target_name}!"),
        "side_eye" => format!("{actor_name} gave {target_name} a side eye."),
        "play_together" => {
            let prop = prop_label.unwrap_or("play spot");
            format!("{actor_name} and {target_name} played together at the {prop}!")
        }
        _ => format!("{actor_name} interacted with {target_name}."),
    }
}

fn format_playdate_backfire_message(
    action: &PlaydateActionDef,
    actor_name: &str,
    target_name: &str,
    prop_label: Option<&str>,
) -> String {
    match action.id {
        "groom" => format!("{actor_name} tried to groom {target_name}, but it felt too familiar!"),
        "chirp" => format!("{actor_name} chirped at {target_name} a little too soon!"),
        "friendly_brawl" => {
            format!("{actor_name} and {target_name} brawled before they were ready for that!")
        }
        "play_together" => {
            let prop = prop_label.unwrap_or("play spot");
            format!(
                "{actor_name} wanted to play with {target_name} at the {prop}, but they aren't close enough yet!"
            )
        }
        _ => format!("{actor_name} was a bit too friendly with {target_name}."),
    }
}

pub fn adjust_friendship_on_profile(
    profile: &mut UserProfile,
    owner_a: &str,
    pet_a: &str,
    owner_b: &str,
    pet_b: &str,
    delta: i32,
) -> i32 {
    let key = friendship_key(owner_a, pet_a, owner_b, pet_b);
    let entry = profile.cat_friendships.entry(key).or_insert(0);
    *entry = (*entry + delta).clamp(-50, 100);
    *entry
}

pub async fn apply_playdate_interaction(
    state: &AppState,
    viewer_email: &str,
    request: &PlaydateInteractRequest,
) -> Result<PlaydateInteractResponse, &'static str> {
    let actor_owner = request.actor_owner.trim();
    let target_owner = request.target_owner.trim();
    let actor_pet_id = request.actor_pet_id.trim();
    let target_pet_id = request.target_pet_id.trim();

    if actor_owner.is_empty()
        || target_owner.is_empty()
        || actor_pet_id.is_empty()
        || target_pet_id.is_empty()
    {
        return Err("invalid");
    }
    if actor_owner == target_owner && actor_pet_id == target_pet_id {
        return Err("invalid");
    }

    let action = action_by_id(request.action.trim()).ok_or("invalid_action")?;
    if action.id == PROP_PLAYDATE_ACTION.id && request.prop_slot.as_deref().unwrap_or("").is_empty() {
        return Err("invalid_prop");
    }

    let viewer = crate::get_or_create_profile(state, viewer_email).await;
    if !cat_allowed_in_scene(state, &viewer, actor_owner, actor_pet_id)
        || !cat_allowed_in_scene(state, &viewer, target_owner, target_pet_id)
    {
        return Err("invalid_cat");
    }

    let actor_name = if crate::birthday_party::is_npc_party_cat(actor_owner, actor_pet_id) {
        crate::birthday_party::npc_display_name(actor_pet_id)
            .unwrap_or("Party guest")
            .to_string()
    } else {
        sharing::pet_name_for_owner(state, actor_owner, actor_pet_id)
    };
    let target_name = if crate::birthday_party::is_npc_party_cat(target_owner, target_pet_id) {
        crate::birthday_party::npc_display_name(target_pet_id)
            .unwrap_or("Party guest")
            .to_string()
    } else {
        sharing::pet_name_for_owner(state, target_owner, target_pet_id)
    };
    let prop_label = request.prop_slot.as_deref().map(decor_slot_label);

    let score_before =
        friendship_score(&viewer, actor_owner, actor_pet_id, target_owner, target_pet_id);
    let delta = effective_friendship_delta(score_before, action);
    let backfired = playdate_action_backfired(score_before, action);
    let message = if backfired {
        format_playdate_backfire_message(action, &actor_name, &target_name, prop_label)
    } else {
        format_playdate_message(action, &actor_name, &target_name, prop_label)
    };

    let mut score = score_before;
    let actor_is_npc = crate::birthday_party::is_npc_party_cat(actor_owner, actor_pet_id);
    let target_is_npc = crate::birthday_party::is_npc_party_cat(target_owner, target_pet_id);
    if actor_is_npc || target_is_npc {
        let today = chrono::Local::now().date_naive();
        if !crate::birthday_party::party_active_for_viewer(state, &viewer, today) {
            return Err("invalid_cat");
        }
        let mut profile = viewer.clone();
        score = adjust_friendship_on_profile(
            &mut profile,
            actor_owner,
            actor_pet_id,
            target_owner,
            target_pet_id,
            delta,
        );
        push_activity(
            &mut profile,
            &format!("Birthday party: {message} Friendship is now {score}."),
        );
        let _ = crate::save_profile(state, &profile).await;
    } else {
        let owners = [actor_owner, target_owner];
        for owner in owners {
            let mut profile = if owner.eq_ignore_ascii_case(viewer_email) {
                viewer.clone()
            } else {
                sharing::load_profile_by_email(state, owner).ok_or("invalid_owner")?
            };
            score = adjust_friendship_on_profile(
                &mut profile,
                actor_owner,
                actor_pet_id,
                target_owner,
                target_pet_id,
                delta,
            );
            if owner.eq_ignore_ascii_case(viewer_email) {
                push_activity(
                    &mut profile,
                    &format!("Playdate: {message} Friendship is now {score}."),
                );
            }
            let _ = crate::save_profile(state, &profile).await;
        }
    }

    let (label, emoji) = friendship_tier(score);
    Ok(PlaydateInteractResponse {
        ok: true,
        message,
        friendship_score: score,
        friendship_label: label.to_string(),
        friendship_emoji: emoji.to_string(),
        actor_pet_id: actor_pet_id.to_string(),
        target_pet_id: target_pet_id.to_string(),
        backfired,
    })
}

fn render_playdate_cat_avatar(pet_name: &str, pet_id: &str, photo_url: Option<&str>) -> String {
    let display_name = if pet_name.trim().is_empty() {
        "Cat"
    } else {
        pet_name
    };
    let photo_src = photo_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(escape_html_attr)
        .unwrap_or_else(|| "/cinderanimate.png".to_string());
    format!(
        r#"<div class="pet-cinder-stage pet-cinder-stage-compact" data-cinder-stage="pet" data-pet-name="{display_name}" data-pet-id="{pet_id}">
      <p class="cinder-pet-label">{display_name}</p>
      <div class="cinder-pet-image-wrap">
        <img class="cinder-pet-image" src="{photo_src}" alt="{display_name} virtual pet" />
      </div>
    </div>"#,
        display_name = escape_html(display_name),
        pet_id = escape_html_attr(pet_id),
        photo_src = photo_src,
    )
}

fn scene_cat_display_name(pet_name: &str) -> &str {
    if pet_name.trim().is_empty() {
        "Cat"
    } else {
        pet_name
    }
}

fn scene_cat_role_label(cat: &SceneCat, is_play_as: bool, party_mode: bool) -> String {
    let display_name = scene_cat_display_name(&cat.pet_name);
    if party_mode && is_play_as && cat.is_birthday_cat {
        return format!("Birthday star · {display_name}");
    }
    if is_play_as {
        return format!("Playing as {display_name}");
    }
    if party_mode && cat.is_birthday_cat {
        return format!("Birthday cat · {display_name}");
    }
    if party_mode && cat.is_npc {
        return "Party guest".to_string();
    }
    if party_mode && !cat.is_owned {
        return format!("{} came to party", cat.owner_label);
    }
    if cat.is_owned {
        "Your housemate".to_string()
    } else {
        format!("{}'s cat", cat.owner_label)
    }
}

fn render_scene_cat(
    cat: &SceneCat,
    slot: usize,
    best_friendship: i32,
    best_label: &str,
    best_emoji: &str,
    is_play_as: bool,
    party_mode: bool,
) -> String {
    let guest_class = if cat.is_owned { "" } else { " cat-home-playdate-guest" };
    let play_as_class = if is_play_as { " cat-home-play-as" } else { "" };
    let is_housemate = cat.is_owned && !is_play_as;
    let housemate_class = if is_housemate { " cat-home-housemate" } else { "" };
    let birthday_class = if party_mode && cat.is_birthday_cat {
        " cat-home-birthday-cat"
    } else {
        ""
    };
    let npc_class = if cat.is_npc { " cat-home-npc-guest" } else { "" };
    let display_name = scene_cat_display_name(&cat.pet_name);
    let role_label = scene_cat_role_label(cat, is_play_as, party_mode);
    let color_hint = if cat.pet_color.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="cat-home-pet-color-hint">{color}</p>"#,
            color = escape_html(cat.pet_color.trim())
        )
    };
    let birthday_heart = if party_mode && cat.is_birthday_cat {
        r#"<span class="cat-home-pet-bubble-cake" aria-hidden="true">🎂</span>"#
    } else {
        ""
    };
    format!(
        r#"<div class="cat-home-pet-stage cat-home-playdate-cat cat-home-pet-slot-{slot}{guest_class}{play_as_class}{housemate_class}{birthday_class}{npc_class}" data-pet-id="{pet_id}" data-pet-owner="{owner}" data-pet-name="{pet_name}" data-owner-label="{owner_label}" data-is-owned="{is_owned}" data-is-housemate="{is_housemate}" data-is-npc="{is_npc}" data-is-birthday-cat="{is_birthday_cat}" data-friendship-score="{friendship_score}" tabindex="0" role="button" aria-label="{display_name}, {role_label}. Friendship {friendship_label}">
  <div class="cat-home-pet-stack">
    <p class="cat-home-pet-bubble" role="note"><span class="cat-home-pet-bubble-name">{display_name}</span>{birthday_heart}<span class="cat-home-pet-bubble-heart" aria-hidden="true">💗</span></p>
    <p class="cat-home-pet-role-chip">{role_label}</p>
    <p class="cat-home-friendship-badge" aria-hidden="true">{friendship_emoji} {friendship_label} · {friendship_level}</p>
    {avatar}
    {color_hint}
  </div>
</div>"#,
        slot = slot,
        guest_class = guest_class,
        play_as_class = play_as_class,
        housemate_class = housemate_class,
        birthday_class = birthday_class,
        npc_class = npc_class,
        pet_id = escape_html_attr(&cat.pet_id),
        owner = escape_html_attr(&cat.owner_email),
        pet_name = escape_html_attr(&cat.pet_name),
        owner_label = escape_html_attr(&cat.owner_label),
        is_owned = if cat.is_owned { "true" } else { "false" },
        is_housemate = if is_housemate { "true" } else { "false" },
        is_npc = if cat.is_npc { "true" } else { "false" },
        is_birthday_cat = if cat.is_birthday_cat { "true" } else { "false" },
        friendship_score = best_friendship,
        friendship_level = escape_html(&format_friendship_level_display(best_friendship)),
        friendship_label = escape_html(best_label),
        friendship_emoji = best_emoji,
        display_name = escape_html(display_name),
        role_label = escape_html(&role_label),
        birthday_heart = birthday_heart,
        avatar = render_playdate_cat_avatar(
            &cat.pet_name,
            &cat.pet_id,
            cat.pet_photo_url.as_deref(),
        ),
        color_hint = color_hint,
    )
}

fn best_friendship_for_cat(viewer: &UserProfile, cat: &SceneCat, all_cats: &[SceneCat]) -> i32 {
    all_cats
        .iter()
        .filter(|other| !(other.pet_id == cat.pet_id && other.owner_email == cat.owner_email))
        .map(|other| {
            friendship_score(
                viewer,
                &cat.owner_email,
                &cat.pet_id,
                &other.owner_email,
                &other.pet_id,
            )
        })
        .max()
        .unwrap_or(0)
}

fn same_scene_cat(left: &SceneCat, right: &SceneCat) -> bool {
    left.pet_id == right.pet_id && left.owner_email == right.owner_email
}

fn find_play_as_cat<'a>(
    cats: &'a [SceneCat],
    play_as_pet_id: &str,
    viewer_email: &str,
) -> Option<&'a SceneCat> {
    let viewer_email = sharing::normalize_email(viewer_email);
    cats.iter().find(|cat| {
        cat.is_owned
            && cat.pet_id == play_as_pet_id
            && sharing::normalize_email(&cat.owner_email) == viewer_email
    })
}

fn friendship_target_role(cat: &SceneCat) -> String {
    if cat.is_owned {
        "Your housemate".to_string()
    } else {
        format!("{}'s cat", cat.owner_label)
    }
}

fn display_friendship_for_cat(
    viewer: &UserProfile,
    cat: &SceneCat,
    play_as: Option<&SceneCat>,
    all_cats: &[SceneCat],
    is_play_as: bool,
) -> i32 {
    if is_play_as {
        return best_friendship_for_cat(viewer, cat, all_cats);
    }
    let Some(play_as) = play_as else {
        return best_friendship_for_cat(viewer, cat, all_cats);
    };
    friendship_score(
        viewer,
        &play_as.owner_email,
        &play_as.pet_id,
        &cat.owner_email,
        &cat.pet_id,
    )
}

fn render_friendship_row(
    viewer: &UserProfile,
    play_as: &SceneCat,
    other: &SceneCat,
) -> String {
    let score = friendship_score(
        viewer,
        &play_as.owner_email,
        &play_as.pet_id,
        &other.owner_email,
        &other.pet_id,
    );
    let (label, emoji) = friendship_tier(score);
    let percent = friendship_progress_percent(score);
    let level_display = format_friendship_level_display(score);
    let name = scene_cat_display_name(&other.pet_name);
    let role = friendship_target_role(other);
    let meter_label = format!("Friendship with {name}: {label} ({level_display})");
    let score_tier_attr = if score < 0 {
        r#" data-score-tier="negative""#
    } else {
        ""
    };

    format!(
        r#"<li class="cat-home-friendship-row"{score_tier_attr} data-target-pet-id="{pet_id}" data-target-owner="{owner}">
  <div class="cat-home-friendship-meta">
    <span class="cat-home-friendship-name">{name}</span>
    <span class="cat-home-friendship-role">{role}</span>
  </div>
  <div class="cat-home-friendship-meter" role="meter" aria-valuenow="{score}" aria-valuemin="{score_min}" aria-valuemax="{score_max}" aria-label="{meter_label}">
    <div class="cat-home-friendship-meter-fill" style="width: {percent}%"></div>
  </div>
  <p class="cat-home-friendship-tier">{emoji} {label} · {level_display}</p>
</li>"#,
        score_tier_attr = score_tier_attr,
        pet_id = escape_html_attr(&other.pet_id),
        owner = escape_html_attr(&other.owner_email),
        name = escape_html(name),
        role = escape_html(&role),
        score = score,
        score_min = FRIENDSHIP_SCORE_MIN,
        score_max = FRIENDSHIP_SCORE_MAX,
        meter_label = escape_html(&meter_label),
        percent = percent,
        emoji = emoji,
        label = escape_html(label),
        level_display = escape_html(&level_display),
    )
}

pub fn render_friendships_panel(
    viewer: &UserProfile,
    play_as_pet_id: &str,
    cats: &[SceneCat],
) -> String {
    if cats.len() < 2 {
        return r#"<section class="cat-home-friendships-panel cat-home-friendships-panel--empty" aria-label="Cat friendships">
  <p class="cat-home-friendships-empty">Invite a friend&apos;s cat over to start tracking friendships!</p>
</section>"#
            .to_string();
    }

    let Some(play_as) = find_play_as_cat(cats, play_as_pet_id, &viewer.email) else {
        return String::new();
    };

    let play_as_name = scene_cat_display_name(&play_as.pet_name);
    let mut others: Vec<&SceneCat> = cats
        .iter()
        .filter(|cat| !same_scene_cat(cat, play_as))
        .collect();
    others.sort_by(|left, right| {
        let left_score = friendship_score(
            viewer,
            &play_as.owner_email,
            &play_as.pet_id,
            &left.owner_email,
            &left.pet_id,
        );
        let right_score = friendship_score(
            viewer,
            &play_as.owner_email,
            &play_as.pet_id,
            &right.owner_email,
            &right.pet_id,
        );
        right_score
            .cmp(&left_score)
            .then_with(|| left.pet_name.cmp(&right.pet_name))
    });

    let rows = others
        .iter()
        .map(|other| render_friendship_row(viewer, play_as, other))
        .collect::<String>();

    format!(
        r#"<section class="cat-home-friendships-panel" aria-labelledby="cat-home-friendships-title" data-play-as-pet-id="{play_as_pet_id}">
  <div class="cat-home-friendships-header">
    <h3 id="cat-home-friendships-title">{play_as_name}&apos;s friendships</h3>
    <p class="field-hint cat-home-friendships-lead">Bars show overall friendship; points below are progress toward the next level.</p>
  </div>
  <ul class="cat-home-friendships-list">{rows}</ul>
</section>"#,
        play_as_pet_id = escape_html_attr(play_as_pet_id),
        play_as_name = escape_html(play_as_name),
        rows = rows,
    )
}

fn order_scene_cats_for_play_as(
    mut cats: Vec<SceneCat>,
    play_as_pet_id: &str,
    viewer_email: &str,
) -> Vec<SceneCat> {
    let viewer_email = sharing::normalize_email(viewer_email);
    if let Some(index) = cats.iter().position(|cat| {
        cat.is_owned
            && cat.pet_id == play_as_pet_id
            && sharing::normalize_email(&cat.owner_email) == viewer_email
    }) {
        let play_as = cats.remove(index);
        cats.insert(0, play_as);
    }
    cats
}

pub fn render_playdate_scene(
    state: &AppState,
    viewer: &UserProfile,
    play_as_pet_id: &str,
    room: &str,
    rug_layer: &str,
    bed_layer: &str,
    toy_layer: &str,
    plant_layer: &str,
    equipped_strip: &str,
    party: Option<&crate::birthday_party::BirthdayPartyContext>,
) -> String {
    let today = chrono::Local::now().date_naive();
    let cats = if let Some(_) = party {
        crate::birthday_party::list_party_scene_cats(state, viewer, today).unwrap_or_default()
    } else {
        list_scene_cats(state, viewer)
    };
    let cats = order_scene_cats_for_play_as(cats, play_as_pet_id, &viewer.email);
    let party_mode = party.is_some();
    let cat_count = cats.len();
    let owned_count = cats.iter().filter(|cat| cat.is_owned).count();
    let friend_count = cats
        .iter()
        .filter(|cat| !cat.is_owned && !cat.is_npc)
        .count();
    let npc_count = cats.iter().filter(|cat| cat.is_npc).count();
    let _ = npc_count;
    let mood = if let Some(ctx) = party {
        crate::birthday_party::party_mood_message(ctx, &cats)
    } else if owned_count > 1 && friend_count > 0 {
        format!(
            "The family cat home — your {} cats and {} friend {} are hanging out!",
            owned_count,
            friend_count,
            if friend_count == 1 { "cat" } else { "cats" },
        )
    } else if owned_count > 1 {
        "The family cat home — your household cats are ready for a playdate. Tap a housemate or decor!".to_string()
    } else if cat_count > 1 {
        format!(
            "The family cat home — {} cats are hanging out for a virtual playdate!",
            cat_count,
        )
    } else if let Some(cat) = cats.first() {
        format!(
            "{} is relaxing in the family cat home — invite a friend's cat for a playdate!",
            escape_html(&cat.pet_name)
        )
    } else {
        "The family cat home is ready for visitors.".to_string()
    };

    let play_as_cat = find_play_as_cat(&cats, play_as_pet_id, &viewer.email);
    let cats_layer = if cats.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="cat-home-cats-layer" data-playdate-cat-count="{count}">{cats_html}</div>"#,
            count = cat_count,
            cats_html = cats
                .iter()
                .enumerate()
                .map(|(slot, cat)| {
                    let is_play_as = cat.pet_id == play_as_pet_id
                        && cat.is_owned
                        && sharing::normalize_email(&cat.owner_email)
                            == sharing::normalize_email(&viewer.email);
                    let score =
                        display_friendship_for_cat(viewer, cat, play_as_cat, &cats, is_play_as);
                    let (label, emoji) = friendship_tier(score);
                    render_scene_cat(cat, slot, score, label, emoji, is_play_as, party_mode)
                })
                .collect::<String>(),
        )
    };
    let friendships_panel =
        render_friendships_panel(viewer, play_as_pet_id, &cats);

    let friendship_json = serde_json::to_string(
        &cats
            .iter()
            .flat_map(|left| {
                cats.iter().filter(move |right| {
                    !(left.pet_id == right.pet_id && left.owner_email == right.owner_email)
                }).map(move |right| {
                    serde_json::json!({
                        "key": friendship_key(&left.owner_email, &left.pet_id, &right.owner_email, &right.pet_id),
                        "score": friendship_score(viewer, &left.owner_email, &left.pet_id, &right.owner_email, &right.pet_id),
                    })
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());

    let floor_decor = format!("{rug_layer}{bed_layer}");
    let accent_decor = format!("{toy_layer}{plant_layer}");
    let party_class = if party_mode {
        " cat-home-scene--birthday-party"
    } else {
        ""
    };
    let party_banner = party
        .map(crate::birthday_party::render_cat_home_banner)
        .unwrap_or_default();
    let party_overlay = if party_mode {
        crate::birthday_party::render_party_overlay()
    } else {
        String::new()
    };
    let playdate_hint = if party_mode {
        "Tap birthday guests and decor for party playdates"
    } else {
        "Tap cats or decor for playdates"
    };

    format!(
        r#"<div class="cat-home-scene cat-home-playdate-scene cat-home-scene--immersive{party_class}" data-room="{room}" data-play-as-pet-id="{play_as_pet_id}" data-birthday-party="{party_mode}">
  <div class="cat-home-room-bg" aria-hidden="true"></div>
  {party_overlay}
  {party_banner}
  <div class="cat-home-scene-ambient" aria-hidden="true">
    <span class="cat-home-ambient-orb cat-home-ambient-orb--1"></span>
    <span class="cat-home-ambient-orb cat-home-ambient-orb--2"></span>
    <span class="cat-home-ambient-orb cat-home-ambient-orb--3"></span>
    <span class="cat-home-ambient-dust cat-home-ambient-dust--1"></span>
    <span class="cat-home-ambient-dust cat-home-ambient-dust--2"></span>
    <span class="cat-home-ambient-dust cat-home-ambient-dust--3"></span>
    <span class="cat-home-ambient-dust cat-home-ambient-dust--4"></span>
  </div>
  <div class="cat-home-scene-vignette" aria-hidden="true"></div>
  <div class="cat-home-scene-floor-shadow" aria-hidden="true"></div>
  {equipped_strip}
  <div class="cat-home-decor-layer cat-home-decor-layer--floor">{floor_decor}</div>
  {cats_layer}
  <div class="cat-home-decor-layer cat-home-decor-layer--accent">{accent_decor}</div>
  <div class="cat-home-scene-hud">
    <p class="cat-home-mood">{mood}</p>
    <p class="cat-home-playdate-hint">{playdate_hint}</p>
  </div>
  <script type="application/json" class="playdate-friendships-data">{friendship_json}</script>
</div>
{friendships_panel}"#,
        room = escape_html_attr(room),
        play_as_pet_id = escape_html_attr(play_as_pet_id),
        equipped_strip = equipped_strip,
        floor_decor = floor_decor,
        cats_layer = cats_layer,
        accent_decor = accent_decor,
        mood = mood,
        friendship_json = friendship_json,
        friendships_panel = friendships_panel,
        party_class = party_class,
        party_mode = if party_mode { "true" } else { "false" },
        party_banner = party_banner,
        party_overlay = party_overlay,
        playdate_hint = playdate_hint,
    )
}

pub fn render_interactive_prop(
    class_suffix: &str,
    slot: &str,
    decor_name: &str,
    emoji: &str,
) -> String {
    format!(
        r#"<button type="button" class="cat-home-prop cat-home-interactive cat-home-{suffix}" data-prop-slot="{slot}" data-prop-name="{name}" aria-label="{name}, playdate spot">
  <span class="cat-home-prop-emoji" aria-hidden="true">{emoji}</span>
  <span class="cat-home-prop-label">{name}</span>
  <span class="cat-home-prop-hint">Playdate</span>
</button>"#,
        suffix = class_suffix,
        slot = escape_html_attr(slot),
        name = escape_html(decor_name),
        emoji = emoji,
    )
}

fn decor_slot_label(slot: &str) -> &'static str {
    match slot {
        "rug" => "rug",
        "bed" => "bed",
        "toy" => "toy",
        "plant" => "plant",
        "room" => "room",
        _ => "play spot",
    }
}
