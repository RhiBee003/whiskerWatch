use crate::parent_wrapped::WrappedPayload;
use crate::pet_id_posts::PetIdPostPayload;
use crate::sharing;
use crate::storage::StorageError;
use crate::storage::StoredSocialPost;
use crate::storage::StoredSocialPostComment;
use crate::storage::StoredSocialPostMedia;
use crate::user_for_email;
use crate::{escape_html, escape_html_attr, profile_has_pet, AppState, UserProfile};
use serde::{Deserialize, Serialize};

pub const MAX_SOCIAL_POSTS: usize = 60;
pub const MAX_SOCIAL_VIDEO_SECONDS: f32 = 10.0;
pub const MAX_SOCIAL_PHOTOS_PER_POST: usize = 10;
pub const MAX_SOCIAL_COMMENT_LEN: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPostUpvoteResponse {
    pub ok: bool,
    pub post_id: String,
    pub upvotes: u32,
    pub viewer_upvoted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialCommentUpvoteResponse {
    pub ok: bool,
    pub comment_id: String,
    pub upvotes: u32,
    pub viewer_upvoted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPostCommentResponse {
    pub ok: bool,
    pub post_id: String,
    pub comment: Option<SocialPostCommentItem>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPostCommentItem {
    pub id: String,
    pub author_username: String,
    pub body: String,
    pub created_at: u64,
    pub upvotes: u32,
    pub viewer_upvoted: bool,
    pub viewer_owns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPostCommentDeleteResponse {
    pub ok: bool,
    pub comment_id: String,
    pub post_id: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialPostCommentDeleteForm {
    pub comment_id: String,
    pub post_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialPostUpvoteForm {
    pub post_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialCommentUpvoteForm {
    pub comment_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialPostCommentForm {
    pub post_id: String,
    pub body: String,
}

fn sort_posts_by_upvotes(posts: &mut [StoredSocialPost]) {
    posts.sort_by(|left, right| {
        right
            .upvotes
            .cmp(&left.upvotes)
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
}

fn sort_posts_by_recent(posts: &mut [StoredSocialPost]) {
    posts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
}

fn comment_to_item(comment: &StoredSocialPostComment, viewer_email: &str) -> SocialPostCommentItem {
    SocialPostCommentItem {
        id: comment.id.clone(),
        author_username: comment.author_username.clone(),
        body: comment.body.clone(),
        created_at: comment.created_at,
        upvotes: comment.upvotes,
        viewer_upvoted: comment.viewer_upvoted,
        viewer_owns: comment.user_id.eq_ignore_ascii_case(viewer_email),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialPostsView {
    Friends,
    All,
}

pub fn normalize_posts_view(value: Option<&str>) -> SocialPostsView {
    match value.map(str::trim).filter(|part| !part.is_empty()) {
        Some("all") => SocialPostsView::All,
        _ => SocialPostsView::Friends,
    }
}

pub fn can_view_social_post(post: &StoredSocialPost, viewer_email: &str) -> bool {
    !post.is_private || post.user_id.eq_ignore_ascii_case(viewer_email)
}

pub fn own_profile_tab_url() -> &'static str {
    "/home?tab=profile"
}

pub fn parent_profile_url(username: &str) -> String {
    format!(
        "/home?tab=profile&parent={}",
        urlencoding::encode(username.trim())
    )
}

pub fn render_parent_profile_link(username: &str, label: Option<&str>) -> String {
    let username = username.trim();
    if username.is_empty() {
        return escape_html(label.unwrap_or("Cat parent"));
    }
    let text = escape_html(label.unwrap_or(username));
    format!(
        r#"<a href="{url}" class="parent-profile-link">{text}</a>"#,
        url = escape_html_attr(&parent_profile_url(username)),
        text = text,
    )
}

pub fn resolve_parent_email(state: &AppState, username: &str) -> Option<String> {
    let username = username.trim();
    if username.is_empty() {
        return None;
    }
    state
        .storage
        .email_for_username(username)
        .ok()
        .flatten()
        .map(|email| sharing::normalize_email(&email))
}

pub fn can_view_parent_profile(state: &AppState, viewer_email: &str, subject_email: &str) -> bool {
    let viewer_email = sharing::normalize_email(viewer_email);
    let subject_email = sharing::normalize_email(subject_email);
    if viewer_email == subject_email {
        return true;
    }
    if sharing::friend_relation(state, &viewer_email, &subject_email)
        == sharing::FriendRelation::Friends
    {
        return true;
    }
    state
        .storage
        .load_profile(&subject_email)
        .ok()
        .flatten()
        .is_some_and(|profile| profile.community_visible && profile_has_pet(&profile))
}

fn format_timestamp(created_at: u64) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(created_at as i64, 0)
        .map(|value| value.format("%b %d, %Y · %I:%M %p").to_string())
        .unwrap_or_else(|| "Recently".to_string())
}

fn community_visible_emails(state: &AppState) -> Vec<String> {
    state
        .storage
        .list_profile_emails()
        .unwrap_or_default()
        .into_iter()
        .filter(|email| {
            state
                .storage
                .load_profile(email)
                .ok()
                .flatten()
                .is_some_and(|profile| profile.community_visible && profile_has_pet(&profile))
        })
        .collect()
}

pub fn collect_social_posts(
    state: &AppState,
    viewer_email: &str,
    view: SocialPostsView,
) -> Vec<StoredSocialPost> {
    let viewer = Some(viewer_email);
    let mut posts = match view {
        SocialPostsView::Friends => {
            let mut authors = sharing::accepted_friend_emails(state, viewer_email);
            authors.push(viewer_email.to_string());
            state
                .storage
                .list_social_posts_from_users_with_engagement(&authors, MAX_SOCIAL_POSTS, viewer)
                .unwrap_or_default()
                .into_iter()
                .filter(|post| {
                    can_view_social_post(post, viewer_email)
                        && post.post_kind != "pet_id"
                        && !sharing::users_block_each_other(state, viewer_email, &post.user_id)
                })
                .collect::<Vec<_>>()
        }
        SocialPostsView::All => {
            let visible = community_visible_emails(state);
            state
                .storage
                .list_social_posts_with_engagement(MAX_SOCIAL_POSTS, viewer)
                .unwrap_or_default()
                .into_iter()
                .filter(|post| {
                    can_view_social_post(post, viewer_email)
                        && post.post_kind != "pet_id"
                        && !sharing::users_block_each_other(state, viewer_email, &post.user_id)
                        && visible
                            .iter()
                            .any(|email| email.eq_ignore_ascii_case(&post.user_id))
                })
                .collect::<Vec<_>>()
        }
    };

    match view {
        SocialPostsView::All => sort_posts_by_upvotes(&mut posts),
        SocialPostsView::Friends => sort_posts_by_recent(&mut posts),
    }
    posts
}

pub fn collect_parent_profile_posts(
    state: &AppState,
    subject_email: &str,
    viewer_email: &str,
) -> Vec<StoredSocialPost> {
    let mut posts = state
        .storage
        .list_social_posts_from_users_with_engagement(
            &[subject_email.to_string()],
            MAX_SOCIAL_POSTS,
            Some(viewer_email),
        )
        .unwrap_or_default()
        .into_iter()
        .filter(|post| {
            can_view_social_post(post, viewer_email)
                && !sharing::users_block_each_other(state, viewer_email, &post.user_id)
        })
        .collect::<Vec<_>>();
    sort_posts_by_upvotes(&mut posts);
    posts
}

pub fn toggle_post_upvote(
    state: &AppState,
    viewer_email: &str,
    post_id: &str,
    created_at: u64,
) -> Result<SocialPostUpvoteResponse, StorageError> {
    let Some(post) = state
        .storage
        .get_social_post_by_id(post_id, Some(viewer_email))?
    else {
        return Err(StorageError::InvalidInput("post not found".into()));
    };
    if !can_view_social_post(&post, viewer_email) {
        return Err(StorageError::InvalidInput("post not found".into()));
    }

    let summary = state
        .storage
        .toggle_social_post_upvote(post_id, viewer_email, created_at)?;
    Ok(SocialPostUpvoteResponse {
        ok: true,
        post_id: post_id.to_string(),
        upvotes: summary.upvotes,
        viewer_upvoted: summary.viewer_upvoted,
        error: None,
    })
}

pub fn toggle_comment_upvote(
    state: &AppState,
    viewer_email: &str,
    comment_id: &str,
    created_at: u64,
) -> Result<SocialCommentUpvoteResponse, StorageError> {
    let Some(post_id) = state.storage.get_social_post_id_for_comment(comment_id)? else {
        return Err(StorageError::InvalidInput("comment not found".into()));
    };
    let Some(post) = state
        .storage
        .get_social_post_by_id(&post_id, Some(viewer_email))?
    else {
        return Err(StorageError::InvalidInput("comment not found".into()));
    };
    if !can_view_social_post(&post, viewer_email) {
        return Err(StorageError::InvalidInput("comment not found".into()));
    }

    let summary =
        state
            .storage
            .toggle_social_comment_upvote(comment_id, viewer_email, created_at)?;
    Ok(SocialCommentUpvoteResponse {
        ok: true,
        comment_id: comment_id.to_string(),
        upvotes: summary.upvotes,
        viewer_upvoted: summary.viewer_upvoted,
        error: None,
    })
}

pub fn add_post_comment(
    state: &AppState,
    viewer_email: &str,
    post_id: &str,
    body: &str,
    created_at: u64,
) -> Result<SocialPostCommentResponse, StorageError> {
    let Some(post) = state
        .storage
        .get_social_post_by_id(post_id, Some(viewer_email))?
    else {
        return Err(StorageError::InvalidInput("post not found".into()));
    };
    if !can_view_social_post(&post, viewer_email) {
        return Err(StorageError::InvalidInput("post not found".into()));
    }

    let username = author_username_for_email(state, viewer_email);
    let comment = state.storage.create_social_post_comment(
        post_id,
        viewer_email,
        &username,
        body,
        created_at,
    )?;
    Ok(SocialPostCommentResponse {
        ok: true,
        post_id: post_id.to_string(),
        comment: Some(comment_to_item(&comment, viewer_email)),
        error: None,
    })
}

fn render_social_post_upvote_controls(post: &StoredSocialPost) -> String {
    let active_class = if post.viewer_upvoted {
        " social-post-upvote-btn is-active"
    } else {
        " social-post-upvote-btn"
    };
    let pressed = if post.viewer_upvoted {
        r#" aria-pressed="true""#
    } else {
        ""
    };
    let label = if post.viewer_upvoted {
        format!("💖 {}", post.upvotes)
    } else {
        format!("🐾 {}", post.upvotes)
    };
    format!(
        r#"<button type="button" class="{class}" data-post-upvote="{id}"{pressed} aria-label="Love this post">{label}</button>"#,
        class = active_class.trim(),
        id = escape_html_attr(&post.id),
        pressed = pressed,
        label = label,
    )
}

pub fn delete_post_comment(
    state: &AppState,
    viewer_email: &str,
    comment_id: &str,
    post_id: &str,
) -> Result<SocialPostCommentDeleteResponse, StorageError> {
    let Some(stored_post_id) = state.storage.get_social_post_id_for_comment(comment_id)? else {
        return Ok(SocialPostCommentDeleteResponse {
            ok: false,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: Some("not_found".into()),
        });
    };
    if stored_post_id != post_id {
        return Ok(SocialPostCommentDeleteResponse {
            ok: false,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: Some("not_found".into()),
        });
    }

    let Some(post) = state
        .storage
        .get_social_post_by_id(post_id, Some(viewer_email))?
    else {
        return Ok(SocialPostCommentDeleteResponse {
            ok: false,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: Some("not_found".into()),
        });
    };
    if !can_view_social_post(&post, viewer_email) {
        return Ok(SocialPostCommentDeleteResponse {
            ok: false,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: Some("not_found".into()),
        });
    }

    match state
        .storage
        .delete_social_post_comment_owned(comment_id, viewer_email)?
    {
        crate::storage::ForumDeleteOutcome::Deleted => Ok(SocialPostCommentDeleteResponse {
            ok: true,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: None,
        }),
        crate::storage::ForumDeleteOutcome::NotAuthorized => Ok(SocialPostCommentDeleteResponse {
            ok: false,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: Some("delete_denied".into()),
        }),
        crate::storage::ForumDeleteOutcome::NotFound => Ok(SocialPostCommentDeleteResponse {
            ok: false,
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            error: Some("not_found".into()),
        }),
    }
}

pub fn render_comment_paw_button() -> &'static str {
    r#"<button type="button" class="comment-paw-btn" aria-label="Comment options" aria-haspopup="menu" aria-expanded="false" title="Comment options">🐾</button>"#
}

fn render_social_post_comment_item(
    comment: &StoredSocialPostComment,
    viewer_email: Option<&str>,
) -> String {
    let active_class = if comment.viewer_upvoted {
        " social-comment-upvote-btn is-active"
    } else {
        " social-comment-upvote-btn"
    };
    let pressed = if comment.viewer_upvoted {
        r#" aria-pressed="true""#
    } else {
        ""
    };
    let is_mine = viewer_email.is_some_and(|email| comment.user_id.eq_ignore_ascii_case(email));
    let mine_class = if is_mine { " is-mine" } else { "" };
    let paw = if is_mine {
        render_comment_paw_button()
    } else {
        ""
    };
    format!(
        r#"<li class="social-post-comment comment-paw-wrap{mine_class}" data-comment-id="{id}" data-post-id="{post_id}" data-comment-delete-kind="social-post">
  <div class="comment-paw-body social-post-comment-main">
    <p class="social-post-comment-meta"><strong>{author}</strong> · {when}</p>
    <p class="social-post-comment-body">{body}</p>
    {paw}
  </div>
  <button type="button" class="{class}" data-comment-upvote="{id}"{pressed} aria-label="Upvote comment">▲ {upvotes}</button>
</li>"#,
        mine_class = mine_class,
        id = escape_html_attr(&comment.id),
        post_id = escape_html_attr(&comment.post_id),
        author = escape_html(&comment.author_username),
        when = escape_html(&format_timestamp(comment.created_at)),
        body = escape_html(&comment.body),
        paw = paw,
        class = active_class.trim(),
        pressed = pressed,
        upvotes = comment.upvotes,
    )
}

fn render_social_post_comments(post: &StoredSocialPost, viewer_email: Option<&str>) -> String {
    if post.comments.is_empty() {
        return String::new();
    }
    let items = post
        .comments
        .iter()
        .map(|comment| render_social_post_comment_item(comment, viewer_email))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<ul class="social-post-comment-list" aria-label="Comments">{items}</ul>"#,
        items = items,
    )
}

fn render_social_post_comment_form(post_id: &str) -> String {
    let field_id = format!("social-post-comment-{post_id}");
    format!(
        r#"<form class="social-post-comment-form login-form" action="/home/social/post/comment" method="post" data-post-comment-form="{post_id}">
  <input type="hidden" name="post_id" value="{post_id}" />
  <label class="visually-hidden" for="{field_id}">Add a comment</label>
  <textarea id="{field_id}" name="body" rows="2" maxlength="{max_len}" placeholder="Share some love for this kitty…" data-emoji-picker required></textarea>
  <button type="submit" class="social-post-comment-submit download-btn">Send 🐾</button>
</form>"#,
        post_id = escape_html_attr(post_id),
        field_id = escape_html_attr(&field_id),
        max_len = MAX_SOCIAL_COMMENT_LEN,
    )
}

fn render_social_post_comments_section(
    post: &StoredSocialPost,
    viewer_email: Option<&str>,
) -> String {
    let summary = if post.comments.is_empty() {
        "💬 Comments".to_string()
    } else if post.comments.len() == 1 {
        "💬 1 comment".to_string()
    } else {
        format!("💬 {} comments", post.comments.len())
    };
    let comments = render_social_post_comments(post, viewer_email);
    let empty_state = if post.comments.is_empty() {
        r#"<p class="social-post-comments-empty">No comments yet — be the first to say something sweet! 🐾</p>"#
    } else {
        ""
    };

    format!(
        r#"<details class="social-post-comments-details">
  <summary class="social-post-comments-summary">
    <span class="social-post-comments-summary-text">{summary}</span>
    <span class="social-post-comments-chevron" aria-hidden="true">▾</span>
  </summary>
  <div class="social-post-comments-body">
    {empty_state}
    {comments}
    {comment_form}
  </div>
</details>"#,
        summary = escape_html(&summary),
        empty_state = empty_state,
        comments = comments,
        comment_form = render_social_post_comment_form(&post.id),
    )
}

fn render_social_post_media(post: &StoredSocialPost) -> String {
    if !post.media_items.is_empty() {
        if post.media_items.len() == 1 {
            return render_single_social_post_media(&post.media_items[0]);
        }
        let total = post.media_items.len();
        let photos = post
            .media_items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                format!(
                    r#"<button type="button" class="social-post-media-open" data-media-type="photo" data-media-url="{url}" aria-label="View photo {num} of {total} larger">
  <img class="social-post-photo" src="{url}" alt="Photo {num} of {total}" loading="lazy" />
</button>"#,
                    url = escape_html_attr(&item.media_url),
                    num = index + 1,
                    total = total,
                )
            })
            .collect::<Vec<_>>()
            .join("");
        return format!(
            r#"<div class="social-post-photo-grid" data-photo-count="{count}">{photos}</div>"#,
            count = total,
            photos = photos,
        );
    }

    let Some(url) = post.media_url.as_deref().filter(|value| !value.is_empty()) else {
        return String::new();
    };
    if post.media_type == "none" {
        return String::new();
    }
    render_single_social_post_media(&StoredSocialPostMedia {
        media_type: post.media_type.clone(),
        media_url: url.to_string(),
        video_duration: post.video_duration,
        sort_order: 0,
    })
}

fn render_single_social_post_media(item: &StoredSocialPostMedia) -> String {
    let url = escape_html_attr(&item.media_url);
    match item.media_type.as_str() {
        "photo" => format!(
            r#"<button type="button" class="social-post-media-open" data-media-type="photo" data-media-url="{url}" aria-label="View photo larger">
  <img class="social-post-photo" src="{url}" alt="" loading="lazy" />
</button>"#,
            url = url,
        ),
        "video" => {
            let duration = item.video_duration.unwrap_or(MAX_SOCIAL_VIDEO_SECONDS);
            format!(
                r#"<div class="social-post-video-shell">
  <video class="social-post-video" src="{url}" controls playsinline preload="metadata" data-max-duration="{duration}"></video>
  <button type="button" class="social-post-media-open social-post-media-expand" data-media-type="video" data-media-url="{url}" aria-label="View video larger">⛶</button>
  <p class="social-post-video-hint">Short clip · up to 10 seconds</p>
</div>"#,
                url = url,
                duration = duration,
            )
        }
        _ => String::new(),
    }
}

fn parse_wrapped_payload(post: &StoredSocialPost) -> Option<WrappedPayload> {
    if post.post_kind != "monthly_wrapped" {
        return None;
    }
    post.wrapped_payload
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok())
}

fn render_wrapped_collage(urls: &[String]) -> String {
    if urls.is_empty() {
        return r#"<p class="parent-wrapped-collage-empty">No photos this month — next month’s collage awaits! 📸</p>"#.to_string();
    }
    let tiles = urls
        .iter()
        .enumerate()
        .map(|(index, url)| {
            format!(
                r#"<button type="button" class="parent-wrapped-collage-tile social-post-media-open" data-media-type="photo" data-media-url="{url}" aria-label="View wrapped photo {num} of {total} larger">
  <img src="{url}" alt="" loading="lazy" />
</button>"#,
                url = escape_html_attr(url),
                num = index + 1,
                total = urls.len(),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<div class="parent-wrapped-collage" data-photo-count="{count}">{tiles}</div>"#,
        count = urls.len(),
        tiles = tiles,
    )
}

fn is_private_parent_profile_achievement_title(title: &str) -> bool {
    title.eq_ignore_ascii_case("WhiskerWatch Plus")
}

fn render_wrapped_achievements(payload: &WrappedPayload, public_view: bool) -> String {
    let achievements: Vec<_> = payload
        .achievements
        .iter()
        .filter(|item| !public_view || !is_private_parent_profile_achievement_title(&item.title))
        .collect();
    if achievements.is_empty() {
        return r#"<p class="parent-wrapped-achievements-empty">Keep caring daily to unlock badges for next month’s wrap-up.</p>"#.to_string();
    }
    let pills = achievements
        .iter()
        .map(|item| {
            format!(
                r#"<li class="parent-wrapped-achievement">
  <span class="parent-wrapped-achievement-badge" aria-hidden="true">{badge}</span>
  <span class="parent-wrapped-achievement-copy">
    <strong>{title}</strong>
    <span>{detail}</span>
  </span>
</li>"#,
                badge = escape_html(&item.badge),
                title = escape_html(&item.title),
                detail = escape_html(&item.detail),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<ul class="parent-wrapped-achievements">{pills}</ul>"#,
        pills = pills,
    )
}

fn parse_pet_id_payload(post: &StoredSocialPost) -> Option<PetIdPostPayload> {
    if post.post_kind != "pet_id" {
        return None;
    }
    post.wrapped_payload
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok())
}

fn render_pet_id_meta(payload: &PetIdPostPayload) -> String {
    let breed = payload.pet_breed.trim();
    let color = payload.pet_color.trim();
    let mut parts = Vec::new();
    if !breed.is_empty() {
        parts.push(escape_html(breed));
    }
    if !color.is_empty() {
        parts.push(escape_html(color));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!(
        r#"<p class="pet-id-post-meta">{parts}</p>"#,
        parts = parts.join(" · "),
    )
}

fn render_pet_id_stage(payload: &PetIdPostPayload) -> String {
    let display_name = {
        let name = payload.pet_name.trim();
        if name.is_empty() {
            "Cinder".to_string()
        } else {
            escape_html(name)
        }
    };
    let photo_layer = payload
        .pet_photo_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(|url| {
            format!(
                r#"<img class="cinder-pet-user-photo" src="{url}" alt="{display_name} profile photo" loading="lazy" />"#,
                url = escape_html_attr(url),
                display_name = display_name,
            )
        })
        .unwrap_or_default();
    let video_hint = if payload.has_video {
        r#"<p class="pet-id-post-video-hint">Playing video saved on My Pet 🎬</p>"#
    } else {
        ""
    };

    format!(
        r#"<div class="pet-cinder-stage pet-id-post-stage" data-cinder-stage="pet" data-pet-name="{display_name}" data-pet-id="{pet_id}">
  <p class="pet-cinder-stage-badge" aria-hidden="true">Official Pet ID · {slot_label}</p>
  <p class="cinder-pet-label">{display_name}</p>
  <div class="cinder-pet-image-wrap">
    <img class="cinder-pet-image" src="/cinderanimate.png" alt="{display_name} virtual pet" />
    {photo_layer}
  </div>
  {meta}
  {video_hint}
</div>"#,
        display_name = display_name,
        pet_id = escape_html_attr(&payload.pet_id),
        slot_label = escape_html(&payload.slot_label),
        photo_layer = photo_layer,
        meta = render_pet_id_meta(payload),
        video_hint = video_hint,
    )
}

fn render_pet_id_post_card(
    state: &AppState,
    viewer_email: &str,
    post: &StoredSocialPost,
    link_author: bool,
) -> String {
    let payload = parse_pet_id_payload(post).unwrap_or(PetIdPostPayload {
        pet_id: String::new(),
        pet_name: String::new(),
        pet_breed: String::new(),
        pet_color: String::new(),
        slot_label: "Pet ID".to_string(),
        pet_photo_url: None,
        has_video: false,
    });
    let when = escape_html(&format_timestamp(post.created_at));
    let author_block = if link_author {
        format!(
            r#"<a href="{url}" class="social-post-author-link">
  <strong class="social-post-author">{author}</strong>
  <time class="social-post-time" datetime="{created_at}">{when}</time>
</a>"#,
            url = escape_html_attr(&parent_profile_url(&post.author_username)),
            author = escape_html(&post.author_username),
            created_at = post.created_at,
            when = when,
        )
    } else {
        format!(
            r#"<div class="social-post-author-static">
  <strong class="social-post-author">{author}</strong>
  <time class="social-post-time" datetime="{created_at}">{when}</time>
</div>"#,
            author = escape_html(&post.author_username),
            created_at = post.created_at,
            when = when,
        )
    };
    let friend_action = sharing::render_friend_add_control(state, viewer_email, &post.user_id);
    let block_action = sharing::render_block_control(state, viewer_email, &post.user_id);
    let delete_form = if post.user_id.eq_ignore_ascii_case(viewer_email) {
        format!(
            r#"<form class="social-post-delete-form" action="/home/social/post/delete" method="post" data-confirm-kind="delete-social-post">
  <input type="hidden" name="post_id" value="{id}" />
  <input type="hidden" name="posts_view" value="" data-social-posts-view />
  <button type="submit" class="social-post-delete-btn" aria-label="Remove post">Remove 🐾</button>
</form>"#,
            id = escape_html_attr(&post.id),
        )
    } else {
        String::new()
    };
    let avatar_initial = post
        .author_username
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "🐱".to_string());
    let avatar = format!(
        r#"<span class="social-post-avatar" aria-hidden="true">{initial}</span>"#,
        initial = escape_html(&avatar_initial),
    );
    let engagement = format!(
        r#"<div class="social-post-engagement">
  <div class="social-post-engagement-bar">
    {upvote}
    {comments}
  </div>
</div>"#,
        upvote = render_social_post_upvote_controls(post),
        comments = render_social_post_comments_section(post, Some(viewer_email)),
    );

    format!(
        r#"<article class="social-post-card pet-id-post-card" data-post-id="{id}">
  <header class="social-post-header">
    <div class="social-post-author-block">{avatar}{author_block}<span class="pet-id-post-badge">Pet ID</span></div>
    <div class="social-post-header-actions">{friend_action}{block_action}{delete_form}</div>
  </header>
  {stage}
  <p class="social-post-caption pet-id-post-caption">{caption}</p>
  {engagement}
</article>"#,
        id = escape_html_attr(&post.id),
        avatar = avatar,
        author_block = author_block,
        friend_action = friend_action,
        block_action = block_action,
        delete_form = delete_form,
        stage = render_pet_id_stage(&payload),
        caption = escape_html(&post.body),
        engagement = engagement,
    )
}

fn render_monthly_wrapped_post_card(
    state: &AppState,
    viewer_email: &str,
    post: &StoredSocialPost,
    link_author: bool,
) -> String {
    let payload = parse_wrapped_payload(post).unwrap_or(WrappedPayload {
        year: post.wrapped_year.unwrap_or(0),
        month: post.wrapped_month.unwrap_or(0),
        month_label: "Parent Wrapped".to_string(),
        parent_grade: "?".to_string(),
        parent_score: 0,
        parent_level: 0,
        post_count: 0,
        total_upvotes: 0,
        achievements: Vec::new(),
        collage_urls: Vec::new(),
    });
    let when = escape_html(&format_timestamp(post.created_at));
    let author_block = if link_author {
        format!(
            r#"<a href="{url}" class="social-post-author-link">
  <strong class="social-post-author">{author}</strong>
  <time class="social-post-time" datetime="{created_at}">{when}</time>
</a>"#,
            url = escape_html_attr(&parent_profile_url(&post.author_username)),
            author = escape_html(&post.author_username),
            created_at = post.created_at,
            when = when,
        )
    } else {
        format!(
            r#"<div class="social-post-author-static">
  <strong class="social-post-author">{author}</strong>
  <time class="social-post-time" datetime="{created_at}">{when}</time>
</div>"#,
            author = escape_html(&post.author_username),
            created_at = post.created_at,
            when = when,
        )
    };
    let friend_action = sharing::render_friend_add_control(state, viewer_email, &post.user_id);
    let block_action = sharing::render_block_control(state, viewer_email, &post.user_id);
    let delete_form = if post.user_id.eq_ignore_ascii_case(viewer_email) {
        format!(
            r#"<form class="social-post-delete-form" action="/home/social/post/delete" method="post" data-confirm-kind="delete-social-post">
  <input type="hidden" name="post_id" value="{id}" />
  <input type="hidden" name="posts_view" value="" data-social-posts-view />
  <button type="submit" class="social-post-delete-btn" aria-label="Remove post">Remove 🐾</button>
</form>"#,
            id = escape_html_attr(&post.id),
        )
    } else {
        String::new()
    };
    let avatar_initial = post
        .author_username
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "🐱".to_string());
    let avatar = format!(
        r#"<span class="social-post-avatar" aria-hidden="true">{initial}</span>"#,
        initial = escape_html(&avatar_initial),
    );
    let engagement = format!(
        r#"<div class="social-post-engagement">
  <div class="social-post-engagement-bar">
    {upvote}
    {comments}
  </div>
</div>"#,
        upvote = render_social_post_upvote_controls(post),
        comments = render_social_post_comments_section(post, Some(viewer_email)),
    );

    format!(
        r#"<article class="social-post-card parent-wrapped-card" data-post-id="{id}">
  <header class="social-post-header">
    <div class="social-post-author-block">{avatar}{author_block}<span class="parent-wrapped-badge">Monthly Wrapped</span></div>
    <div class="social-post-header-actions">{friend_action}{block_action}{delete_form}</div>
  </header>
  <div class="parent-wrapped-body">
    <div class="parent-wrapped-hero">
      <p class="parent-wrapped-kicker">{month_label}</p>
      <p class="parent-wrapped-grade" aria-label="Parent grade {grade}">{grade}</p>
      <p class="parent-wrapped-score">{score} parent points</p>
    </div>
    <ul class="parent-wrapped-stats">
      <li><span>Level</span><strong>{level}</strong></li>
      <li><span>Posts</span><strong>{posts}</strong></li>
      <li><span>Loves</span><strong>{loves}</strong></li>
      <li><span>Badges</span><strong>{badges}</strong></li>
    </ul>
    <section class="parent-wrapped-section">
      <h3 class="parent-wrapped-section-title">Achievements</h3>
      {achievements}
    </section>
    <section class="parent-wrapped-section">
      <h3 class="parent-wrapped-section-title">Your month in photos</h3>
      {collage}
    </section>
  </div>
  <p class="social-post-caption parent-wrapped-caption">{caption}</p>
  {engagement}
</article>"#,
        id = escape_html_attr(&post.id),
        avatar = avatar,
        author_block = author_block,
        friend_action = friend_action,
        block_action = block_action,
        delete_form = delete_form,
        month_label = escape_html(&payload.month_label),
        grade = escape_html(&payload.parent_grade),
        score = payload.parent_score,
        level = payload.parent_level,
        posts = payload.post_count,
        loves = payload.total_upvotes,
        badges = payload
            .achievements
            .iter()
            .filter(|item| {
                post.user_id.eq_ignore_ascii_case(viewer_email)
                    || !is_private_parent_profile_achievement_title(&item.title)
            })
            .count(),
        achievements = render_wrapped_achievements(
            &payload,
            !post.user_id.eq_ignore_ascii_case(viewer_email),
        ),
        collage = render_wrapped_collage(&payload.collage_urls),
        caption = escape_html(&post.body),
        engagement = engagement,
    )
}

pub fn render_social_post_card(
    state: &AppState,
    viewer_email: &str,
    post: &StoredSocialPost,
    link_author: bool,
) -> String {
    if post.post_kind == "monthly_wrapped" {
        return render_monthly_wrapped_post_card(state, viewer_email, post, link_author);
    }
    if post.post_kind == "pet_id" {
        return render_pet_id_post_card(state, viewer_email, post, link_author);
    }

    let when = escape_html(&format_timestamp(post.created_at));
    let author_block = if link_author {
        format!(
            r#"<a href="{url}" class="social-post-author-link">
  <strong class="social-post-author">{author}</strong>
  <time class="social-post-time" datetime="{created_at}">{when}</time>
</a>"#,
            url = escape_html_attr(&parent_profile_url(&post.author_username)),
            author = escape_html(&post.author_username),
            created_at = post.created_at,
            when = when,
        )
    } else {
        format!(
            r#"<div class="social-post-author-static">
  <strong class="social-post-author">{author}</strong>
  <time class="social-post-time" datetime="{created_at}">{when}</time>
</div>"#,
            author = escape_html(&post.author_username),
            created_at = post.created_at,
            when = when,
        )
    };
    let caption = if post.body.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="social-post-caption">{body}</p>"#,
            body = escape_html(&post.body),
        )
    };
    let media = render_social_post_media(post);
    let media_block = if media.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="social-post-media">{media}</div>"#,
            media = media
        )
    };
    let friend_action = sharing::render_friend_add_control(state, viewer_email, &post.user_id);
    let block_action = sharing::render_block_control(state, viewer_email, &post.user_id);
    let private_badge = if post.is_private && post.user_id.eq_ignore_ascii_case(viewer_email) {
        r#"<span class="social-post-private-badge">Private</span>"#
    } else {
        ""
    };
    let delete_form = if post.user_id.eq_ignore_ascii_case(viewer_email) {
        format!(
            r#"<form class="social-post-delete-form" action="/home/social/post/delete" method="post" data-confirm-kind="delete-social-post">
  <input type="hidden" name="post_id" value="{id}" />
  <input type="hidden" name="posts_view" value="" data-social-posts-view />
  <button type="submit" class="social-post-delete-btn" aria-label="Remove post">Remove 🐾</button>
</form>"#,
            id = escape_html_attr(&post.id),
        )
    } else {
        String::new()
    };

    let avatar_initial = post
        .author_username
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "🐱".to_string());
    let avatar = format!(
        r#"<span class="social-post-avatar" aria-hidden="true">{initial}</span>"#,
        initial = escape_html(&avatar_initial),
    );

    let engagement = format!(
        r#"<div class="social-post-engagement">
  <div class="social-post-engagement-bar">
    {upvote}
    {comments}
  </div>
</div>"#,
        upvote = render_social_post_upvote_controls(post),
        comments = render_social_post_comments_section(post, Some(viewer_email)),
    );

    format!(
        r#"<article class="social-post-card" data-post-id="{id}">
  <header class="social-post-header">
    <div class="social-post-author-block">{avatar}{author_block}{private_badge}</div>
    <div class="social-post-header-actions">{friend_action}{block_action}{delete_form}</div>
  </header>
  {media_block}
  {caption}
  {engagement}
</article>"#,
        id = escape_html_attr(&post.id),
        avatar = avatar,
        author_block = author_block,
        private_badge = private_badge,
        friend_action = friend_action,
        block_action = block_action,
        delete_form = delete_form,
        media_block = media_block,
        caption = caption,
        engagement = engagement,
    )
}

pub fn render_social_post_form(instance: &str) -> String {
    let instance = instance.trim();
    let form_id = format!("social-post-form-{instance}");
    let body_id = format!("social_post_body_{instance}");
    let media_id = format!("social_post_media_{instance}");
    let media_cta_id = format!("social_post_media_cta_{instance}");
    let preview_id = format!("social-post-media-preview-{instance}");
    let duration_id = format!("social_post_video_duration_{instance}");
    let submit_id = format!("social_post_submit_{instance}");

    format!(
        r#"<details class="dashboard-card social-post-compose-card">
  <summary class="social-post-compose-summary">
    <span class="social-post-compose-summary-text">Share a photo or video</span>
  </summary>
  <div class="social-post-compose-body">
    <p class="field-hint">Post up to {max_photos} cat photos or one video up to {max_seconds} seconds for your friends — or share with the whole community in All posts. Posts also appear on your profile.</p>
    <form class="login-form social-post-form" id="{form_id}" data-social-compose="{instance}" action="/home/social/post" method="post" enctype="multipart/form-data">
      <label for="{body_id}">Caption (optional)</label>
      <textarea id="{body_id}" name="body" rows="3" maxlength="2000" placeholder="What is your kitty up to?" data-emoji-picker></textarea>
      <label class="social-post-private-option">
        <input type="checkbox" name="private" value="1" />
        <span>Private post — only visible on your profile, not in friends or community feeds</span>
      </label>
      <fieldset class="social-post-media-fieldset">
        <legend>Photos or video</legend>
        <div class="pet-photo-upload social-post-media-upload">
          <input id="{media_id}" name="media" type="file" class="pet-photo-input social-post-media-input" accept="image/jpeg,image/png,image/webp,image/heic,image/heif,video/mp4,video/webm,video/quicktime,.heic,.heif" multiple required />
          <label for="{media_id}" class="pet-photo-paw-btn" aria-label="Choose a photo or video to share">
            <span class="pet-photo-paw-icon" aria-hidden="true">📸</span>
          </label>
          <p class="pet-photo-upload-cta social-post-media-cta" id="{media_cta_id}">Tap to pick up to {max_photos} photos or one video 🐾</p>
        </div>
        <div class="social-post-media-preview-shell" data-social-preview-shell hidden>
          <p class="social-post-media-preview-label">Preview before posting ✨</p>
          <div id="{preview_id}" class="social-post-media-preview pet-photo-preview" aria-live="polite"></div>
        </div>
        <p class="field-hint">Pick up to {max_photos} photos or one video — you will see a preview here to crop or trim before posting. Videos must be {max_seconds} seconds or shorter.</p>
      </fieldset>
      <input type="hidden" name="video_duration" id="{duration_id}" value="" />
      <button type="submit" class="download-btn login-submit" id="{submit_id}">Post</button>
    </form>
  </div>
</details>"#,
        form_id = form_id,
        body_id = body_id,
        media_id = media_id,
        media_cta_id = media_cta_id,
        preview_id = preview_id,
        duration_id = duration_id,
        submit_id = submit_id,
        instance = escape_html_attr(instance),
        max_seconds = MAX_SOCIAL_VIDEO_SECONDS as u32,
        max_photos = MAX_SOCIAL_PHOTOS_PER_POST,
    )
}

pub fn render_social_posts_view_toggle(view: SocialPostsView) -> String {
    let friends_active = if view == SocialPostsView::Friends {
        " active"
    } else {
        ""
    };
    let all_active = if view == SocialPostsView::All {
        " active"
    } else {
        ""
    };
    format!(
        r#"<nav class="social-posts-view-toggle" aria-label="Post feed view">
  <a class="social-posts-view-btn{friends_active}" href="/home?tab=forum&amp;community=friends&amp;posts_view=friends">Friends</a>
  <a class="social-posts-view-btn{all_active}" href="/home?tab=forum&amp;community=friends&amp;posts_view=all">All posts</a>
</nav>"#,
        friends_active = friends_active,
        all_active = all_active,
    )
}

pub fn render_friends_posts_section(
    state: &AppState,
    viewer_email: &str,
    profile: &UserProfile,
    view: SocialPostsView,
) -> String {
    let posts = collect_social_posts(state, viewer_email, view);
    let view_toggle = render_social_posts_view_toggle(view);
    let compose = if profile_has_pet(profile) {
        render_social_post_form("community")
    } else {
        r#"<p class="field-hint">Set up your cat on the My Pet tab before sharing photos and videos.</p>"#
            .to_string()
    };

    let feed = if posts.is_empty() {
        let empty_message = match view {
            SocialPostsView::Friends => {
                if sharing::accepted_friend_emails(state, viewer_email).is_empty() {
                    r#"<div class="community-friends-empty">
  <p class="community-friends-empty-emoji" aria-hidden="true">🐾💕</p>
  <p class="community-friends-empty-title">No friend posts yet</p>
  <p class="community-friends-empty-copy">Connect with other cat parents on the Friends tab first — then their photos and videos will show up here.</p>
  <a href="/home?tab=friends" class="community-friends-cta">Find cat parents on Friends tab</a>
  <p class="community-friends-empty-alt"><a class="community-friends-secondary-link" href="/home?tab=forum&amp;community=friends&amp;posts_view=all">Browse all community posts</a></p>
</div>"#
                } else {
                    "Your friends haven't posted yet — share a photo or video to get things started!"
                }
            }
            SocialPostsView::All => {
                "No community posts yet. Be the first to share a cat photo or short video!"
            }
        };
        format!(r#"<p class="social-posts-empty">{empty_message}</p>"#)
    } else {
        format!(
            r#"<div class="social-posts-feed">{cards}</div>"#,
            cards = posts
                .iter()
                .map(|post| render_social_post_card(state, viewer_email, post, true))
                .collect::<Vec<_>>()
                .join(""),
        )
    };

    let intro = match view {
        SocialPostsView::Friends => {
            "Photos and short videos from your WhiskerWatch friends. Tap a name to visit their profile."
        }
        SocialPostsView::All => {
            "Photos and videos from cat parents across WhiskerWatch — most upvoted posts rise to the top. Tap a name to visit their profile."
        }
    };

    format!(
        r#"<section class="community-section community-section-friends" id="community-friends-panel">
  <div class="community-section-header">
    <h2>Posts</h2>
    <p class="field-hint">{intro}</p>
  </div>
  {view_toggle}
  {compose}
  {feed}
</section>"#,
        intro = intro,
        view_toggle = view_toggle,
        compose = compose,
        feed = feed,
    )
}

pub fn render_parent_profile_page(
    state: &AppState,
    viewer_email: &str,
    username: &str,
    show_back_link: bool,
) -> String {
    let Some(subject_email) = resolve_parent_email(state, username) else {
        return r#"<h1>Profile</h1>
<p class="panel-intro">That cat parent could not be found.</p>
<p><a href="/home?tab=forum&amp;community=friends&amp;posts_view=all" class="download-btn">Back to posts</a></p>"#
            .to_string();
    };

    if sharing::users_block_each_other(state, viewer_email, &subject_email) {
        return format!(
            r#"<h1>Profile</h1>
<p class="panel-intro">This profile isn't available.</p>
<p><a href="/home?tab=forum&amp;community=friends&amp;posts_view=all" class="download-btn">Back to posts</a></p>"#,
        );
    }

    if !can_view_parent_profile(state, viewer_email, &subject_email) {
        return format!(
            r#"<h1>Profile</h1>
<p class="panel-intro">{name}'s profile is private. Connect as friends or browse community cats to discover more parents.</p>
<p><a href="/home?tab=forum&amp;community=friends&amp;posts_view=all" class="download-btn">Back to posts</a></p>"#,
            name = escape_html(username.trim()),
        );
    }

    let subject_profile = state.storage.load_profile(&subject_email).ok().flatten();
    let display_username = user_for_email(state, &subject_email)
        .map(|user| user.username)
        .unwrap_or_else(|| username.trim().to_string());
    let photo = sharing::user_profile_photo_src(state, &subject_email);
    let photo_alt = subject_profile
        .as_ref()
        .map(crate::display_pet_name)
        .filter(|name| !name.eq_ignore_ascii_case("no pet yet"))
        .unwrap_or_else(|| "Cat profile photo".to_string());
    let pet_name = subject_profile
        .as_ref()
        .map(|profile| profile.pet_name.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Their cat".to_string());
    let pet_breed = subject_profile
        .as_ref()
        .map(|profile| profile.pet_breed.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let is_self = subject_email.eq_ignore_ascii_case(viewer_email);
    let show_personal = sharing::can_see_personal_pet_details(state, viewer_email, &subject_email);
    let interact_menu = if is_self {
        String::new()
    } else {
        sharing::render_profile_interact_menu(
            state,
            viewer_email,
            &subject_email,
            username.trim(),
            true,
        )
    };

    let pet_line = if show_personal {
        format!(
            r#"<p class="parent-profile-pet">Caring for <strong>{pet_name}</strong></p>"#,
            pet_name = escape_html(&pet_name),
        )
    } else {
        r#"<p class="parent-profile-pet">WhiskerWatch cat parent</p>"#.to_string()
    };

    let breed_line = if show_personal {
        pet_breed
            .as_ref()
            .map(|breed| {
                format!(
                    r#"<p class="parent-profile-breed">{breed}</p>"#,
                    breed = escape_html(breed),
                )
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    let achievements_html = subject_profile
        .as_ref()
        .map(|profile| crate::achievements::render_parent_profile_achievements(profile, is_self))
        .unwrap_or_default();
    let friends_html =
        sharing::render_parent_profile_friends_section(state, &subject_email, is_self);

    let posts = collect_parent_profile_posts(state, &subject_email, viewer_email);
    let posts_html = if posts.is_empty() {
        if is_self {
            format!(
                r#"<p class="social-posts-empty">You haven't posted yet. <a href="/home?tab=forum&amp;community=friends">Share your first photo or video</a> — it will show up here on your profile.</p>"#
            )
        } else {
            r#"<p class="social-posts-empty">No posts yet from this cat parent.</p>"#.to_string()
        }
    } else {
        format!(
            r#"<div class="social-posts-feed parent-profile-posts">{cards}</div>"#,
            cards = posts
                .iter()
                .map(|post| render_social_post_card(state, viewer_email, post, false))
                .collect::<Vec<_>>()
                .join(""),
        )
    };

    let compose = if is_self && subject_profile.as_ref().is_some_and(profile_has_pet) {
        render_social_post_form("profile")
    } else {
        String::new()
    };

    let back_link = if show_back_link {
        r#"<p class="parent-profile-back"><a href="/home?tab=forum&amp;community=friends&amp;posts_view=all">← Back to posts</a></p>"#
            .to_string()
    } else {
        String::new()
    };

    let page_heading = if is_self && !show_back_link {
        r#"<h1 class="parent-profile-page-title">Your profile</h1>"#.to_string()
    } else {
        String::new()
    };

    format!(
        r#"<div class="parent-profile-page">
  {page_heading}
  {back_link}
  <header class="parent-profile-header dashboard-card">
    {interact_menu}
    <img class="parent-profile-photo" src="{photo}" alt="{photo_alt}" width="96" height="96" />
    <div class="parent-profile-meta">
      <h1 class="parent-profile-name">{username}</h1>
      {pet_line}
      {breed_line}
    </div>
  </header>
  {achievements_html}
  {friends_html}
  {compose}
  <section class="parent-profile-posts-section">
    <h2 class="parent-profile-posts-title">Posts</h2>
    <p class="field-hint parent-profile-posts-intro">Most upvoted posts appear first on this profile.</p>
    {posts_html}
  </section>
</div>"#,
        page_heading = page_heading,
        back_link = back_link,
        photo = escape_html_attr(&photo),
        photo_alt = escape_html_attr(&photo_alt),
        username = escape_html(&display_username),
        pet_line = pet_line,
        breed_line = breed_line,
        achievements_html = achievements_html,
        friends_html = friends_html,
        interact_menu = interact_menu,
        compose = compose,
        posts_html = posts_html,
    )
}

pub fn author_username_for_email(state: &AppState, email: &str) -> String {
    user_for_email(state, email)
        .map(|user| user.username)
        .unwrap_or_else(|| "Cat parent".to_string())
}
