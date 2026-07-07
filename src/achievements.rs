use crate::entitlements;
use crate::streak_rewards::{self, StreakRewardTier};
use crate::{escape_html, list_pet_summaries, UserProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achievement {
    pub id: &'static str,
    pub badge: &'static str,
    pub title: String,
    pub detail: String,
}

pub fn collect_achievements(profile: &UserProfile) -> Vec<Achievement> {
    let mut achievements = Vec::new();

    for tier in streak_rewards::REWARD_TIERS {
        if profile.best_care_streak >= tier.days
            || profile
                .claimed_streak_rewards
                .iter()
                .any(|claimed| *claimed == tier.days)
        {
            achievements.push(streak_achievement(tier));
        }
    }

    if entitlements::has_premium(profile.premium_unlocked, &profile.email) {
        achievements.push(Achievement {
            id: "plus_member",
            badge: "✨",
            title: "WhiskerWatch Plus".to_string(),
            detail: "Lifetime premium member".to_string(),
        });
    }

    let pet_count = list_pet_summaries(profile).len();
    if pet_count >= 2 {
        achievements.push(Achievement {
            id: "multi_cat_parent",
            badge: "🐱",
            title: "Multi-cat parent".to_string(),
            detail: format!("Caring for {pet_count} cats"),
        });
    }

    let guide_count = profile.owned_breed_guides.len();
    if guide_count > 0 {
        achievements.push(Achievement {
            id: "breed_guides",
            badge: "📚",
            title: if guide_count == 1 {
                "Breed guide unlocked".to_string()
            } else {
                "Breed guide collector".to_string()
            },
            detail: if guide_count == 1 {
                "Unlocked a premium breed care guide".to_string()
            } else {
                format!("Unlocked {guide_count} premium breed care guides")
            },
        });
    }

    achievements
}

pub fn collect_parent_profile_achievements(
    profile: &UserProfile,
    is_self: bool,
) -> Vec<Achievement> {
    collect_achievements(profile)
        .into_iter()
        .filter(|achievement| is_self || !is_private_parent_profile_achievement(achievement))
        .collect()
}

fn is_private_parent_profile_achievement(achievement: &Achievement) -> bool {
    matches!(achievement.id, "plus_member")
}

fn streak_achievement(tier: &StreakRewardTier) -> Achievement {
    Achievement {
        id: "streak",
        badge: tier.badge,
        title: tier.title.to_string(),
        detail: format!("{days}-day care streak", days = tier.days),
    }
}

pub fn render_parent_profile_achievements(profile: &UserProfile, is_self: bool) -> String {
    let achievements = collect_parent_profile_achievements(profile, is_self);
    if achievements.is_empty() {
        if is_self {
            return r#"<section class="parent-profile-achievements-section dashboard-card">
  <h2 class="parent-profile-achievements-title">Achievements</h2>
  <p class="parent-profile-achievements-empty">Complete daily care tasks to build your streak and unlock achievements here.</p>
</section>"#
                .to_string();
        }
        return String::new();
    }

    let items = achievements
        .iter()
        .map(|achievement| {
            format!(
                r#"<li class="parent-profile-achievement">
  <span class="parent-profile-achievement-badge" aria-hidden="true">{badge}</span>
  <div class="parent-profile-achievement-copy">
    <span class="parent-profile-achievement-title">{title}</span>
    <span class="parent-profile-achievement-detail">{detail}</span>
  </div>
</li>"#,
                badge = achievement.badge,
                title = escape_html(&achievement.title),
                detail = escape_html(&achievement.detail),
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<section class="parent-profile-achievements-section dashboard-card">
  <h2 class="parent-profile-achievements-title">Achievements</h2>
  <ul class="parent-profile-achievements">{items}</ul>
</section>"#,
        items = items,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_profile;

    #[test]
    fn streak_and_plus_achievements_render_on_profile() {
        let mut profile = default_profile("achieve@example.com");
        profile.premium_unlocked = true;
        profile.best_care_streak = 7;
        profile.claimed_streak_rewards.push(3);

        let achievements = collect_achievements(&profile);
        assert!(achievements.iter().any(|item| item.title == "First flame"));
        assert!(achievements.iter().any(|item| item.title == "Week warrior"));
        assert!(achievements
            .iter()
            .any(|item| item.title == "WhiskerWatch Plus"));

        let html = render_parent_profile_achievements(&profile, false);
        assert!(html.contains("parent-profile-achievements"));
        assert!(html.contains("Week warrior"));
        assert!(!html.contains("WhiskerWatch Plus"));

        let own_html = render_parent_profile_achievements(&profile, true);
        assert!(own_html.contains("WhiskerWatch Plus"));
    }

    #[test]
    fn plus_achievement_hidden_on_public_parent_profiles() {
        let mut profile = default_profile("plus@example.com");
        profile.premium_unlocked = true;
        let public = collect_parent_profile_achievements(&profile, false);
        assert!(public.iter().all(|item| item.id != "plus_member"));
        let own = collect_parent_profile_achievements(&profile, true);
        assert!(own.iter().any(|item| item.id == "plus_member"));
    }

    #[test]
    fn empty_achievements_hidden_for_other_parents() {
        let profile = default_profile("new@example.com");
        assert!(render_parent_profile_achievements(&profile, false).is_empty());
        assert!(render_parent_profile_achievements(&profile, true).contains("Achievements"));
    }
}
