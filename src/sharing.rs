use crate::{
    escape_html, escape_html_attr, household_pet_is_complete, memorial, pet_snapshot,
    profile_has_pet, user_for_email, visible_calendar_events, AppState, CalendarEvent, UserProfile,
    PRIMARY_PET_ID,
};
use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use crate::storage::{StoredFriendRequest, StoredFriendSummary, StoredPetShare};

pub const FRIEND_STATUS_PENDING: &str = "pending";
pub const FRIEND_STATUS_ACCEPTED: &str = "accepted";
pub const FRIEND_STATUS_DECLINED: &str = "declined";

pub const SHARE_STATUS_PENDING: &str = "pending";
pub const SHARE_STATUS_ACCEPTED: &str = "accepted";
pub const SHARE_STATUS_DECLINED: &str = "declined";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendRequest {
    pub id: String,
    pub from_email: String,
    pub to_email: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendSummary {
    pub friend_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PetShare {
    pub id: String,
    pub owner_email: String,
    pub shared_with_email: String,
    pub pet_id: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessiblePetSummary {
    pub pet_id: String,
    pub pet_name: String,
    pub owner_email: String,
    pub owner_label: String,
    pub is_owned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetCareTarget {
    pub viewer_email: String,
    pub owner_email: String,
    pub pet_id: String,
    pub is_shared: bool,
}

#[derive(Deserialize)]
pub struct FriendRequestForm {
    pub friend_email: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendLookupError {
    Empty,
    NotFound,
}

pub fn resolve_friend_identifier(state: &AppState, raw: &str) -> Result<String, FriendLookupError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(FriendLookupError::Empty);
    }

    if trimmed.contains('@') {
        return Ok(normalize_email(trimmed));
    }

    state
        .storage
        .email_for_username(trimmed)
        .map_err(|_| FriendLookupError::NotFound)?
        .map(|email| normalize_email(&email))
        .ok_or(FriendLookupError::NotFound)
}

pub fn accepted_friend_emails(state: &AppState, email: &str) -> Vec<String> {
    state
        .storage
        .list_friends(email)
        .unwrap_or_default()
        .into_iter()
        .map(|summary| summary.friend_email)
        .collect()
}

pub fn friends_pending_count(state: &AppState, email: &str) -> usize {
    let incoming = state
        .storage
        .list_incoming_friend_requests(email)
        .map(|items| items.len())
        .unwrap_or(0);
    let shares = state
        .storage
        .list_incoming_pet_shares(email)
        .map(|items| items.len())
        .unwrap_or(0);
    incoming + shares
}

pub fn render_friends_tab_label(state: &AppState, email: &str) -> String {
    let pending = friends_pending_count(state, email);
    if pending > 0 {
        format!(
            r#"Friends <span class="friends-tab-badge" aria-label="{pending} pending">{pending}</span>"#
        )
    } else {
        "Friends".to_string()
    }
}

#[derive(Deserialize)]
pub struct FriendRespondForm {
    pub request_id: String,
    pub action: String,
}

#[derive(Deserialize)]
pub struct PetShareForm {
    pub friend_email: String,
    pub pet_id: String,
}

#[derive(Deserialize)]
pub struct PetShareRespondForm {
    pub share_id: String,
    pub action: String,
}

#[derive(Deserialize)]
pub struct PetShareRevokeForm {
    pub share_id: String,
}

pub fn normalize_email(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn user_label(state: &AppState, email: &str) -> String {
    user_for_email(state, email)
        .map(|user| {
            let username = user.username.trim();
            if username.is_empty() {
                email.to_string()
            } else {
                username.to_string()
            }
        })
        .unwrap_or_else(|| email.to_string())
}

pub fn active_pet_owner_email(profile: &UserProfile) -> &str {
    profile
        .active_pet_owner_email
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(profile.email.as_str())
}

pub fn is_viewing_shared_pet(profile: &UserProfile) -> bool {
    profile
        .active_pet_owner_email
        .as_deref()
        .is_some_and(|owner| !owner.eq_ignore_ascii_case(&profile.email))
}

pub fn pet_summaries_for_profile(profile: &UserProfile) -> Vec<(String, String)> {
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

pub fn owner_has_pet(profile: &UserProfile, pet_id: &str) -> bool {
    pet_snapshot(profile, pet_id).is_some()
}

fn stored_friend_request(req: StoredFriendRequest) -> FriendRequest {
    FriendRequest {
        id: req.id,
        from_email: req.from_email,
        to_email: req.to_email,
        status: req.status,
        created_at: req.created_at,
    }
}

fn stored_friend_summary(summary: StoredFriendSummary) -> FriendSummary {
    FriendSummary {
        friend_email: summary.friend_email,
    }
}

fn stored_pet_share(share: StoredPetShare) -> PetShare {
    PetShare {
        id: share.id,
        owner_email: share.owner_email,
        shared_with_email: share.shared_with_email,
        pet_id: share.pet_id,
        status: share.status,
        created_at: share.created_at,
    }
}

pub fn load_profile_by_email(state: &AppState, email: &str) -> Option<UserProfile> {
    state
        .storage
        .load_profile(email)
        .ok()
        .flatten()
        .map(|mut profile| {
            profile.email = email.to_string();
            profile
        })
}

pub fn pet_name_for_owner(state: &AppState, owner_email: &str, pet_id: &str) -> String {
    load_profile_by_email(state, owner_email)
        .and_then(|profile| pet_snapshot(&profile, pet_id))
        .map(|snapshot| snapshot.pet_name)
        .unwrap_or_else(|| "Cat".to_string())
}

pub fn list_accessible_pets(state: &AppState, viewer: &UserProfile) -> Vec<AccessiblePetSummary> {
    let mut pets = Vec::new();
    for (pet_id, pet_name) in pet_summaries_for_profile(viewer) {
        pets.push(AccessiblePetSummary {
            pet_id,
            pet_name,
            owner_email: viewer.email.clone(),
            owner_label: "You".to_string(),
            is_owned: true,
        });
    }

    if let Ok(shares) = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
    {
        for share in shares.into_iter().map(stored_pet_share) {
            let Some(owner_profile) = load_profile_by_email(state, &share.owner_email) else {
                continue;
            };
            let Some(snapshot) = pet_snapshot(&owner_profile, &share.pet_id) else {
                continue;
            };
            pets.push(AccessiblePetSummary {
                pet_id: share.pet_id,
                pet_name: snapshot.pet_name,
                owner_email: share.owner_email.clone(),
                owner_label: user_label(state, &share.owner_email),
                is_owned: false,
            });
        }
    }

    pets
}

pub fn can_access_shared_pet(
    state: &AppState,
    viewer_email: &str,
    owner_email: &str,
    pet_id: &str,
) -> bool {
    if viewer_email.eq_ignore_ascii_case(owner_email) {
        return true;
    }
    state
        .storage
        .has_accepted_pet_share(owner_email, viewer_email, pet_id)
        .unwrap_or(false)
}

pub fn resolve_pet_care_target(
    state: &AppState,
    viewer: &UserProfile,
    pet_id: &str,
    owner_email: Option<&str>,
) -> Option<PetCareTarget> {
    let pet_id = pet_id.trim();
    if pet_id.is_empty() {
        return None;
    }

    if let Some(owner) = owner_email {
        let owner = normalize_email(owner);
        if owner == normalize_email(&viewer.email) {
            if owner_has_pet(viewer, pet_id) {
                return Some(PetCareTarget {
                    viewer_email: viewer.email.clone(),
                    owner_email: viewer.email.clone(),
                    pet_id: pet_id.to_string(),
                    is_shared: false,
                });
            }
            return None;
        }
        if !can_access_shared_pet(state, &viewer.email, &owner, pet_id) {
            return None;
        }
        let owner_profile = load_profile_by_email(state, &owner)?;
        if !owner_has_pet(&owner_profile, pet_id) {
            return None;
        }
        return Some(PetCareTarget {
            viewer_email: viewer.email.clone(),
            owner_email: owner,
            pet_id: pet_id.to_string(),
            is_shared: true,
        });
    }

    if owner_has_pet(viewer, pet_id) {
        return Some(PetCareTarget {
            viewer_email: viewer.email.clone(),
            owner_email: viewer.email.clone(),
            pet_id: pet_id.to_string(),
            is_shared: false,
        });
    }

    None
}

pub fn resolve_active_pet_care_target(
    state: &AppState,
    viewer: &UserProfile,
) -> Option<PetCareTarget> {
    let owner = active_pet_owner_email(viewer);
    resolve_pet_care_target(state, viewer, &viewer.active_pet_id, Some(owner))
}

pub fn set_active_pet_selection(
    profile: &mut UserProfile,
    pet_id: &str,
    owner_email: Option<&str>,
) -> bool {
    let owner = owner_email
        .map(normalize_email)
        .filter(|value| !value.is_empty());
    let changed = profile.active_pet_id != pet_id
        || profile.active_pet_owner_email.as_deref() != owner.as_deref();
    profile.active_pet_id = pet_id.to_string();
    profile.active_pet_owner_email = owner;
    changed
}

pub fn accessible_pet_exists(
    state: &AppState,
    viewer: &UserProfile,
    pet_id: &str,
    owner_email: Option<&str>,
) -> bool {
    resolve_pet_care_target(state, viewer, pet_id, owner_email).is_some()
}

pub fn apply_snapshot_to_profile_view(mut view: UserProfile, snapshot: &crate::PetSnapshot) -> UserProfile {
    view.pet_name = snapshot.pet_name.clone();
    view.pet_breed = snapshot.pet_breed.clone();
    view.pet_color = snapshot.pet_color.clone();
    view.pet_mood = snapshot.pet_mood.clone();
    view.pet_age_weeks = snapshot.pet_age_weeks;
    view.pet_age_years = snapshot.pet_age_years;
    view.pet_birth_date = snapshot.pet_birth_date.clone();
    view.last_vet_date = snapshot.last_vet_date.clone();
    view.never_been_to_vet = snapshot.never_been_to_vet;
    view.pet_conditions = snapshot.pet_conditions.clone();
    view.pet_medications = snapshot.pet_medications.clone();
    view.pet_indoor_outdoor = snapshot.pet_indoor_outdoor.clone();
    view.vaccine_history = snapshot.vaccine_history.clone();
    view.pet_vaccines_unknown = snapshot.pet_vaccines_unknown;
    view.care_schedule = snapshot.care_schedule.clone();
    view.pet_photo_url = snapshot.pet_photo_url.clone();
    view.pet_video_url = snapshot.pet_video_url.clone();
    view.pet_video_clip_start = snapshot.pet_video_clip_start;
    view.pet_video_clip_duration = snapshot.pet_video_clip_duration;
    view.pet_video_zoom = snapshot.pet_video_zoom;
    view.pet_video_offset_x = snapshot.pet_video_offset_x;
    view.pet_video_offset_y = snapshot.pet_video_offset_y;
    view.deceased = snapshot.deceased;
    view.deceased_at = snapshot.deceased_at.clone();
    view.memorial_videos = snapshot.memorial_videos.clone();
    view.memorial_comfort_seen = snapshot.memorial_comfort_seen;
    view
}

pub fn active_pet_view_profile(state: &AppState, viewer: &UserProfile) -> UserProfile {
    let Some(target) = resolve_active_pet_care_target(state, viewer) else {
        return viewer.clone();
    };
    let owner_profile = if target.is_shared {
        load_profile_by_email(state, &target.owner_email).unwrap_or_else(|| viewer.clone())
    } else {
        viewer.clone()
    };
    let Some(snapshot) = pet_snapshot(&owner_profile, &target.pet_id) else {
        return viewer.clone();
    };
    let mut view = owner_profile;
    view.active_pet_id = target.pet_id.clone();
    view.active_pet_owner_email = if target.is_shared {
        Some(target.owner_email.clone())
    } else {
        None
    };
    apply_snapshot_to_profile_view(view, &snapshot)
}

pub fn tasks_view_profile(state: &AppState, viewer: &UserProfile) -> UserProfile {
    let Some(target) = resolve_active_pet_care_target(state, viewer) else {
        return viewer.clone();
    };
    if !target.is_shared {
        return viewer.clone();
    }
    let Some(mut owner) = load_profile_by_email(state, &target.owner_email) else {
        return viewer.clone();
    };
    owner.active_pet_id = target.pet_id.clone();
    owner.active_pet_owner_email = Some(target.owner_email.clone());
    owner.tasks = owner
        .tasks
        .iter()
        .filter(|task| task.pet_id == target.pet_id)
        .cloned()
        .collect();
    owner
}

pub fn calendar_view_profile(state: &AppState, viewer: &UserProfile) -> UserProfile {
    let mut merged = viewer.clone();

    if let Ok(shares) = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
    {
        for share in shares {
            let Some(owner) = load_profile_by_email(state, &share.owner_email) else {
                continue;
            };
            for task in &owner.tasks {
                if task.pet_id == share.pet_id {
                    merged.tasks.push(task.clone());
                }
            }
            merged
                .user_calendar_events
                .extend(owner.user_calendar_events.clone());
        }
    }

    merged
}

pub fn visible_calendar_events_for_viewer(
    state: &AppState,
    viewer: &UserProfile,
    reference_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let mut events = visible_calendar_events(viewer, reference_date);
    let today = Local::now().date_naive();
    let horizon = today + Duration::days(730);

    if let Ok(shares) = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
    {
        for share in shares {
            let Some(owner) = load_profile_by_email(state, &share.owner_email) else {
                continue;
            };
            events.extend(owner.user_calendar_events.iter().cloned());
            let premium =
                crate::entitlements::can_access_health_records(owner.premium_unlocked, &owner.email);
            let Some(snapshot) = pet_snapshot(&owner, &share.pet_id) else {
                continue;
            };
            if premium {
                events.extend(crate::generate_vet_calendar_events_for_snapshot(
                    &snapshot,
                    reference_date,
                ));
                events.extend(crate::generate_vaccine_calendar_events_for_snapshot(
                    &snapshot,
                    reference_date,
                ));
            }
            events.extend(crate::generate_birthday_calendar_events_for_snapshot(
                &snapshot, today, horizon,
            ));
            events.extend(crate::generate_daily_care_calendar_events_for_snapshot(
                &snapshot, today, horizon,
            ));
        }
    }

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

pub fn render_shared_pet_banner(state: &AppState, viewer: &UserProfile) -> String {
    if !is_viewing_shared_pet(viewer) {
        return String::new();
    }
    let owner = active_pet_owner_email(viewer);
    let pet_name = pet_name_for_owner(state, owner, &viewer.active_pet_id);
    format!(
        r#"<p class="shared-pet-banner" role="status">Helping care for <strong>{pet}</strong> · shared by {owner}</p>"#,
        pet = escape_html(&pet_name),
        owner = escape_html(&user_label(state, owner)),
    )
}

pub fn render_pet_switcher(state: &AppState, profile: &UserProfile) -> String {
    let pets = list_accessible_pets(state, profile);
    if pets.len() <= 1 {
        return String::new();
    }

    let active_id = profile.active_pet_id.as_str();
    let active_owner = active_pet_owner_email(profile);
    let active_index = pets
        .iter()
        .position(|pet| pet.pet_id == active_id && pet.owner_email == active_owner)
        .unwrap_or(0);
    let prev_idx = if active_index == 0 {
        pets.len() - 1
    } else {
        active_index - 1
    };
    let next_idx = (active_index + 1) % pets.len();
    let prev = &pets[prev_idx];
    let next = &pets[next_idx];

    let tabs = pets
        .iter()
        .map(|pet| {
            let active = pet.pet_id == active_id && pet.owner_email == active_owner;
            let active_class = if active {
                " pet-switcher-tab-active"
            } else {
                ""
            };
            let angel = memorial::pet_switcher_angel_suffix(profile, &pet.pet_id, &pet.owner_email);
            let label = if pet.is_owned {
                format!("{}{}", escape_html(&pet.pet_name), angel)
            } else {
                format!(
                    "{} · {}",
                    escape_html(&pet.pet_name),
                    escape_html(&pet.owner_label)
                )
            };
            format!(
                r#"<a href="/home?tab=pet&amp;pet={pet_id}&amp;pet_owner={owner}" class="pet-switcher-tab{active_class}" aria-current="{current}">{label}</a>"#,
                pet_id = escape_html_attr(&pet.pet_id),
                owner = escape_html_attr(&pet.owner_email),
                active_class = active_class,
                current = if active { "page" } else { "false" },
                label = label,
            )
        })
        .collect::<String>();

    format!(
        r#"<nav class="pet-switcher" aria-label="Switch cat">
  <button type="button" class="pet-switcher-nav" data-pet-target="{prev_id}" data-pet-owner="{prev_owner}" aria-label="Previous cat">‹</button>
  <div class="pet-switcher-tabs">{tabs}</div>
  <button type="button" class="pet-switcher-nav" data-pet-target="{next_id}" data-pet-owner="{next_owner}" aria-label="Next cat">›</button>
  <p class="field-hint pet-switcher-count">{position} of {total} cats</p>
</nav>"#,
        prev_id = escape_html_attr(&prev.pet_id),
        prev_owner = escape_html_attr(&prev.owner_email),
        next_id = escape_html_attr(&next.pet_id),
        next_owner = escape_html_attr(&next.owner_email),
        tabs = tabs,
        position = active_index + 1,
        total = pets.len(),
    )
}

fn render_pet_share_options(owned_pets: &[(String, String)]) -> String {
    if owned_pets.is_empty() {
        return r#"<option value="">Set up a cat first</option>"#.to_string();
    }
    owned_pets
        .iter()
        .map(|(pet_id, pet_name)| {
            format!(
                r#"<option value="{}">{}</option>"#,
                escape_html_attr(pet_id),
                escape_html(pet_name),
            )
        })
        .collect()
}

pub fn render_tasks_shared_banner(profile: &UserProfile, state: &AppState) -> String {
    if !is_viewing_shared_pet(profile) {
        return String::new();
    }
    let owner = active_pet_owner_email(profile);
    let pet_name = pet_name_for_owner(state, owner, &profile.active_pet_id);
    format!(
        r#"<p class="shared-care-banner" role="status">You can complete <strong>{pet}</strong>'s care tasks and earn paw points — changes save to {owner}'s schedule.</p>"#,
        pet = escape_html(&pet_name),
        owner = escape_html(&user_label(state, owner)),
    )
}

pub fn render_calendar_shared_banner(state: &AppState, viewer: &UserProfile) -> String {
    let accepted = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
        .unwrap_or_default();
    if accepted.is_empty() && !is_viewing_shared_pet(viewer) {
        return String::new();
    }
    if is_viewing_shared_pet(viewer) {
        let owner = active_pet_owner_email(viewer);
        let pet_name = pet_name_for_owner(state, owner, &viewer.active_pet_id);
        return format!(
            r#"<p class="shared-care-banner" role="status">Calendar shows <strong>{pet}</strong>'s feeding schedule, vet reminders, and shared events from {owner}.</p>"#,
            pet = escape_html(&pet_name),
            owner = escape_html(&user_label(state, owner)),
        );
    }
    format!(
        r#"<p class="shared-care-banner" role="status">Your calendar also includes care schedules and vet reminders for <strong>{}</strong> shared cat{}.</p>"#,
        accepted.len(),
        if accepted.len() == 1 { "" } else { "s" }
    )
}

pub fn render_account_friends_section(
    state: &AppState,
    viewer_email: &str,
    owned_pets: &[(String, String)],
) -> String {
    format!(
        "{}{}",
        render_friends_card(state, viewer_email),
        render_pet_sharing_card(state, viewer_email, owned_pets),
    )
}

fn render_friends_card(state: &AppState, viewer_email: &str) -> String {
    let friends = state
        .storage
        .list_friends(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_summary)
        .collect::<Vec<_>>();
    let incoming = state
        .storage
        .list_incoming_friend_requests(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_request)
        .collect::<Vec<_>>();
    let outgoing = state
        .storage
        .list_outgoing_friend_requests(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_request)
        .collect::<Vec<_>>();
    let outgoing_friend_html: String = outgoing
        .iter()
        .map(|req| {
            format!(
                r#"<li class="friend-request-item friend-request-outgoing"><span>Waiting on <strong>{to}</strong></span><span class="field-hint">Request sent</span></li>"#,
                to = escape_html(&user_label(state, &req.to_email)),
            )
        })
        .collect();

    let incoming_friend_html: String = incoming
        .iter()
        .map(|req| {
            format!(
                r#"<li class="friend-request-item"><span>{from}</span>
  <form action="/home/friends/respond" method="post" class="inline-action-form">
    <input type="hidden" name="request_id" value="{id}" />
    <input type="hidden" name="action" value="accept" />
    <button type="submit" class="download-btn">Accept</button>
  </form>
  <form action="/home/friends/respond" method="post" class="inline-action-form">
    <input type="hidden" name="request_id" value="{id}" />
    <input type="hidden" name="action" value="decline" />
    <button type="submit" class="onboarding-skip-btn">Decline</button>
  </form></li>"#,
                from = escape_html(&user_label(state, &req.from_email)),
                id = escape_html_attr(&req.id),
            )
        })
        .collect();

    let friends_list_html = if friends.is_empty() {
        r#"<p class="field-hint">No friends yet — open Add friends above to connect by email or username.</p>"#.to_string()
    } else {
        format!(
            "<ul class=\"friend-list\">{}</ul>",
            friends
                .iter()
                .map(|friend| {
                    format!(
                        r#"<li class="friend-list-item"><strong>{}</strong> <span class="field-hint">{}</span></li>"#,
                        escape_html(&user_label(state, &friend.friend_email)),
                        escape_html(&friend.friend_email),
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    format!(
        r#"<article class="dashboard-card friends-sharing-card">
  <details class="friends-add-card">
    <summary class="friends-add-summary">
      <span class="friends-add-summary-text">Add friends</span>
    </summary>
    <div class="friends-add-body">
      <p class="field-hint">Connect with another WhiskerWatch parent first — then you can share specific cats, tasks, and schedules from the card below.</p>
      <form class="login-form add-friend-form" action="/home/friends/request" method="post">
        <label for="friend_email">Friend's email or username</label>
        <input id="friend_email" name="friend_email" type="text" required autocomplete="off" placeholder="friend@example.com or whiskerparent42" />
        <button type="submit" class="download-btn login-submit">Send friend request 💌</button>
      </form>
    </div>
  </details>
  {incoming_friends}
  {outgoing_friends}
  <h3 class="friends-subhead">Your friends</h3>
  {friends_list}
</article>"#,
        incoming_friends = if incoming_friend_html.is_empty() {
            String::new()
        } else {
            format!(
                "<h3 class=\"friends-subhead\">Friend requests for you</h3><ul class=\"friend-request-list\">{incoming_friend_html}</ul>"
            )
        },
        outgoing_friends = if outgoing_friend_html.is_empty() {
            String::new()
        } else {
            format!(
                "<h3 class=\"friends-subhead\">Requests you sent</h3><ul class=\"friend-request-list\">{outgoing_friend_html}</ul>"
            )
        },
        friends_list = friends_list_html,
    )
}

fn render_pet_sharing_card(
    state: &AppState,
    viewer_email: &str,
    owned_pets: &[(String, String)],
) -> String {
    let friends = state
        .storage
        .list_friends(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_summary)
        .collect::<Vec<_>>();
    let incoming_shares = state
        .storage
        .list_incoming_pet_shares(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_pet_share)
        .collect::<Vec<_>>();
    let accepted_shared = state
        .storage
        .list_accepted_pet_shares_for_recipient(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_pet_share)
        .collect::<Vec<_>>();
    let outgoing_shares = state
        .storage
        .list_outgoing_pet_shares(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_pet_share)
        .collect::<Vec<_>>();

    let pet_options = render_pet_share_options(owned_pets);

    let friend_options: String = if friends.is_empty() {
        r#"<option value="">Add a friend first</option>"#.to_string()
    } else {
        friends
            .iter()
            .map(|friend| {
                format!(
                    r#"<option value="{}">{}</option>"#,
                    escape_html_attr(&friend.friend_email),
                    escape_html(&user_label(state, &friend.friend_email)),
                )
            })
            .collect()
    };

    let per_friend_share_html = if owned_pets.is_empty() || friends.is_empty() {
        String::new()
    } else {
        format!(
            "<ul class=\"friend-share-list\">{}</ul>",
            friends
                .iter()
                .map(|friend| {
                    format!(
                        r#"<li class="friend-share-item">
  <span class="friend-share-label">Share with <strong>{label}</strong></span>
  <form class="login-form friend-quick-share-form" action="/home/pets/share" method="post">
    <input type="hidden" name="friend_email" value="{email}" />
    <label class="visually-hidden" for="share_pet_{email_id}">Cat to share with {label}</label>
    <select id="share_pet_{email_id}" name="pet_id" required>{pet_options}</select>
    <button type="submit" class="download-btn">Share tasks &amp; schedule</button>
  </form>
</li>"#,
                        label = escape_html(&user_label(state, &friend.friend_email)),
                        email = escape_html_attr(&friend.friend_email),
                        email_id = escape_html_attr(&friend.friend_email.replace('@', "_at_")),
                        pet_options = pet_options,
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    let share_form = if owned_pets.is_empty() {
        r#"<p class="field-hint">Set up a cat on the My Pet tab before sharing care schedules.</p>"#.to_string()
    } else if friends.is_empty() {
        r#"<p class="field-hint">Add a friend above to share a specific cat's tasks, feeding schedule, and calendar.</p>"#.to_string()
    } else {
        format!(
            r#"<form class="login-form share-cat-form" action="/home/pets/share" method="post">
  <label for="share_friend_email">Share with</label>
  <select id="share_friend_email" name="friend_email" required>{friend_options}</select>
  <label for="share_pet_id">Which cat</label>
  <select id="share_pet_id" name="pet_id" required>{pet_options}</select>
  <p class="field-hint">Your friend will see this cat's care tasks, feeding times, vet reminders, health records, and calendar events. They can complete tasks on your behalf.</p>
  <button type="submit" class="download-btn login-submit">Share cat care 🐾</button>
</form>
{per_friend_share}"#,
            friend_options = friend_options,
            pet_options = pet_options,
            per_friend_share = if per_friend_share_html.is_empty() {
                String::new()
            } else {
                format!(
                    "<h3 class=\"friends-subhead\">Quick share with a friend</h3>{per_friend_share_html}"
                )
            },
        )
    };

    let incoming_share_html: String = incoming_shares
        .iter()
        .map(|share| {
            let cat_name = pet_name_for_owner(state, &share.owner_email, &share.pet_id);
            format!(
                r#"<li class="share-request-item">
  <div class="share-request-copy">
    <strong>{cat}</strong> from {owner}
    <span class="field-hint">Includes care tasks, feeding schedule, and calendar reminders</span>
  </div>
  <form action="/home/pets/share/respond" method="post" class="inline-action-form">
    <input type="hidden" name="share_id" value="{id}" />
    <input type="hidden" name="action" value="accept" />
    <button type="submit" class="download-btn">Accept care access</button>
  </form>
  <form action="/home/pets/share/respond" method="post" class="inline-action-form">
    <input type="hidden" name="share_id" value="{id}" />
    <input type="hidden" name="action" value="decline" />
    <button type="submit" class="onboarding-skip-btn">Decline</button>
  </form>
</li>"#,
                cat = escape_html(&cat_name),
                owner = escape_html(&user_label(state, &share.owner_email)),
                id = escape_html_attr(&share.id),
            )
        })
        .collect();

    let shared_with_me_html = if accepted_shared.is_empty() {
        r#"<p class="field-hint">No cats shared with you yet. When a friend shares, you'll see their tasks and schedule here.</p>"#.to_string()
    } else {
        format!(
            "<ul class=\"shared-pet-list\">{}</ul>",
            accepted_shared
                .iter()
                .map(|share| {
                    let cat_name = pet_name_for_owner(state, &share.owner_email, &share.pet_id);
                    let owner_label = user_label(state, &share.owner_email);
                    format!(
                        r#"<li class="shared-pet-list-item">
  <a href="/home?tab=pet&amp;pet={pet_id}&amp;pet_owner={owner}"><strong>{cat}</strong> · shared by {owner_label}</a>
  <span class="shared-pet-links">
    <a href="/home?tab=tasks&amp;pet={pet_id}&amp;pet_owner={owner}">Tasks</a>
    <a href="/home?tab=calendar&amp;pet={pet_id}&amp;pet_owner={owner}">Calendar</a>
    <a href="/home?tab=health&amp;pet={pet_id}&amp;pet_owner={owner}">Health</a>
  </span>
</li>"#,
                        pet_id = escape_html_attr(&share.pet_id),
                        owner = escape_html_attr(&share.owner_email),
                        cat = escape_html(&cat_name),
                        owner_label = escape_html(&owner_label),
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    let outgoing_share_html = if outgoing_shares.is_empty() {
        r#"<p class="field-hint">You have not shared any cats yet.</p>"#.to_string()
    } else {
        format!(
            "<ul class=\"outgoing-share-list\">{}</ul>",
            outgoing_shares
                .iter()
                .map(|share| {
                    let cat_name = pet_name_for_owner(state, viewer_email, &share.pet_id);
                    let status_label = if share.status == SHARE_STATUS_PENDING {
                        "Invite pending"
                    } else {
                        "Sharing tasks & schedule"
                    };
                    format!(
                        r#"<li class="outgoing-share-item">
  <div class="outgoing-share-copy">
    <strong>{cat}</strong> with {friend}
    <span class="field-hint">{status}</span>
  </div>
  <form action="/home/pets/share/revoke" method="post" class="inline-action-form">
    <input type="hidden" name="share_id" value="{id}" />
    <button type="submit" class="onboarding-skip-btn">Stop sharing</button>
  </form>
</li>"#,
                        cat = escape_html(&cat_name),
                        friend = escape_html(&user_label(state, &share.shared_with_email)),
                        status = escape_html(status_label),
                        id = escape_html_attr(&share.id),
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    format!(
        r#"<article class="dashboard-card pet-sharing-card">
  <h2>Share pets, tasks &amp; schedules</h2>
  <p class="field-hint">Pick a specific cat to share its care tasks, feeding times, vet reminders, and calendar with a friend — perfect for co-parents, sitters, and family helpers.</p>
  {share_form}
  <h3 class="friends-subhead">Cats you're sharing</h3>
  {outgoing_shares}
  <h3 class="friends-subhead">Cats shared with you</h3>
  {shared_with_me}
  {incoming_shares}
</article>"#,
        share_form = share_form,
        outgoing_shares = outgoing_share_html,
        shared_with_me = shared_with_me_html,
        incoming_shares = if incoming_share_html.is_empty() {
            String::new()
        } else {
            format!(
                "<h3 class=\"friends-subhead\">Care share invites for you</h3><ul class=\"share-request-list\">{incoming_share_html}</ul>"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::User;

    fn test_state_with_users() -> AppState {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = crate::storage::Storage::open_at(temp.path().to_path_buf()).expect("storage");
        let state = AppState { storage };
        state
            .storage
            .save_user(&User {
                username: "mochi_mom".to_string(),
                first_name: "Mochi".to_string(),
                last_name: "Mom".to_string(),
                email: "mom@example.com".to_string(),
                password: "secret1!".to_string(),
                created_at: 1,
            })
            .expect("save mom");
        state
            .storage
            .save_user(&User {
                username: "catdad".to_string(),
                first_name: "Cat".to_string(),
                last_name: "Dad".to_string(),
                email: "dad@example.com".to_string(),
                password: "secret2!".to_string(),
                created_at: 2,
            })
            .expect("save dad");
        state
    }

    #[test]
    fn resolve_friend_identifier_accepts_username_or_email() {
        let state = test_state_with_users();
        assert_eq!(
            resolve_friend_identifier(&state, "catdad").expect("username"),
            "dad@example.com"
        );
        assert_eq!(
            resolve_friend_identifier(&state, "DAD@example.com").expect("email"),
            "dad@example.com"
        );
        assert_eq!(
            resolve_friend_identifier(&state, "missing").err(),
            Some(FriendLookupError::NotFound)
        );
    }
}
