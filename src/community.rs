use crate::breeds;
use crate::list_pet_summaries;
use crate::pet_snapshot;
use crate::profile_has_pet;
use crate::user_for_email;
use crate::AppState;
use crate::UserProfile;

pub const MAX_CAT_FEED: usize = 48;
const PET_VIDEO_CLIP_MIN_SECONDS: f32 = 3.0;
const PET_VIDEO_CLIP_MAX_SECONDS: f32 = 6.0;

#[derive(Clone)]
pub struct PublicCatCard {
    pub author_username: String,
    pub pet_name: String,
    pub pet_breed: String,
    pub pet_color: String,
    pub parent_level: u32,
    pub care_streak_days: u32,
    pub pet_photo_url: Option<String>,
    pub pet_video_url: Option<String>,
    pub pet_video_clip_start: f32,
    pub pet_video_clip_duration: f32,
    pub equipped_outfit: String,
    pub is_viewer: bool,
    #[allow(dead_code)]
    pub breed_slug: String,
}

fn clip_start(value: Option<f32>) -> f32 {
    value.unwrap_or(0.0).max(0.0)
}

fn clip_duration(value: Option<f32>) -> f32 {
    value
        .unwrap_or(PET_VIDEO_CLIP_MAX_SECONDS)
        .clamp(PET_VIDEO_CLIP_MIN_SECONDS, PET_VIDEO_CLIP_MAX_SECONDS)
}

pub fn breed_slug_for_name(name: &str) -> String {
    breeds::breed_slug(name)
}

pub fn breed_label_for_slug(slug: &str) -> String {
    let normalized = slug.trim().to_lowercase();
    if normalized.is_empty() {
        return "All breeds".to_string();
    }

    for category in breeds::CATALOG {
        for breed in category.breeds {
            if breeds::breed_slug(breed.name) == normalized {
                return breed.name.to_string();
            }
        }
    }

    normalized
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn collect_public_cat_cards(
    state: &AppState,
    viewer_email: &str,
    breed_filter: Option<&str>,
) -> Vec<PublicCatCard> {
    let emails = state.storage.list_profile_emails().unwrap_or_default();
    let filter_slug = breed_filter
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);

    let mut cards = Vec::new();

    for email in emails {
        let Ok(Some(profile)) = state.storage.load_profile(&email) else {
            continue;
        };

        if !profile.community_visible || !profile_has_pet(&profile) {
            continue;
        }

        let is_viewer = email.eq_ignore_ascii_case(viewer_email);
        let author_username = user_for_email(state, &email)
            .map(|user| user.username)
            .unwrap_or_else(|| "Cat parent".to_string());

        for (pet_id, _) in list_pet_summaries(&profile) {
            let Some(snapshot) = pet_snapshot(&profile, &pet_id) else {
                continue;
            };

            let breed_slug = breed_slug_for_name(&snapshot.pet_breed);
            if let Some(ref slug) = filter_slug {
                if &breed_slug != slug {
                    continue;
                }
            }

            cards.push(PublicCatCard {
                author_username: author_username.clone(),
                pet_name: snapshot.pet_name.clone(),
                pet_breed: snapshot.pet_breed.clone(),
                pet_color: snapshot.pet_color.clone(),
                parent_level: profile.parent_level,
                care_streak_days: profile.care_streak_days,
                pet_photo_url: snapshot.pet_photo_url.clone(),
                pet_video_url: snapshot.pet_video_url.clone(),
                pet_video_clip_start: clip_start(snapshot.pet_video_clip_start),
                pet_video_clip_duration: clip_duration(snapshot.pet_video_clip_duration),
                equipped_outfit: profile.equipped_outfit.clone(),
                is_viewer,
                breed_slug,
            });
        }
    }

    cards.sort_by(|left, right| {
        match (left.is_viewer, right.is_viewer) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => right
                .parent_level
                .cmp(&left.parent_level)
                .then(right.care_streak_days.cmp(&left.care_streak_days))
                .then(left.pet_name.cmp(&right.pet_name)),
        }
    });
    cards.truncate(MAX_CAT_FEED);
    cards
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html(value)
}

pub fn render_cat_feed_card(card: &PublicCatCard) -> String {
    let pet_name = escape_html(&card.pet_name);
    let photo = if let Some(url) = card
        .pet_photo_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        format!(
            r#"<img class="community-cat-photo" src="{url}" alt="{pet_name}" loading="lazy" />"#,
            url = escape_html_attr(url),
            pet_name = pet_name,
        )
    } else {
        r#"<div class="community-cat-photo community-cat-photo-placeholder" aria-hidden="true">🐱</div>"#
            .to_string()
    };

    let has_video = card
        .pet_video_url
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    let video = if let Some(url) = card
        .pet_video_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        format!(
            r#"<div class="community-cat-video-optional" hidden>
      <video
        class="community-cat-video-player"
        src="{url}"
        muted
        playsinline
        webkit-playsinline
        preload="metadata"
        data-clip-start="{clip_start}"
        data-clip-duration="{clip_duration}"
        aria-label="Video of {pet_name} playing"
      ></video>
    </div>"#,
            url = escape_html_attr(url),
            pet_name = pet_name,
            clip_start = card.pet_video_clip_start,
            clip_duration = card.pet_video_clip_duration,
        )
    } else {
        String::new()
    };
    let media_toggle = if has_video {
        format!(
            r#"<button type="button" class="community-cat-media-toggle" aria-pressed="false">Watch {pet_name} play! 🎬</button>"#,
            pet_name = pet_name,
        )
    } else {
        String::new()
    };

    let you_badge = if card.is_viewer {
        r#"<span class="community-cat-you-badge">You</span>"#
    } else {
        ""
    };

    let color_line = if card.pet_color.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="community-cat-color">{color}</p>"#,
            color = escape_html(&card.pet_color),
        )
    };

    let streak_line = if card.care_streak_days >= 3 {
        format!(
            r#"<p class="community-cat-streak">💗 {days}-day care streak</p>"#,
            days = card.care_streak_days,
        )
    } else {
        String::new()
    };

    let outfit_line = if card.equipped_outfit.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="community-cat-outfit">Wearing {outfit}</p>"#,
            outfit = escape_html(&card.equipped_outfit),
        )
    };

    format!(
        r#"<article class="community-cat-card">
  <div class="community-cat-media" data-pet-name="{name}">
    <div class="community-cat-photo-wrap">
      {photo}
      {video}
    </div>
    {media_toggle}
  </div>
  <h3 class="community-cat-name">{name}{you_badge}</h3>
  <p class="community-cat-breed">{breed}</p>
  {color_line}
  <p class="community-cat-level">Parent level {level}</p>
  {streak_line}
  {outfit_line}
  <p class="community-cat-parent">by {parent}</p>
</article>"#,
        photo = photo,
        video = video,
        media_toggle = media_toggle,
        name = pet_name,
        you_badge = you_badge,
        breed = escape_html(&card.pet_breed),
        color_line = color_line,
        level = card.parent_level,
        streak_line = streak_line,
        outfit_line = outfit_line,
        parent = escape_html(&card.author_username),
    )
}

pub fn render_breed_filter_options(selected_slug: &str, profile: &UserProfile) -> String {
    let selected = selected_slug.trim().to_lowercase();
    let mut options = vec![format!(
        r#"<option value=""{}>All breeds</option>"#,
        if selected.is_empty() { " selected" } else { "" },
    )];

    let mut seen = std::collections::HashSet::new();
    for category in breeds::CATALOG {
        for breed in category.breeds {
            let slug = breeds::breed_slug(breed.name);
            if !seen.insert(slug.clone()) {
                continue;
            }
            let is_selected = if selected == slug { " selected" } else { "" };
            options.push(format!(
                r#"<option value="{slug}"{is_selected}>{name}</option>"#,
                slug = escape_html_attr(&slug),
                name = escape_html(breed.name),
            ));
        }
    }

    if profile_has_pet(profile) {
        let mine = breed_slug_for_name(&profile.pet_breed);
        if seen.insert(mine.clone()) {
            let is_selected = if selected == mine { " selected" } else { "" };
            options.push(format!(
                r#"<option value="{slug}"{is_selected}>{name} (your breed)</option>"#,
                slug = escape_html_attr(&mine),
                name = escape_html(&profile.pet_breed),
            ));
        }
    }

    options.join("")
}

pub fn render_cat_feed_section(
    state: &AppState,
    viewer_email: &str,
    profile: &UserProfile,
    breed_filter: Option<&str>,
) -> String {
    let cards = collect_public_cat_cards(state, viewer_email, breed_filter);
    let selected = breed_filter.unwrap_or("");
    let breed_options = render_breed_filter_options(selected, profile);

    let grid = if cards.is_empty() {
        if profile_has_pet(profile) {
            if profile.community_visible {
                r#"<p class="community-feed-empty">No cats match this filter yet. Invite friends or check back as the community grows!</p>"#
                    .to_string()
            } else {
                r#"<p class="community-feed-empty">Your cat is hidden from the community feed. Turn on community visibility on the Account tab to show your photo and playing video here.</p>"#
                    .to_string()
            }
        } else {
            r#"<p class="community-feed-empty">Create your pet profile to join the community feed and see WhiskerWatch cats.</p>"#
                .to_string()
        }
    } else {
        format!(
            r#"<div class="community-cat-grid">{cards}</div>"#,
            cards = cards.iter().map(render_cat_feed_card).collect::<String>(),
        )
    };

    format!(
        r#"<section class="community-section community-section-cats" id="community-cats-panel">
  <div class="community-section-header">
    <h2>Community cats</h2>
    <p class="field-hint">A public showcase of WhiskerWatch cats, including yours. Tap play on cards with a video to watch kitties in action. Hide yours anytime on the Account tab.</p>
  </div>
  <form class="community-breed-filter login-form" action="/home" method="get">
    <input type="hidden" name="tab" value="forum" />
    <input type="hidden" name="community" value="cats" />
    <label for="community-cats-breed">Filter by breed</label>
    <select id="community-cats-breed" name="breed" onchange="this.form.submit()">{breed_options}</select>
  </form>
  {grid}
</section>"#,
        breed_options = breed_options,
        grid = grid,
    )
}

pub fn render_account_visibility_section(profile: &UserProfile) -> String {
    let checked = if profile.community_visible {
        " checked"
    } else {
        ""
    };
    let status = if profile.community_visible {
        "Your cat can appear in the community feed."
    } else {
        "Your cat is hidden from the community feed."
    };

    format!(
        r#"<article class="dashboard-card community-visibility-card">
  <h2>Community privacy</h2>
  <p class="field-hint">{status}</p>
  <form class="login-form community-visibility-form" action="/home/community/visibility" method="post">
    <label class="checkbox-pill">
      <input type="checkbox" name="community_visible" value="on"{checked} />
      Show my cat in the community feed
    </label>
    <button type="submit" class="download-btn login-submit">Save privacy setting</button>
  </form>
</article>"#,
        status = status,
        checked = checked,
    )
}

pub fn render_breed_badge(breed_slug: &str) -> String {
    if breed_slug.trim().is_empty() {
        return r#"<span class="forum-breed-badge forum-breed-badge-general">General</span>"#
            .to_string();
    }

    format!(
        r#"<span class="forum-breed-badge">{label}</span>"#,
        label = escape_html(&breed_label_for_slug(breed_slug)),
    )
}
