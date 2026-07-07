use crate::{
    escape_html, escape_html_attr, push_activity,
    share_cards::{format_cute_streak_markup, CuteStreakStyle, STREAK_MILESTONES},
    UserProfile,
};

pub struct StreakRewardTier {
    pub days: u32,
    pub paw_points: u32,
    pub title: &'static str,
    pub badge: &'static str,
    pub blurb: &'static str,
}

pub const REWARD_TIERS: &[StreakRewardTier] = &[
    StreakRewardTier {
        days: 3,
        paw_points: 15,
        title: "First flame",
        badge: "🔥",
        blurb: "Three days of showing up for your cat — that's the spark!",
    },
    StreakRewardTier {
        days: 7,
        paw_points: 35,
        title: "Week warrior",
        badge: "⭐",
        blurb: "A full week of daily care. Your kitty notices the routine.",
    },
    StreakRewardTier {
        days: 14,
        paw_points: 75,
        title: "Fortnight fur-riend",
        badge: "🐾",
        blurb: "Two steady weeks — you're building real parent habits.",
    },
    StreakRewardTier {
        days: 30,
        paw_points: 150,
        title: "Monthly marvel",
        badge: "🌟",
        blurb: "Thirty days of love in action. That's dedication.",
    },
    StreakRewardTier {
        days: 60,
        paw_points: 300,
        title: "Dedicated parent",
        badge: "💗",
        blurb: "Sixty days strong — your cat's care rhythm is rock solid.",
    },
    StreakRewardTier {
        days: 100,
        paw_points: 500,
        title: "Century champion",
        badge: "👑",
        blurb: "One hundred days! Legendary whisker parent energy.",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardState {
    Locked { days_remaining: u32 },
    Ready,
    Claimed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimError {
    InvalidMilestone,
    NotReached,
    AlreadyClaimed,
}

pub fn tier_for_days(days: u32) -> Option<&'static StreakRewardTier> {
    REWARD_TIERS.iter().find(|tier| tier.days == days)
}

pub fn reward_state(profile: &UserProfile, days: u32) -> RewardState {
    if profile
        .claimed_streak_rewards
        .iter()
        .any(|claimed| *claimed == days)
    {
        return RewardState::Claimed;
    }
    if profile.care_streak_days >= days {
        return RewardState::Ready;
    }
    RewardState::Locked {
        days_remaining: days.saturating_sub(profile.care_streak_days),
    }
}

pub fn next_milestone(profile: &UserProfile) -> Option<u32> {
    STREAK_MILESTONES
        .iter()
        .copied()
        .find(|milestone| *milestone > profile.care_streak_days)
}

pub fn claim_streak_reward(profile: &mut UserProfile, days: u32) -> Result<u32, ClaimError> {
    let Some(tier) = tier_for_days(days) else {
        return Err(ClaimError::InvalidMilestone);
    };
    if profile
        .claimed_streak_rewards
        .iter()
        .any(|claimed| *claimed == days)
    {
        return Err(ClaimError::AlreadyClaimed);
    }
    if profile.care_streak_days < days {
        return Err(ClaimError::NotReached);
    }

    profile.claimed_streak_rewards.push(days);
    profile.paw_points = profile.paw_points.saturating_add(tier.paw_points);
    push_activity(
        profile,
        &format!(
            "Collected {days}-day streak reward: +{} paw points ({})!",
            tier.paw_points, tier.title
        ),
    );
    Ok(tier.paw_points)
}

pub fn render_care_streak_chip(profile: &UserProfile) -> String {
    let aria = if profile.care_streak_days == 0 {
        "Care streak — keep going rewards".to_string()
    } else if profile.care_streak_days == 1 {
        "Care streak: 1 day — view rewards".to_string()
    } else {
        format!(
            "Care streak: {} days — view rewards",
            profile.care_streak_days
        )
    };
    let label = format_cute_streak_markup(profile.care_streak_days, CuteStreakStyle::Chip);

    format!(
        r#"<a href="/home/streak" class="stat-chip stat-chip-button care-streak-chip" aria-label="{aria}"><span class="stat-label">Streak</span><span class="stat-value">{label}</span></a>"#,
        aria = escape_html_attr(&aria),
        label = label,
    )
}

pub fn render_status_block(status: Option<&str>, points: Option<u32>) -> String {
    match status {
        Some("streak_reward_claimed") => {
            let bonus = points.unwrap_or(0);
            format!(
                r#"<p class="auth-success status-flash" role="status">Reward collected! +{bonus} paw points added to your balance. 🐾</p>"#
            )
        }
        Some("streak_reward_locked") => {
            r#"<p class="auth-error status-flash" role="alert">Keep caring daily — you haven't reached that streak milestone yet.</p>"#
                .to_string()
        }
        Some("streak_reward_claimed_already") => {
            r#"<p class="auth-error status-flash" role="alert">You already collected that streak reward.</p>"#
                .to_string()
        }
        Some("streak_reward_invalid") => {
            r#"<p class="auth-error status-flash" role="alert">That streak reward could not be collected. Please try again.</p>"#
                .to_string()
        }
        _ => String::new(),
    }
}

pub fn render_keep_going_content(
    profile: &UserProfile,
    pet_name: &str,
    status: Option<&str>,
    claimed_points: Option<u32>,
) -> String {
    let pet = escape_html(pet_name);
    let streak_label = format_cute_streak_markup(profile.care_streak_days, CuteStreakStyle::Hero);
    let best = profile.best_care_streak;
    let best_line = if best > profile.care_streak_days && best > 0 {
        format!(
            r#"<p class="streak-keep-best">Personal best: <strong>{best} days</strong></p>"#,
            best = best
        )
    } else {
        String::new()
    };

    let progress = next_milestone(profile).map(|milestone| {
        let remaining = milestone.saturating_sub(profile.care_streak_days);
        format!(
            r#"<p class="streak-keep-progress">Next reward at <strong>{milestone} days</strong> — {remaining} more daily task{plural} to go! 🐾</p>"#,
            plural = if remaining == 1 { "" } else { "s" }
        )
    }).unwrap_or_else(|| {
        r#"<p class="streak-keep-progress">You've unlocked every streak reward — incredible dedication! 💗</p>"#
            .to_string()
    });

    let ready_count = REWARD_TIERS
        .iter()
        .filter(|tier| reward_state(profile, tier.days) == RewardState::Ready)
        .count();
    let collect_hint = if ready_count > 0 {
        format!(
            r#"<p class="streak-keep-collect-hint" role="status"><strong>{ready_count}</strong> reward{plural} ready to collect!</p>"#,
            plural = if ready_count == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };

    let reward_cards: String = REWARD_TIERS
        .iter()
        .map(|tier| {
            let state = reward_state(profile, tier.days);
            let state_class = match state {
                RewardState::Ready => "streak-reward-ready",
                RewardState::Claimed => "streak-reward-claimed",
                RewardState::Locked { .. } => "streak-reward-locked",
            };
            let action = match state {
                RewardState::Ready => format!(
                    r#"<form action="/home/streak/claim" method="post" class="streak-reward-claim-form">
  <input type="hidden" name="milestone" value="{days}" />
  <button type="submit" class="download-btn streak-reward-collect-btn">Collect +{points} paw points</button>
</form>"#,
                    days = tier.days,
                    points = tier.paw_points,
                ),
                RewardState::Claimed => {
                    r#"<p class="streak-reward-status streak-reward-status-claimed">Collected ✓</p>"#
                        .to_string()
                }
                RewardState::Locked { days_remaining } => format!(
                    r#"<p class="streak-reward-status streak-reward-status-locked">{remaining} more day{plural} to unlock</p>"#,
                    remaining = days_remaining,
                    plural = if days_remaining == 1 { "" } else { "s" },
                ),
            };
            format!(
                r#"<article class="streak-reward-card {state_class}">
  <div class="streak-reward-badge" aria-hidden="true">{badge}</div>
  <div class="streak-reward-copy">
    <h3>{title} <span class="streak-reward-days">{days}-day streak</span></h3>
    <p>{blurb}</p>
    <p class="streak-reward-points">+{points} paw points</p>
  </div>
  {action}
</article>"#,
                badge = tier.badge,
                title = escape_html(tier.title),
                days = tier.days,
                blurb = escape_html(tier.blurb),
                points = tier.paw_points,
                action = action,
                state_class = state_class,
            )
        })
        .collect();

    let status_block = render_status_block(status, claimed_points);

    include_str!("../templates/streak-keep-going.html")
        .replace("{{STATUS_BLOCK}}", &status_block)
        .replace("{{PET}}", &pet)
        .replace("{{STREAK_LABEL}}", &streak_label)
        .replace("{{BEST_LINE}}", &best_line)
        .replace("{{PROGRESS}}", &progress)
        .replace("{{COLLECT_HINT}}", &collect_hint)
        .replace("{{REWARD_CARDS}}", &reward_cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_profile;

    #[test]
    fn claim_adds_points_once() {
        let mut profile = default_profile("streak@example.com");
        profile.care_streak_days = 7;
        let points = claim_streak_reward(&mut profile, 7).expect("claim");
        assert_eq!(points, 35);
        assert_eq!(profile.paw_points, 35);
        assert!(claim_streak_reward(&mut profile, 7).is_err());
    }

    #[test]
    fn cannot_claim_before_milestone() {
        let mut profile = default_profile("streak@example.com");
        profile.care_streak_days = 5;
        assert_eq!(
            claim_streak_reward(&mut profile, 7),
            Err(ClaimError::NotReached)
        );
    }

    #[test]
    fn streak_chip_links_to_keep_going_page() {
        let mut profile = default_profile("streak@example.com");
        profile.care_streak_days = 4;
        let html = render_care_streak_chip(&profile);
        assert!(html.contains(r#"href="/home/streak""#));
        assert!(html.contains("stat-chip-button"));
    }
}
