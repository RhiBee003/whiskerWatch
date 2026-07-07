use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::achievements;
use crate::profile_has_pet;
use crate::storage::{StorageError, StoredSocialPost};
use crate::{AppState, UserProfile};

const MAX_COLLAGE_PHOTOS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedAchievementSnapshot {
    pub badge: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedPayload {
    pub year: u32,
    pub month: u32,
    pub month_label: String,
    pub parent_grade: String,
    pub parent_score: u32,
    pub parent_level: u32,
    pub post_count: u32,
    pub total_upvotes: u32,
    pub achievements: Vec<WrappedAchievementSnapshot>,
    pub collage_urls: Vec<String>,
}

pub fn previous_calendar_month(now: chrono::DateTime<Utc>) -> (u32, u32) {
    let date = now.date_naive();
    if date.month() == 1 {
        ((date.year() - 1) as u32, 12)
    } else {
        (date.year() as u32, date.month() - 1)
    }
}

pub fn month_range_unix(year: u32, month: u32) -> Option<(u64, u64)> {
    let start_date = NaiveDate::from_ymd_opt(year as i32, month, 1)?;
    let end_date = if month == 12 {
        NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year as i32, month + 1, 1)?
    };
    let start = Utc
        .from_utc_datetime(&start_date.and_hms_opt(0, 0, 0)?)
        .timestamp() as u64;
    let end = (Utc
        .from_utc_datetime(&end_date.and_hms_opt(0, 0, 0)?)
        .timestamp() as u64)
        .saturating_sub(1);
    Some((start, end))
}

pub fn month_label(year: u32, month: u32) -> String {
    NaiveDate::from_ymd_opt(year as i32, month, 1)
        .map(|date| date.format("%B %Y").to_string())
        .unwrap_or_else(|| format!("{month}/{year}"))
}

pub fn compute_parent_grade(
    post_count: u32,
    parent_level: u32,
    achievement_count: usize,
    total_upvotes: u32,
) -> (String, u32) {
    let post_score = (post_count.min(20) * 3).min(30);
    let level_score = (parent_level.min(20) * 2).min(30);
    let achievement_score = (achievement_count as u32 * 5).min(25);
    let upvote_score = total_upvotes.min(25);
    let total = post_score + level_score + achievement_score + upvote_score;
    let grade = match total {
        90..=110 => "S",
        75..=89 => "A",
        60..=74 => "B",
        45..=59 => "C",
        30..=44 => "D",
        _ => "F",
    };
    (grade.to_string(), total)
}

fn collect_collage_urls(posts: &[StoredSocialPost]) -> Vec<String> {
    let mut urls = Vec::new();
    for post in posts {
        if !post.media_items.is_empty() {
            for item in &post.media_items {
                if item.media_type == "photo" && !item.media_url.trim().is_empty() {
                    urls.push(item.media_url.clone());
                    if urls.len() >= MAX_COLLAGE_PHOTOS {
                        return urls;
                    }
                }
            }
        } else if post.media_type == "photo" {
            if let Some(url) = post.media_url.as_deref().filter(|value| !value.is_empty()) {
                urls.push(url.to_string());
                if urls.len() >= MAX_COLLAGE_PHOTOS {
                    return urls;
                }
            }
        }
    }
    urls
}

pub fn build_monthly_wrapped_payload(
    state: &AppState,
    profile: &UserProfile,
    year: u32,
    month: u32,
) -> Result<WrappedPayload, StorageError> {
    let (start_ts, end_ts) = month_range_unix(year, month)
        .ok_or_else(|| StorageError::InvalidInput("invalid wrapped month".into()))?;
    let mut posts =
        state
            .storage
            .list_user_standard_posts_in_range(&profile.email, start_ts, end_ts)?;
    state
        .storage
        .hydrate_social_posts_engagement(&mut posts, None)?;
    let total_upvotes = posts.iter().map(|post| post.upvotes).sum();

    let achievements = achievements::collect_achievements(profile)
        .into_iter()
        .map(|item| WrappedAchievementSnapshot {
            badge: item.badge.to_string(),
            title: item.title,
            detail: item.detail,
        })
        .collect::<Vec<_>>();
    let (parent_grade, parent_score) = compute_parent_grade(
        posts.len() as u32,
        profile.parent_level,
        achievements.len(),
        total_upvotes,
    );

    Ok(WrappedPayload {
        year,
        month,
        month_label: month_label(year, month),
        parent_grade,
        parent_score,
        parent_level: profile.parent_level,
        post_count: posts.len() as u32,
        total_upvotes,
        achievements,
        collage_urls: collect_collage_urls(&posts),
    })
}

pub fn maybe_publish_monthly_wrapped(
    state: &AppState,
    profile: &UserProfile,
    username: &str,
    now_ts: u64,
) -> Result<Option<String>, StorageError> {
    if !profile.community_visible || !profile_has_pet(profile) {
        return Ok(None);
    }

    let now = chrono::DateTime::<Utc>::from_timestamp(now_ts as i64, 0).unwrap_or_else(Utc::now);
    let (year, month) = previous_calendar_month(now);
    if state
        .storage
        .has_monthly_wrapped(&profile.email, year, month)?
    {
        return Ok(None);
    }

    let payload = build_monthly_wrapped_payload(state, profile, year, month)?;
    let body = format!(
        "🎀 My {} Parent Wrapped — Grade {}",
        payload.month_label, payload.parent_grade
    );
    let payload_json = serde_json::to_string(&payload)?;
    let post = state.storage.create_monthly_wrapped_post(
        &profile.email,
        username,
        &body,
        &payload_json,
        year,
        month,
        now_ts,
    )?;
    Ok(Some(post.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_profile;
    use crate::storage::Storage;
    use crate::storage::StoredSocialPostMedia;
    use uuid::Uuid;

    #[test]
    fn compute_parent_grade_maps_score_to_letter() {
        assert_eq!(compute_parent_grade(0, 0, 0, 0), ("F".to_string(), 0));
        assert_eq!(compute_parent_grade(10, 10, 4, 20), ("S".to_string(), 90));
        assert_eq!(compute_parent_grade(20, 20, 5, 50), ("S".to_string(), 110));
    }

    #[test]
    fn previous_calendar_month_rolls_over_january() {
        let jan = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        assert_eq!(previous_calendar_month(jan), (2025, 12));
    }

    #[test]
    fn monthly_wrapped_publishes_once_per_month() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-parent-wrapped-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };

        let mut profile = default_profile("wrapped@test.local");
        profile.community_visible = true;
        profile.parent_level = 6;
        profile.best_care_streak = 7;
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_age_weeks = Some(52);
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        state.storage.save_profile(&profile).expect("save profile");

        let march = Utc.with_ymd_and_hms(2026, 3, 5, 12, 0, 0).unwrap();
        let (year, month) = previous_calendar_month(march);
        let (start, end) = month_range_unix(year, month).expect("range");
        let mid_month = start + (end - start) / 2;
        state
            .storage
            .create_social_post(
                &profile.email,
                "wrappedparent",
                "February zoomies",
                &[StoredSocialPostMedia {
                    media_type: "photo".to_string(),
                    media_url: "/uploads/cat.jpg".to_string(),
                    video_duration: None,
                    sort_order: 0,
                }],
                false,
                mid_month,
            )
            .expect("create post");

        let published = maybe_publish_monthly_wrapped(
            &state,
            &profile,
            "wrappedparent",
            march.timestamp() as u64,
        )
        .expect("publish");
        assert!(published.is_some());
        assert!(state
            .storage
            .has_monthly_wrapped(&profile.email, year, month)
            .expect("check wrapped"));

        let again = maybe_publish_monthly_wrapped(
            &state,
            &profile,
            "wrappedparent",
            march.timestamp() as u64,
        )
        .expect("publish again");
        assert!(again.is_none());
    }
}
