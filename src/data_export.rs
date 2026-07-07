use crate::{
    format_member_since, storage, timestamp_now, user_for_email, AppState, ForumPost, ForumReply,
    UserProfile,
};
use chrono::TimeZone;
use serde::Serialize;
use storage::{
    StoredFriendMessage, StoredFriendSummary, StoredPetShare, StoredSocialPost,
    StoredSocialPostComment, StoredSocialPostMedia,
};

const EXPORT_FORMAT_VERSION: &str = "1";
const EXPORT_CONVERSATION_LIMIT: usize = 10_000;
const EXPORT_SOCIAL_POST_LIMIT: usize = 10_000;

#[derive(Serialize)]
struct ExportedAccount {
    username: String,
    first_name: String,
    last_name: String,
    email: String,
    member_since: String,
    created_at: u64,
}

#[derive(Serialize)]
struct ExportedForumPost {
    #[serde(flatten)]
    post: ForumPost,
    replies: Vec<ForumReply>,
}

#[derive(Serialize)]
struct ExportedSocialPostMedia {
    media_type: String,
    media_url: String,
    video_duration: Option<f32>,
    sort_order: u32,
}

#[derive(Serialize)]
struct ExportedSocialPostComment {
    id: String,
    post_id: String,
    user_id: String,
    author_username: String,
    body: String,
    created_at: u64,
    upvotes: u32,
}

#[derive(Serialize)]
struct ExportedSocialPost {
    id: String,
    body: String,
    media_type: String,
    media_url: Option<String>,
    video_duration: Option<f32>,
    is_private: bool,
    created_at: u64,
    post_kind: String,
    wrapped_payload: Option<String>,
    wrapped_year: Option<u32>,
    wrapped_month: Option<u32>,
    media_items: Vec<ExportedSocialPostMedia>,
    upvotes: u32,
    comments: Vec<ExportedSocialPostComment>,
}

#[derive(Serialize)]
struct ExportedFriendMessage {
    id: String,
    from_email: String,
    to_email: String,
    body: String,
    media_type: String,
    media_url: Option<String>,
    video_duration: Option<f32>,
    created_at: u64,
    read_at: Option<u64>,
}

#[derive(Serialize)]
struct ExportedFriendConversation {
    partner_email: String,
    messages: Vec<ExportedFriendMessage>,
}

#[derive(Serialize)]
struct ExportedPetShare {
    id: String,
    owner_email: String,
    shared_with_email: String,
    pet_id: String,
    status: String,
    created_at: u64,
}

#[derive(Serialize)]
struct UserDataExport {
    format_version: &'static str,
    app: &'static str,
    exported_at: u64,
    account: ExportedAccount,
    profile: UserProfile,
    forum_posts: Vec<ExportedForumPost>,
    forum_replies: Vec<ForumReply>,
    feedback_submissions: Vec<crate::FeedbackSubmission>,
    social_posts: Vec<ExportedSocialPost>,
    friends: Vec<String>,
    pet_shares_outgoing: Vec<ExportedPetShare>,
    pet_shares_incoming: Vec<ExportedPetShare>,
    friend_conversations: Vec<ExportedFriendConversation>,
}

pub fn render_account_data_export_section() -> String {
    r##"<article class="dashboard-card account-data-export-card">
  <h2>Your data</h2>
  <p class="field-hint">Download a JSON backup of your account, cats, care history, forum posts, and messages. You can keep it for your records or move to another service — WhiskerWatch never locks you in.</p>
  <p class="account-data-export-actions">
    <a href="/home/data-export" class="download-btn account-data-export-btn" download>Download my data</a>
  </p>
</article>"##.to_string()
}

pub async fn build_export(
    state: &AppState,
    email: &str,
) -> Result<(String, Vec<u8>), storage::StorageError> {
    let user = user_for_email(state, email);
    let profile = state
        .storage
        .load_profile(email)?
        .ok_or_else(|| storage::StorageError::InvalidInput("profile not found".into()))?;

    let account = ExportedAccount {
        username: user
            .as_ref()
            .map(|u| u.username.clone())
            .unwrap_or_else(|| "Parent".to_string()),
        first_name: user
            .as_ref()
            .map(|u| u.first_name.clone())
            .unwrap_or_default(),
        last_name: user
            .as_ref()
            .map(|u| u.last_name.clone())
            .unwrap_or_default(),
        email: email.to_string(),
        member_since: user
            .as_ref()
            .map(|u| format_member_since(u.created_at))
            .unwrap_or_else(|| "Unknown".to_string()),
        created_at: user.as_ref().map(|u| u.created_at).unwrap_or(0),
    };

    let forum_posts = collect_forum_posts(state, email)?;
    let forum_replies = state.storage.list_forum_replies_by_user(email)?;
    let feedback_submissions = collect_feedback(state, email)?;
    let social_posts = collect_social_posts(state, email)?;
    let friends = state
        .storage
        .list_friends(email)?
        .into_iter()
        .map(|friend: StoredFriendSummary| friend.friend_email)
        .collect();
    let pet_shares_outgoing = state
        .storage
        .list_outgoing_pet_shares(email)?
        .into_iter()
        .map(ExportedPetShare::from)
        .collect();
    let pet_shares_incoming = state
        .storage
        .list_incoming_pet_shares(email)?
        .into_iter()
        .map(ExportedPetShare::from)
        .collect();
    let friend_conversations = collect_friend_conversations(state, email)?;

    let export = UserDataExport {
        format_version: EXPORT_FORMAT_VERSION,
        app: "WhiskerWatch",
        exported_at: timestamp_now(),
        account,
        profile,
        forum_posts,
        forum_replies,
        feedback_submissions,
        social_posts,
        friends,
        pet_shares_outgoing,
        pet_shares_incoming,
        friend_conversations,
    };

    let json = serde_json::to_vec_pretty(&export)?;
    let filename = export_filename(&export.account.username, email);
    Ok((filename, json))
}

fn collect_forum_posts(
    state: &AppState,
    email: &str,
) -> Result<Vec<ExportedForumPost>, storage::StorageError> {
    let posts = state.storage.list_forum_posts_by_user(email)?;
    let mut exported = Vec::with_capacity(posts.len());
    for post in posts {
        let replies = state.storage.list_forum_replies(post.id)?;
        exported.push(ExportedForumPost { post, replies });
    }
    Ok(exported)
}

fn collect_feedback(
    state: &AppState,
    email: &str,
) -> Result<Vec<crate::FeedbackSubmission>, storage::StorageError> {
    let email_lower = email.trim().to_ascii_lowercase();
    Ok(state
        .storage
        .load_feedback()?
        .into_iter()
        .filter(|entry| {
            entry
                .user_id
                .as_deref()
                .is_some_and(|user_id| user_id.eq_ignore_ascii_case(email))
                || entry.email.eq_ignore_ascii_case(&email_lower)
        })
        .collect())
}

fn collect_social_posts(
    state: &AppState,
    email: &str,
) -> Result<Vec<ExportedSocialPost>, storage::StorageError> {
    Ok(state
        .storage
        .list_social_posts_for_user(email, EXPORT_SOCIAL_POST_LIMIT)?
        .into_iter()
        .map(ExportedSocialPost::from)
        .collect())
}

fn collect_friend_conversations(
    state: &AppState,
    email: &str,
) -> Result<Vec<ExportedFriendConversation>, storage::StorageError> {
    let partners = state.storage.list_message_thread_partners(email)?;
    let mut conversations = Vec::with_capacity(partners.len());
    for partner_email in partners {
        let messages = state
            .storage
            .list_friend_conversation(email, &partner_email, EXPORT_CONVERSATION_LIMIT)?
            .into_iter()
            .map(ExportedFriendMessage::from)
            .collect();
        conversations.push(ExportedFriendConversation {
            partner_email,
            messages,
        });
    }
    Ok(conversations)
}

fn export_filename(username: &str, email: &str) -> String {
    let slug: String = username
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(40)
        .collect();
    let slug = if slug.is_empty() {
        email
            .split('@')
            .next()
            .unwrap_or("user")
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .take(40)
            .collect::<String>()
    } else {
        slug
    };
    let date = chrono::Utc
        .timestamp_opt(timestamp_now() as i64, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "export".to_string());
    format!("whiskerwatch-backup-{slug}-{date}.json")
}

impl From<StoredPetShare> for ExportedPetShare {
    fn from(share: StoredPetShare) -> Self {
        Self {
            id: share.id,
            owner_email: share.owner_email,
            shared_with_email: share.shared_with_email,
            pet_id: share.pet_id,
            status: share.status,
            created_at: share.created_at,
        }
    }
}

impl From<StoredFriendMessage> for ExportedFriendMessage {
    fn from(message: StoredFriendMessage) -> Self {
        Self {
            id: message.id,
            from_email: message.from_email,
            to_email: message.to_email,
            body: message.body,
            media_type: message.media_type,
            media_url: message.media_url,
            video_duration: message.video_duration,
            created_at: message.created_at,
            read_at: message.read_at,
        }
    }
}

impl From<StoredSocialPostMedia> for ExportedSocialPostMedia {
    fn from(media: StoredSocialPostMedia) -> Self {
        Self {
            media_type: media.media_type,
            media_url: media.media_url,
            video_duration: media.video_duration,
            sort_order: media.sort_order,
        }
    }
}

impl From<StoredSocialPostComment> for ExportedSocialPostComment {
    fn from(comment: StoredSocialPostComment) -> Self {
        Self {
            id: comment.id,
            post_id: comment.post_id,
            user_id: comment.user_id,
            author_username: comment.author_username,
            body: comment.body,
            created_at: comment.created_at,
            upvotes: comment.upvotes,
        }
    }
}

impl From<StoredSocialPost> for ExportedSocialPost {
    fn from(post: StoredSocialPost) -> Self {
        Self {
            id: post.id,
            body: post.body,
            media_type: post.media_type,
            media_url: post.media_url,
            video_duration: post.video_duration,
            is_private: post.is_private,
            created_at: post.created_at,
            post_kind: post.post_kind,
            wrapped_payload: post.wrapped_payload,
            wrapped_year: post.wrapped_year,
            wrapped_month: post.wrapped_month,
            media_items: post
                .media_items
                .into_iter()
                .map(ExportedSocialPostMedia::from)
                .collect(),
            upvotes: post.upvotes,
            comments: post
                .comments
                .into_iter()
                .map(ExportedSocialPostComment::from)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_profile, storage::Storage, User};
    use uuid::Uuid;

    #[test]
    fn export_filename_sanitizes_username() {
        let filename = export_filename("Cinder's Mom!", "user@example.com");
        assert!(filename.starts_with("whiskerwatch-backup-CindersMom-"));
        assert!(filename.ends_with(".json"));
    }

    #[tokio::test]
    async fn build_export_includes_profile_and_account() {
        let storage = Storage::open_at(
            std::env::temp_dir().join(format!("ww-data-export-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let email = "export@test.local";
        storage
            .save_user(&User {
                username: "exportuser".to_string(),
                first_name: "Export".to_string(),
                last_name: "User".to_string(),
                email: email.to_string(),
                password: "password123".to_string(),
                created_at: 1_700_000_000,
            })
            .expect("save user");
        let mut profile = default_profile(email);
        profile.pet_name = "Mittens".to_string();
        storage.save_profile(&profile).expect("save profile");

        let state = AppState { storage };
        let (filename, bytes) = build_export(&state, email).await.expect("export");
        assert!(filename.contains("exportuser"));
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(json["format_version"], "1");
        assert_eq!(json["account"]["email"], email);
        assert_eq!(json["profile"]["pet_name"], "Mittens");
    }
}
