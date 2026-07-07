use crate::playdates::SceneCat;
use crate::{escape_html, pet_snapshot, sharing, AppState, PetSnapshot, UserProfile};
use chrono::{Datelike, NaiveDate};
use std::collections::HashSet;

pub const NPC_PARTY_OWNER: &str = "__npc_party__";

const NPC_GUESTS: &[(&str, &str, &str)] = &[
    ("npc-party-biscuit", "Biscuit", "Ginger tabby"),
    ("npc-party-mochi", "Mochi", "Calico"),
    ("npc-party-whiskers", "Whiskers", "Gray tuxedo"),
    ("npc-party-luna", "Luna", "Black cat"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BirthdayPet {
    pub pet_id: String,
    pub pet_name: String,
}

#[derive(Debug, Clone)]
pub struct BirthdayPartyContext {
    pub birthday_pets: Vec<BirthdayPet>,
    pub invited_friend_count: usize,
    pub npc_guest_count: usize,
}

pub fn is_pet_birthday_today(snapshot: &PetSnapshot, today: NaiveDate) -> bool {
    let Some(birth) = crate::pet_birth_date_for_snapshot(snapshot, today) else {
        return false;
    };
    birth.month() == today.month() && birth.day() == today.day()
}

pub fn birthday_pets_today(profile: &UserProfile, today: NaiveDate) -> Vec<BirthdayPet> {
    let mut pets = Vec::new();
    for (pet_id, _) in sharing::pet_summaries_for_profile(profile) {
        let Some(snapshot) = pet_snapshot(profile, &pet_id) else {
            continue;
        };
        if snapshot.deceased {
            continue;
        }
        if is_pet_birthday_today(&snapshot, today) {
            pets.push(BirthdayPet {
                pet_id,
                pet_name: snapshot.pet_name,
            });
        }
    }
    pets
}

pub fn party_context(
    state: &AppState,
    profile: &UserProfile,
    today: NaiveDate,
) -> Option<BirthdayPartyContext> {
    let birthday_pets = birthday_pets_today(profile, today);
    if birthday_pets.is_empty() {
        return None;
    }
    let friends = sharing::accepted_friend_emails(state, &profile.email);
    let npc_guest_count = if friends.is_empty() {
        NPC_GUESTS.len()
    } else {
        0
    };
    Some(BirthdayPartyContext {
        birthday_pets,
        invited_friend_count: friends.len(),
        npc_guest_count,
    })
}

pub fn is_npc_party_cat(owner_email: &str, _pet_id: &str) -> bool {
    owner_email == NPC_PARTY_OWNER
}

pub fn npc_display_name(pet_id: &str) -> Option<&'static str> {
    NPC_GUESTS
        .iter()
        .find(|(id, _, _)| *id == pet_id)
        .map(|(_, name, _)| *name)
}

pub fn party_active_for_viewer(state: &AppState, viewer: &UserProfile, today: NaiveDate) -> bool {
    party_context(state, viewer, today).is_some()
}

pub fn list_party_scene_cats(
    state: &AppState,
    viewer: &UserProfile,
    today: NaiveDate,
) -> Option<Vec<SceneCat>> {
    let party = party_context(state, viewer, today)?;
    let birthday_ids: HashSet<String> = party
        .birthday_pets
        .iter()
        .map(|pet| pet.pet_id.clone())
        .collect();

    let mut cats = Vec::new();
    let mut seen = HashSet::new();

    for (pet_id, _) in sharing::pet_summaries_for_profile(viewer) {
        let Some(snapshot) = pet_snapshot(viewer, &pet_id) else {
            continue;
        };
        if snapshot.deceased {
            continue;
        }
        let dedupe = format!("{}|{}", sharing::normalize_email(&viewer.email), pet_id);
        if !seen.insert(dedupe) {
            continue;
        }
        let is_birthday = birthday_ids.contains(&pet_id);
        cats.push(SceneCat {
            pet_id,
            owner_email: viewer.email.clone(),
            pet_name: snapshot.pet_name,
            pet_color: snapshot.pet_color,
            pet_photo_url: snapshot.pet_photo_url,
            is_owned: true,
            owner_label: "You".to_string(),
            is_npc: false,
            is_birthday_cat: is_birthday,
        });
    }

    let friends = sharing::accepted_friend_emails(state, &viewer.email);
    for friend_email in friends {
        let Some(friend_profile) = sharing::load_profile_by_email(state, &friend_email) else {
            continue;
        };
        let owner_label = sharing::user_label(state, &friend_email);
        for (pet_id, _) in sharing::pet_summaries_for_profile(&friend_profile) {
            let Some(snapshot) = pet_snapshot(&friend_profile, &pet_id) else {
                continue;
            };
            if snapshot.deceased {
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

    if party.npc_guest_count > 0 {
        let reserved_names: HashSet<String> = cats
            .iter()
            .map(|cat| cat.pet_name.trim().to_ascii_lowercase())
            .collect();
        for (pet_id, name, color) in NPC_GUESTS.iter().take(party.npc_guest_count) {
            if reserved_names.contains(&name.to_ascii_lowercase()) {
                continue;
            }
            cats.push(SceneCat {
                pet_id: (*pet_id).to_string(),
                owner_email: NPC_PARTY_OWNER.to_string(),
                pet_name: (*name).to_string(),
                pet_color: (*color).to_string(),
                pet_photo_url: None,
                is_owned: false,
                owner_label: "Party guest".to_string(),
                is_npc: true,
                is_birthday_cat: false,
            });
        }
    }

    Some(cats)
}

fn birthday_names_line(pets: &[BirthdayPet]) -> String {
    let names: Vec<String> = pets
        .iter()
        .map(|pet| {
            let name = pet.pet_name.trim();
            if name.is_empty() {
                "Your cat".to_string()
            } else {
                name.to_string()
            }
        })
        .collect();
    match names.as_slice() {
        [] => "Your cat".to_string(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let last = names.last().cloned().unwrap_or_default();
            let head = names[..names.len() - 1].join(", ");
            format!("{head}, and {last}")
        }
    }
}

fn invite_summary(ctx: &BirthdayPartyContext) -> String {
    if ctx.invited_friend_count > 0 {
        let friend_word = if ctx.invited_friend_count == 1 {
            "friend"
        } else {
            "friends"
        };
        format!(
            "We invited all {} of your {} — their cats are at the party in Cat Home!",
            ctx.invited_friend_count, friend_word
        )
    } else {
        format!(
            "You do not have friends on WhiskerWatch yet, so {} neighborhood cats came to celebrate!",
            ctx.npc_guest_count
        )
    }
}

pub fn render_dashboard_banner(ctx: &BirthdayPartyContext) -> String {
    let names = birthday_names_line(&ctx.birthday_pets);
    let invite = invite_summary(ctx);
    format!(
        r#"<div class="birthday-party-banner birthday-party-banner--dashboard" role="status">
  <p class="birthday-party-banner-kicker">🎂 Virtual birthday party</p>
  <p class="birthday-party-banner-title">Happy birthday, {names}!</p>
  <p class="birthday-party-banner-copy">{invite}</p>
  <a class="download-btn birthday-party-banner-cta" href="/home/cat-home">Join the party</a>
</div>"#,
        names = escape_html(&names),
        invite = escape_html(&invite),
    )
}

pub fn render_cat_home_banner(ctx: &BirthdayPartyContext) -> String {
    let names = birthday_names_line(&ctx.birthday_pets);
    let invite = invite_summary(ctx);
    format!(
        r#"<div class="birthday-party-banner birthday-party-banner--scene" role="status">
  <p class="birthday-party-banner-kicker">🎈 Birthday party today</p>
  <p class="birthday-party-banner-title">{names} is turning another year more adorable!</p>
  <p class="birthday-party-banner-copy">{invite}</p>
</div>"#,
        names = escape_html(&names),
        invite = escape_html(&invite),
    )
}

pub fn party_mood_message(ctx: &BirthdayPartyContext, cats: &[SceneCat]) -> String {
    let names = birthday_names_line(&ctx.birthday_pets);
    let guest_count = cats.iter().filter(|cat| !cat.is_owned).count();
    if guest_count == 0 {
        return format!(
            "Happy birthday, {names}! The party is ready — more guests may arrive as friends join WhiskerWatch."
        );
    }
    format!(
        "Happy birthday, {names}! {guest_count} party guests are playing in the family cat home right now.",
        guest_count = guest_count,
    )
}

pub fn cat_home_title(ctx: &BirthdayPartyContext) -> String {
    let names = birthday_names_line(&ctx.birthday_pets);
    if ctx.birthday_pets.len() == 1 {
        format!("{names}'s Birthday Party")
    } else {
        format!("{names}' Birthday Party")
    }
}

pub fn cat_home_intro(ctx: &BirthdayPartyContext) -> String {
    if ctx.invited_friend_count > 0 {
        "Your friends were invited — their cats joined the celebration. Tap guests and decor for birthday playdates!".to_string()
    } else {
        "Neighborhood cats heard about the party and came to play. Tap guests and decor for birthday playdates!".to_string()
    }
}

pub fn render_party_overlay() -> String {
    r#"<div class="cat-home-birthday-overlay" aria-hidden="true">
  <span class="cat-home-birthday-balloon cat-home-birthday-balloon--1">🎈</span>
  <span class="cat-home-birthday-balloon cat-home-birthday-balloon--2">🎉</span>
  <span class="cat-home-birthday-balloon cat-home-birthday-balloon--3">🎂</span>
  <span class="cat-home-birthday-confetti cat-home-birthday-confetti--1"></span>
  <span class="cat-home-birthday-confetti cat-home-birthday-confetti--2"></span>
  <span class="cat-home-birthday-confetti cat-home-birthday-confetti--3"></span>
</div>"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use crate::{default_profile, AppState, PRIMARY_PET_ID};
    use chrono::Local;
    use uuid::Uuid;

    fn test_state() -> AppState {
        let storage =
            Storage::open_at(std::env::temp_dir().join(format!("ww-birthday-{}", Uuid::new_v4())))
                .expect("storage");
        AppState { storage }
    }

    fn profile_with_birthday_today(name: &str) -> (UserProfile, NaiveDate) {
        let today = Local::now().date_naive();
        let mut profile = default_profile("party@example.com");
        profile.pet_name = name.to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_color = "Tabby".to_string();
        profile.pet_birth_date = Some(today.format("%Y-%m-%d").to_string());
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        profile.pet_photo_url = Some("/uploads/cat.png".to_string());
        profile.onboarding_completed = true;
        (profile, today)
    }

    #[test]
    fn birthday_pets_detected_for_today() {
        let (profile, today) = profile_with_birthday_today("Mochi");
        let pets = birthday_pets_today(&profile, today);
        assert_eq!(pets.len(), 1);
        assert_eq!(pets[0].pet_id, PRIMARY_PET_ID);
        assert_eq!(pets[0].pet_name, "Mochi");
    }

    #[test]
    fn party_context_adds_npc_guests_without_friends() {
        let state = test_state();
        let (profile, today) = profile_with_birthday_today("Mochi");
        let ctx = party_context(&state, &profile, today).expect("party");
        assert_eq!(ctx.invited_friend_count, 0);
        assert_eq!(ctx.npc_guest_count, NPC_GUESTS.len());
        let cats = list_party_scene_cats(&state, &profile, today).expect("cats");
        assert!(cats.iter().any(|cat| cat.is_npc));
        assert!(cats.iter().any(|cat| cat.is_owned && cat.is_birthday_cat));
    }

    #[test]
    fn dashboard_banner_links_to_cat_home() {
        let state = test_state();
        let (profile, today) = profile_with_birthday_today("Mochi");
        let ctx = party_context(&state, &profile, today).expect("party");
        let html = render_dashboard_banner(&ctx);
        assert!(html.contains("/home/cat-home"));
        assert!(html.contains("Happy birthday"));
    }
}
