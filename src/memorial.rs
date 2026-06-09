use crate::{
    escape_html, escape_html_attr, list_pet_summaries, pet_snapshot, profile_has_pet,
    scheduled_task, user_needs_pet_setup, UserProfile, UserTask, PRIMARY_PET_ID,
};

const DAILY_CARE_TASK_IDS: [&str; 5] = [
    "water_bowl_morning",
    "water_bowl_night",
    "litter_check",
    "play_session",
    "replace_litter",
];
use chrono::Local;

pub const MAX_MEMORIAL_VIDEOS: usize = 10;

pub const MEMORIAL_SELF_HUG_TASK_ID: &str = "memorial_self_hug";
pub const MEMORIAL_CANDLE_TASK_ID: &str = "memorial_light_candle";
pub const MEMORIAL_PET_FOR_ANGEL_TASK_ID: &str = "memorial_pet_for_angel";

const MEMORIAL_TASK_IDS: [&str; 3] = [
    MEMORIAL_SELF_HUG_TASK_ID,
    MEMORIAL_CANDLE_TASK_ID,
    MEMORIAL_PET_FOR_ANGEL_TASK_ID,
];

pub fn is_memorial_task_id(task_id: &str) -> bool {
    MEMORIAL_TASK_IDS.contains(&task_id)
}

pub fn pet_is_deceased(profile: &UserProfile, pet_id: &str) -> bool {
    pet_snapshot(profile, pet_id)
        .map(|snapshot| snapshot.deceased)
        .unwrap_or(false)
}

pub fn active_pet_is_deceased(profile: &UserProfile) -> bool {
    pet_is_deceased(profile, &profile.active_pet_id)
}

fn first_living_pet_name(profile: &UserProfile, except_pet_id: &str) -> Option<String> {
    list_pet_summaries(profile)
        .into_iter()
        .filter(|(pet_id, _)| pet_id != except_pet_id)
        .find_map(|(pet_id, _)| {
            if pet_is_deceased(profile, &pet_id) {
                return None;
            }
            pet_snapshot(profile, &pet_id).map(|snapshot| snapshot.pet_name)
        })
}

pub fn memorialize_pet(profile: &mut UserProfile, pet_id: &str) -> bool {
    let today = Local::now().date_naive().to_string();
    if pet_id == PRIMARY_PET_ID {
        if !profile_has_pet(profile) || profile.deceased {
            return false;
        }
        profile.deceased = true;
        profile.deceased_at = Some(today);
        profile.memorial_comfort_seen = false;
        return true;
    }

    let Some(pet) = profile
        .additional_pets
        .iter_mut()
        .find(|pet| pet.id == pet_id)
    else {
        return false;
    };
    if pet.deceased {
        return false;
    }
    pet.deceased = true;
    pet.deceased_at = Some(today);
    pet.memorial_comfort_seen = false;
    true
}

pub fn dismiss_memorial_comfort(profile: &mut UserProfile, pet_id: &str) -> bool {
    if pet_id == PRIMARY_PET_ID {
        if !profile.deceased {
            return false;
        }
        profile.memorial_comfort_seen = true;
        return true;
    }
    let Some(pet) = profile
        .additional_pets
        .iter_mut()
        .find(|pet| pet.id == pet_id)
    else {
        return false;
    };
    if !pet.deceased {
        return false;
    }
    pet.memorial_comfort_seen = true;
    true
}

pub fn memorial_videos_for_pet(profile: &UserProfile, pet_id: &str) -> Vec<String> {
    if pet_id == PRIMARY_PET_ID {
        return profile.memorial_videos.clone();
    }
    profile
        .additional_pets
        .iter()
        .find(|pet| pet.id == pet_id)
        .map(|pet| pet.memorial_videos.clone())
        .unwrap_or_default()
}

pub fn memorial_comfort_seen(profile: &UserProfile, pet_id: &str) -> bool {
    if pet_id == PRIMARY_PET_ID {
        return profile.memorial_comfort_seen;
    }
    profile
        .additional_pets
        .iter()
        .find(|pet| pet.id == pet_id)
        .map(|pet| pet.memorial_comfort_seen)
        .unwrap_or(true)
}

pub fn set_memorial_video_slot(
    profile: &mut UserProfile,
    pet_id: &str,
    slot: usize,
    url: String,
) -> bool {
    if slot >= MAX_MEMORIAL_VIDEOS {
        return false;
    }
    if pet_id == PRIMARY_PET_ID {
        profile.memorial_videos.resize(MAX_MEMORIAL_VIDEOS, String::new());
        profile.memorial_videos[slot] = url;
        return true;
    }
    let Some(pet) = profile
        .additional_pets
        .iter_mut()
        .find(|pet| pet.id == pet_id)
    else {
        return false;
    };
    pet.memorial_videos.resize(MAX_MEMORIAL_VIDEOS, String::new());
    pet.memorial_videos[slot] = url;
    true
}

fn remove_daily_care_tasks_for_pet(profile: &mut UserProfile, pet_id: &str) -> bool {
    let before = profile.tasks.len();
    profile.tasks.retain(|task| {
        if task.pet_id != pet_id {
            return true;
        }
        if is_memorial_task_id(&task.id) {
            return true;
        }
        if crate::breed_guides::is_breed_guide_task_id(&task.id) {
            return false;
        }
        if task.id == crate::VET_APPOINTMENT_TASK_ID {
            return false;
        }
        if crate::FEEDING_TASK_IDS.contains(&task.id.as_str())
            || DAILY_CARE_TASK_IDS.contains(&task.id.as_str())
        {
            return false;
        }
        !task.id.starts_with("custom_")
    });
    profile.tasks.len() != before
}

pub fn ensure_memorial_tasks_for_pet(profile: &mut UserProfile, pet_id: &str) -> bool {
    if !pet_is_deceased(profile, pet_id) {
        return false;
    }

    let mut changed = remove_daily_care_tasks_for_pet(profile, pet_id);
    let Some(snapshot) = pet_snapshot(profile, pet_id) else {
        return changed;
    };
    let pet_name = snapshot.pet_name.clone();
    let today = Local::now().date_naive();

    let mut desired: Vec<UserTask> = vec![
        scheduled_task(
            MEMORIAL_SELF_HUG_TASK_ID,
            "Give yourself a gentle hug 💗",
            "Anytime today",
            12 * 60,
            8,
            today,
            pet_id,
        ),
        scheduled_task(
            MEMORIAL_CANDLE_TASK_ID,
            &format!("Light a candle for {}", pet_name),
            "Anytime today",
            19 * 60,
            8,
            today,
            pet_id,
        ),
    ];

    if let Some(living_name) = first_living_pet_name(profile, pet_id) {
        desired.push(scheduled_task(
            MEMORIAL_PET_FOR_ANGEL_TASK_ID,
            &format!("Pet {} and tell them about {}", living_name, pet_name),
            "Anytime today",
            15 * 60,
            10,
            today,
            pet_id,
        ));
    }

    let desired_ids: std::collections::HashSet<String> =
        desired.iter().map(|task| task.id.clone()).collect();

    let before_len = profile.tasks.len();
    profile.tasks.retain(|task| {
        task.pet_id != pet_id
            || !is_memorial_task_id(&task.id)
            || desired_ids.contains(&task.id)
    });
    if profile.tasks.len() != before_len {
        changed = true;
    }

    for task in desired {
        if profile.tasks.iter().any(|existing| {
            existing.id == task.id && existing.pet_id == pet_id
        }) {
            continue;
        }
        profile.tasks.push(task);
        changed = true;
    }

    changed
}

pub fn ensure_memorial_tasks(profile: &mut UserProfile) -> bool {
    let mut changed = false;
    for (pet_id, _) in list_pet_summaries(profile) {
        if ensure_memorial_tasks_for_pet(profile, &pet_id) {
            changed = true;
        }
    }
    changed
}

pub fn memorial_videos_json(profile: &UserProfile, pet_id: &str) -> String {
    let stored = memorial_videos_for_pet(profile, pet_id);
    let videos: Vec<&str> = stored
        .iter()
        .map(String::as_str)
        .filter(|url| !url.trim().is_empty())
        .collect();
    serde_json::to_string(&videos).unwrap_or_else(|_| "[]".to_string())
}

fn memorial_photo_src(profile: &UserProfile) -> String {
    profile
        .pet_photo_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "/cinderanimate.png".to_string())
}

pub fn render_account_pet_photo(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile) {
        return r#"<p class="account-pet-photo-empty">Add your cat on the My Pet tab to upload a profile photo and playing video clip.</p>"#
            .to_string();
    }

    if !active_pet_is_deceased(profile) {
        return crate::render_account_pet_photo_living(profile);
    }

    let pet_name = escape_html(&profile.pet_name);
    let photo_src = escape_html_attr(&memorial_photo_src(profile));
    let videos_json = escape_html_attr(&memorial_videos_json(profile, &profile.active_pet_id));
    let video_count = memorial_videos_for_pet(profile, &profile.active_pet_id)
        .iter()
        .filter(|url| !url.trim().is_empty())
        .count();

    format!(
        r#"<div class="account-pet-photo account-pet-photo-memorial" id="memorial-photo-stage" data-pet-name="{pet_name}" data-memorial-videos="{videos_json}">
  <div class="memorial-photo-halo" aria-hidden="true">👼</div>
  <button type="button" class="memorial-photo-cycle" id="memorial-photo-cycle" aria-label="Cycle through memory clips of {pet_name}">
    <img class="memorial-photo-image account-pet-photo-image" src="{photo_src}" alt="{pet_name} memorial photo" />
    <video class="memorial-photo-video" muted playsinline webkit-playsinline preload="metadata" hidden></video>
  </button>
  <p class="account-pet-photo-caption memorial-photo-caption">{pet_name} · angel cat · tap to cycle memories ({video_count}/10 clips)</p>
</div>"#,
        pet_name = pet_name,
        photo_src = photo_src,
        videos_json = videos_json,
        video_count = video_count,
    )
}

pub fn render_memorial_video_uploads(profile: &UserProfile) -> String {
    if !active_pet_is_deceased(profile) {
        return String::new();
    }

    let pet_id = escape_html_attr(&profile.active_pet_id);
    let videos = memorial_videos_for_pet(profile, &profile.active_pet_id);
    let mut slots = String::new();
    for slot in 0..MAX_MEMORIAL_VIDEOS {
        let filled = videos
            .get(slot)
            .is_some_and(|url| !url.trim().is_empty());
        let status = if filled {
            format!("Clip {} saved", slot + 1)
        } else {
            format!("Clip {} empty", slot + 1)
        };
        let input_id = format!("memorial-video-{slot}");
        slots.push_str(&format!(
            r#"<div class="memorial-video-slot">
  <form class="memorial-video-slot-form login-form" action="/home/pets/memorial-video" method="post" enctype="multipart/form-data">
    <input type="hidden" name="pet_id" value="{pet_id}" />
    <input type="hidden" name="slot" value="{slot}" />
    <label for="{input_id}" class="memorial-video-slot-label">{status}</label>
    <input id="{input_id}" name="memorial_video" type="file" accept="video/mp4,video/webm,video/quicktime,.mp4,.webm,.mov" />
    <button type="submit" class="download-btn memorial-video-upload-btn">Save clip</button>
  </form>
</div>"#,
            pet_id = pet_id,
            slot = slot,
            input_id = input_id,
            status = status,
        ));
    }

    format!(
        r#"<article class="dashboard-card memorial-videos-card">
  <h2>Memory clips for {pet_name}</h2>
  <p class="field-hint">Add up to 10 short videos. Tap their profile photo on this page to cycle through them on repeat.</p>
  <div class="memorial-video-grid">{slots}</div>
</article>"#,
        pet_name = escape_html(&profile.pet_name),
        slots = slots,
    )
}

pub fn render_mark_memorial_card(profile: &UserProfile) -> String {
    if user_needs_pet_setup(profile)
        || active_pet_is_deceased(profile)
        || profile.active_pet_owner_email.is_some()
    {
        return String::new();
    }

    let pet_name = escape_html(&profile.pet_name);
    let pet_id = escape_html_attr(&profile.active_pet_id);
    format!(
        r#"<article class="dashboard-card memorial-mark-card">
  <h2>Difficult news</h2>
  <p class="field-hint">If {pet_name} has crossed the rainbow bridge, we can gently update their profile into a calming memorial space.</p>
  <form class="memorial-mark-form" action="/home/pets/memorialize" method="post" onsubmit="return confirm('We are so sorry. Create a memorial space for {pet_name}?');">
    <input type="hidden" name="pet_id" value="{pet_id}" />
    <button type="submit" class="memorial-mark-btn">My cat has passed away</button>
  </form>
</article>"#,
        pet_name = pet_name,
        pet_id = pet_id,
    )
}

pub fn render_memorial_comfort_modal(profile: &UserProfile) -> String {
    if !active_pet_is_deceased(profile) || memorial_comfort_seen(profile, &profile.active_pet_id) {
        return String::new();
    }

    let pet_name = escape_html(&profile.pet_name);
    let pet_id = escape_html_attr(&profile.active_pet_id);
    format!(
        r#"<div class="memorial-comfort-backdrop" id="memorial-comfort-modal" role="dialog" aria-modal="true" aria-labelledby="memorial-comfort-title">
  <div class="memorial-comfort-modal">
    <p class="memorial-comfort-emoji" aria-hidden="true">🪽</p>
    <h2 id="memorial-comfort-title">You loved {pet_name} so well</h2>
    <p>Grief is love with nowhere to go — and what you shared was real. It is okay to feel wobbly, quiet, or all over the place.</p>
    <p>WhiskerWatch will keep {pet_name} close as your angel cat. Take your time. You are not alone, and it will be okay.</p>
    <form action="/home/pets/memorial-comfort" method="post">
      <input type="hidden" name="pet_id" value="{pet_id}" />
      <button type="submit" class="download-btn login-submit">Thank you — I am ready</button>
    </form>
  </div>
</div>"#,
        pet_name = pet_name,
        pet_id = pet_id,
    )
}

pub fn render_angel_pet_avatar(profile: &UserProfile) -> String {
    let snapshot = crate::active_pet_snapshot(profile);
    let pet_name_raw = snapshot
        .as_ref()
        .map(|pet| pet.pet_name.as_str())
        .unwrap_or(profile.pet_name.as_str());
    let pet_name = escape_html(pet_name_raw);
    let photo_src = snapshot
        .as_ref()
        .and_then(|pet| pet.pet_photo_url.as_deref())
        .filter(|value| !value.is_empty())
        .map(escape_html_attr)
        .unwrap_or_else(|| "/cinderanimate.png".to_string());

    format!(
        r#"<div class="pet-cinder-stage pet-cinder-stage-angel" id="cinder-pet-stage" data-pet-name="{pet_name}">
  <p class="cinder-pet-label cinder-pet-label-angel">{pet_name} 👼</p>
  <div class="cinder-pet-image-wrap cinder-pet-image-wrap-angel">
    <div class="angel-halo" aria-hidden="true"></div>
    <img class="cinder-pet-image cinder-pet-image-angel" src="{photo_src}" alt="{pet_name} angel cat" />
  </div>
</div>"#,
        pet_name = pet_name,
        photo_src = photo_src,
    )
}

pub fn render_angel_cat_home_scene(profile: &UserProfile) -> String {
    let pet_name = if profile.pet_name.trim().is_empty() {
        "Your cat".to_string()
    } else {
        profile.pet_name.clone()
    };
    let pet_avatar = render_angel_pet_avatar(profile);

    format!(
        r#"<div class="cat-home-scene cat-home-scene-angel" data-room="angel">
  <div class="cat-home-room-bg cat-home-room-bg-angel" aria-hidden="true"></div>
  <div class="cat-home-pet-stage">
    <div class="cat-home-pet-stack">
      <p class="cat-home-pet-bubble cat-home-pet-bubble-angel" role="note">watching over you 🪽</p>
      {pet_avatar}
    </div>
  </div>
  <p class="cat-home-mood cat-home-mood-angel">{pet_name} is your angel cat now, still curled up in their home among the stars.</p>
</div>"#,
        pet_avatar = pet_avatar,
        pet_name = escape_html(&pet_name),
    )
}

pub fn pet_switcher_angel_suffix(profile: &UserProfile, pet_id: &str, owner_email: &str) -> String {
    if owner_email != profile.email {
        return String::new();
    }
    if pet_is_deceased(profile, pet_id) {
        " 👼".to_string()
    } else {
        String::new()
    }
}

pub fn push_memorial_activity(profile: &mut UserProfile, pet_name: &str) {
    crate::push_activity(
        profile,
        &format!("Created a memorial space for {pet_name}. They will stay as your angel cat."),
    );
}

pub fn memorial_status_message(status: Option<&str>, pet_name: &str) -> Option<String> {
    match status {
        Some("memorial_started") => Some(format!(
            "We are holding space for {}. Their memorial profile is ready on the Account tab.",
            pet_name
        )),
        Some("memorial_video_saved") => Some(format!(
            "Memory clip saved for {}. Tap their photo to cycle through clips.",
            pet_name
        )),
        Some("memorial_video_invalid") => {
            Some("That memory clip could not be saved. Try MP4, WebM, or MOV under 50MB.".to_string())
        }
        Some("memorial_invalid") => Some("That memorial update could not be saved.".to_string()),
        _ => None,
    }
}
