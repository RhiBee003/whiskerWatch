use crate::sharing;
use crate::user_for_email;
use crate::{escape_html, escape_html_attr, profile_has_pet, AppState, UserProfile};
use crate::storage::StoredSocialPost;
use crate::storage::StoredSocialPostComment;
use crate::storage::StoredSocialPostMedia;
use crate::storage::{StorageError};
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

fn comment_to_item(comment: &StoredSocialPostComment) -> SocialPostCommentItem {
    SocialPostCommentItem {
        id: comment.id.clone(),
        author_username: comment.author_username.clone(),
        body: comment.body.clone(),
        created_at: comment.created_at,
        upvotes: comment.upvotes,
        viewer_upvoted: comment.viewer_upvoted,
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
                .filter(|post| can_view_social_post(post, viewer_email))
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
        .filter(|post| can_view_social_post(post, viewer_email))
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
    let Some(post_id) = state
        .storage
        .get_social_post_id_for_comment(comment_id)?
    else {
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

    let summary = state
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
        comment: Some(comment_to_item(&comment)),
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
    format!(
        r#"<button type="button" class="{class}" data-post-upvote="{id}"{pressed} aria-label="Upvote post">▲ {upvotes}</button>"#,
        class = active_class.trim(),
        id = escape_html_attr(&post.id),
        pressed = pressed,
        upvotes = post.upvotes,
    )
}

fn render_social_post_comment_item(comment: &StoredSocialPostComment) -> String {
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
    format!(
        r#"<li class="social-post-comment" data-comment-id="{id}">
  <div class="social-post-comment-main">
    <p class="social-post-comment-meta"><strong>{author}</strong> · {when}</p>
    <p class="social-post-comment-body">{body}</p>
  </div>
  <button type="button" class="{class}" data-comment-upvote="{id}"{pressed} aria-label="Upvote comment">▲ {upvotes}</button>
</li>"#,
        id = escape_html_attr(&comment.id),
        author = escape_html(&comment.author_username),
        when = escape_html(&format_timestamp(comment.created_at)),
        body = escape_html(&comment.body),
        class = active_class.trim(),
        pressed = pressed,
        upvotes = comment.upvotes,
    )
}

fn render_social_post_comments(post: &StoredSocialPost) -> String {
    if post.comments.is_empty() {
        return String::new();
    }
    let items = post
        .comments
        .iter()
        .map(render_social_post_comment_item)
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
        r#"<form class="social-post-comment-form login-form" data-post-comment-form="{post_id}">
  <label class="visually-hidden" for="{field_id}">Add a comment</label>
  <textarea id="{field_id}" name="body" rows="2" maxlength="{max_len}" placeholder="Add a comment…" data-emoji-picker required></textarea>
  <button type="submit" class="download-btn">Comment</button>
</form>"#,
        post_id = escape_html_attr(post_id),
        field_id = escape_html_attr(&field_id),
        max_len = MAX_SOCIAL_COMMENT_LEN,
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
                    r#"<img class="social-post-photo" src="{url}" alt="Photo {num} of {total}" loading="lazy" />"#,
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
            r#"<img class="social-post-photo" src="{url}" alt="" loading="lazy" />"#,
            url = url,
        ),
        "video" => {
            let duration = item.video_duration.unwrap_or(MAX_SOCIAL_VIDEO_SECONDS);
            format!(
                r#"<video class="social-post-video" src="{url}" controls playsinline preload="metadata" data-max-duration="{duration}"></video>
  <p class="social-post-video-hint">Short clip · up to 10 seconds</p>"#,
                url = url,
                duration = duration,
            )
        }
        _ => String::new(),
    }
}

pub fn render_social_post_card(
    state: &AppState,
    viewer_email: &str,
    post: &StoredSocialPost,
    link_author: bool,
) -> String {
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
        format!(r#"<div class="social-post-media">{media}</div>"#, media = media)
    };
    let friend_action = sharing::render_friend_add_control(state, viewer_email, &post.user_id);
    let private_badge = if post.is_private && post.user_id.eq_ignore_ascii_case(viewer_email) {
        r#"<span class="social-post-private-badge">Private</span>"#
    } else {
        ""
    };
    let delete_form = if post.user_id.eq_ignore_ascii_case(viewer_email) {
        format!(
            r#"<form class="social-post-delete-form" action="/home/social/post/delete" method="post" data-confirm="Remove this post? 🐾">
  <input type="hidden" name="post_id" value="{id}" />
  <input type="hidden" name="posts_view" value="" data-social-posts-view />
  <button type="submit" class="social-post-delete-btn" aria-label="Remove post">Remove 🐾</button>
</form>"#,
            id = escape_html_attr(&post.id),
        )
    } else {
        String::new()
    };

    let engagement = format!(
        r#"<div class="social-post-engagement">
  {upvote}
  <span class="social-post-comment-count">{comment_label}</span>
</div>
{comments}
{comment_form}"#,
        upvote = render_social_post_upvote_controls(post),
        comment_label = if post.comments.is_empty() {
            "0 comments".to_string()
        } else if post.comments.len() == 1 {
            "1 comment".to_string()
        } else {
            format!("{} comments", post.comments.len())
        },
        comments = render_social_post_comments(post),
        comment_form = render_social_post_comment_form(&post.id),
    );

    format!(
        r#"<article class="social-post-card" data-post-id="{id}">
  <header class="social-post-header">
    <div class="social-post-author-block">{author_block}{private_badge}</div>
    <div class="social-post-header-actions">{friend_action}{delete_form}</div>
  </header>
  {media_block}
  {caption}
  {engagement}
</article>"#,
        id = escape_html_attr(&post.id),
        author_block = author_block,
        private_badge = private_badge,
        friend_action = friend_action,
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
                    r#"No friend posts yet. Add friends on the Friends tab, or switch to <a href="/home?tab=forum&amp;community=friends&amp;posts_view=all">All posts</a> to browse the community feed."#
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

    if !can_view_parent_profile(state, viewer_email, &subject_email) {
        return format!(
            r#"<h1>Profile</h1>
<p class="panel-intro">{name}'s profile is private. Connect as friends or browse community cats to discover more parents.</p>
<p><a href="/home?tab=forum&amp;community=friends&amp;posts_view=all" class="download-btn">Back to posts</a></p>"#,
            name = escape_html(username.trim()),
        );
    }

    let subject_profile = state
        .storage
        .load_profile(&subject_email)
        .ok()
        .flatten();
    let display_username = user_for_email(state, &subject_email)
        .map(|user| user.username)
        .unwrap_or_else(|| username.trim().to_string());
    let photo = sharing::user_profile_photo_src(state, &subject_email);
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
    let show_personal =
        sharing::can_see_personal_pet_details(state, viewer_email, &subject_email);
    let friend_action = if is_self {
        String::new()
    } else {
        sharing::render_friend_add_control(state, viewer_email, &subject_email)
    };
    let message_link = if is_self {
        String::new()
    } else if sharing::friend_relation(state, viewer_email, &subject_email)
        == sharing::FriendRelation::Friends
    {
        format!(
            r#"<a href="/home?tab=friends&amp;chat={email}" class="download-btn parent-profile-message-btn">Message</a>"#,
            email = escape_html_attr(&subject_email),
        )
    } else {
        String::new()
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
    <img class="parent-profile-photo" src="{photo}" alt="" width="96" height="96" />
    <div class="parent-profile-meta">
      <h1 class="parent-profile-name">{username}</h1>
      {pet_line}
      {breed_line}
      <div class="parent-profile-actions">{friend_action}{message_link}</div>
    </div>
  </header>
  {achievements_html}
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
        username = escape_html(&display_username),
        pet_line = pet_line,
        breed_line = breed_line,
        achievements_html = achievements_html,
        friend_action = friend_action,
        message_link = message_link,
        compose = compose,
        posts_html = posts_html,
    )
}

pub fn author_username_for_email(state: &AppState, email: &str) -> String {
    user_for_email(state, email)
        .map(|user| user.username)
        .unwrap_or_else(|| "Cat parent".to_string())
}
