use crate::breeds;
use crate::list_pet_summaries;
use crate::pet_snapshot;
use crate::profile_has_pet;
use crate::sharing;
use crate::social_posts;
use crate::user_for_email;
use crate::AppState;
use crate::UserProfile;

pub const MAX_CAT_FEED: usize = 48;
const PET_VIDEO_CLIP_MIN_SECONDS: f32 = 3.0;
const PET_VIDEO_CLIP_MAX_SECONDS: f32 = 6.0;

#[derive(Clone)]
pub struct PublicCatCard {
    pub owner_email: String,
    pub author_username: String,
    pub pet_name: String,
    pub pet_breed: String,
    pub pet_color: String,
    pub care_streak_days: u32,
    pub pet_photo_url: Option<String>,
    pub pet_video_url: Option<String>,
    pub pet_video_clip_start: f32,
    pub pet_video_clip_duration: f32,
    pub equipped_outfit: String,
    pub is_viewer: bool,
    pub deceased: bool,
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
        if sharing::users_block_each_other(state, viewer_email, &email) {
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
                owner_email: email.clone(),
                author_username: author_username.clone(),
                pet_name: snapshot.pet_name.clone(),
                pet_breed: snapshot.pet_breed.clone(),
                pet_color: snapshot.pet_color.clone(),
                care_streak_days: profile.care_streak_days,
                pet_photo_url: snapshot.pet_photo_url.clone(),
                pet_video_url: snapshot.pet_video_url.clone(),
                pet_video_clip_start: clip_start(snapshot.pet_video_clip_start),
                pet_video_clip_duration: clip_duration(snapshot.pet_video_clip_duration),
                equipped_outfit: profile.equipped_outfit.clone(),
                is_viewer,
                deceased: snapshot.deceased,
                breed_slug,
            });
        }
    }

    cards.sort_by(|left, right| {
        match (left.is_viewer, right.is_viewer) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => right
                .care_streak_days
                .cmp(&left.care_streak_days)
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

pub fn render_community_legend() -> String {
    r#"<div class="community-legend" role="note" aria-label="Community card key">
  <p class="community-legend-title">Key</p>
  <ul class="community-legend-list">
    <li>
      <span class="community-legend-symbol community-legend-memorial" aria-hidden="true">★</span>
      <span>Angel cat — passed away and remembered with love</span>
    </li>
    <li>
      <span class="community-legend-symbol community-legend-you">You</span>
      <span>Your cat in the community feed</span>
    </li>
  </ul>
</div>"#
        .to_string()
}

pub fn render_cat_feed_card(state: &AppState, viewer_email: &str, card: &PublicCatCard) -> String {
    let show_personal =
        sharing::can_see_personal_pet_details(state, viewer_email, &card.owner_email);
    let pet_name = escape_html(&card.pet_name);
    let parent_display = escape_html(&card.author_username);
    let media_label = if show_personal {
        pet_name.clone()
    } else {
        format!("{parent_display}'s cat")
    };
    let photo = if let Some(url) = card
        .pet_photo_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        format!(
            r#"<img class="community-cat-photo" src="{url}" alt="{alt}" loading="lazy" />"#,
            url = escape_html_attr(url),
            alt = media_label,
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
        aria-label="Video of {media_label} playing"
      ></video>
    </div>"#,
            url = escape_html_attr(url),
            media_label = media_label,
            clip_start = card.pet_video_clip_start,
            clip_duration = card.pet_video_clip_duration,
        )
    } else {
        String::new()
    };
    let media_toggle = if has_video {
        let aria_label = if show_personal {
            format!("Play {pet_name}'s clip")
        } else {
            "Play clip".to_string()
        };
        format!(
            r#"<button type="button" class="community-cat-media-toggle community-cat-media-toggle--clip" aria-pressed="false" aria-label="{aria}">✨🐾</button>"#,
            aria = escape_html_attr(&aria_label),
        )
    } else {
        String::new()
    };

    let you_badge = if card.is_viewer {
        r#"<span class="community-cat-you-badge">You</span>"#
    } else {
        ""
    };

    let memorial_badge = if card.deceased {
        r#"<span class="community-cat-memorial-badge" title="Angel cat">★</span>"#
    } else {
        ""
    };

    let memorial_line = if card.deceased {
        r#"<p class="community-cat-memorial-status"><span aria-hidden="true">★</span> Angel cat</p>"#
            .to_string()
    } else {
        String::new()
    };

    let card_class = if card.deceased {
        "community-cat-card community-cat-card-memorial"
    } else {
        "community-cat-card"
    };

    let card_title = if show_personal {
        pet_name.clone()
    } else {
        parent_display.clone()
    };

    let breed_line = if show_personal {
        format!(
            r#"<p class="community-cat-breed">{breed}</p>"#,
            breed = escape_html(&card.pet_breed),
        )
    } else {
        String::new()
    };

    let color_line = if show_personal && !card.pet_color.trim().is_empty() {
        format!(
            r#"<p class="community-cat-color">{color}</p>"#,
            color = escape_html(&card.pet_color),
        )
    } else {
        String::new()
    };

    let streak_line = if show_personal && !card.deceased && card.care_streak_days >= 3 {
        format!(
            r#"<p class="community-cat-streak">💗 {days}-day care streak</p>"#,
            days = card.care_streak_days,
        )
    } else {
        String::new()
    };

    let outfit_line = if show_personal && !card.equipped_outfit.trim().is_empty() {
        format!(
            r#"<p class="community-cat-outfit">Wearing {outfit}</p>"#,
            outfit = escape_html(&card.equipped_outfit),
        )
    } else {
        String::new()
    };

    let parent_line = if show_personal {
        format!(r#"<p class="community-cat-parent">by {parent_display}</p>"#)
    } else {
        String::new()
    };

    let profile_url = escape_html_attr(&social_posts::parent_profile_url(&card.author_username));
    let profile_label = if card.is_viewer {
        "View your profile and posts"
    } else {
        "View profile and posts"
    };
    let interact_menu = if card.is_viewer {
        String::new()
    } else {
        sharing::render_profile_interact_menu(
            state,
            viewer_email,
            &card.owner_email,
            &card.author_username,
            false,
        )
    };
    let friend_action_line = interact_menu;

    format!(
        r#"<article class="{card_class}">
  {memorial_badge}
  {friend_action_line}
  <div class="community-cat-media" data-pet-name="{name}">
    <a href="{profile_url}" class="community-cat-card-link" aria-label="{profile_label} for {parent_display}">
      <div class="community-cat-photo-wrap">
        {photo}
        {video}
      </div>
      <h3 class="community-cat-name">{name}{you_badge}</h3>
      <div class="community-cat-details">
        {breed_line}
        {color_line}
        {memorial_line}
        {streak_line}
        {outfit_line}
        {parent_line}
      </div>
    </a>
    <div class="community-cat-media-footer">
      {media_toggle}
    </div>
  </div>
</article>"#,
        card_class = card_class,
        profile_url = profile_url,
        profile_label = profile_label,
        parent_display = parent_display,
        photo = photo,
        video = video,
        media_toggle = media_toggle,
        memorial_badge = memorial_badge,
        name = card_title,
        you_badge = you_badge,
        memorial_line = memorial_line,
        breed_line = breed_line,
        color_line = color_line,
        streak_line = streak_line,
        outfit_line = outfit_line,
        parent_line = parent_line,
        friend_action_line = friend_action_line,
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
            cards = cards
                .iter()
                .map(|card| render_cat_feed_card(state, viewer_email, card))
                .collect::<String>(),
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
