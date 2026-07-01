use crate::{
    escape_html, escape_html_attr, household_pet_is_complete, memorial, pet_snapshot,
    profile_has_pet, user_for_email, visible_calendar_events, AppState, CalendarEvent,
    UserProfile, CALENDAR_PREVIEW_HORIZON_DAYS, PRIMARY_PET_ID,
};
use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use crate::storage::{
    StorageError, StoredFriendMessage, StoredFriendRequest, StoredFriendSummary, StoredMessageThread,
    StoredPetShare,
    StoredUserSearchHit,
};

pub const FRIEND_STATUS_PENDING: &str = "pending";
pub const FRIEND_STATUS_ACCEPTED: &str = "accepted";
pub const FRIEND_STATUS_DECLINED: &str = "declined";

pub const SHARE_STATUS_PENDING: &str = "pending";
pub const SHARE_STATUS_ACCEPTED: &str = "accepted";
pub const SHARE_STATUS_DECLINED: &str = "declined";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendRequest {
    pub id: String,
    pub from_email: String,
    pub to_email: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendSummary {
    pub friend_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PetShare {
    pub id: String,
    pub owner_email: String,
    pub shared_with_email: String,
    pub pet_id: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessiblePetSummary {
    pub pet_id: String,
    pub pet_name: String,
    pub owner_email: String,
    pub owner_label: String,
    pub is_owned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetCareTarget {
    pub viewer_email: String,
    pub owner_email: String,
    pub pet_id: String,
    pub is_shared: bool,
}

#[derive(Deserialize)]
pub struct FriendRequestForm {
    pub friend_email: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FriendSearchResult {
    pub email: String,
    pub username: String,
    pub photo_url: String,
    pub pet_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FriendSearchResponse {
    pub ok: bool,
    pub results: Vec<FriendSearchResult>,
}

const FRIEND_SEARCH_LIMIT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendLookupError {
    Empty,
    NotFound,
}

pub fn resolve_friend_identifier(state: &AppState, raw: &str) -> Result<String, FriendLookupError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(FriendLookupError::Empty);
    }

    if trimmed.contains('@') {
        return Ok(normalize_email(trimmed));
    }

    state
        .storage
        .email_for_username(trimmed)
        .map_err(|_| FriendLookupError::NotFound)?
        .map(|email| normalize_email(&email))
        .ok_or(FriendLookupError::NotFound)
}

pub fn accepted_friend_emails(state: &AppState, email: &str) -> Vec<String> {
    state
        .storage
        .list_friends(email)
        .unwrap_or_default()
        .into_iter()
        .map(|summary| summary.friend_email)
        .filter(|friend_email| !users_block_each_other(state, email, friend_email))
        .collect()
}

pub fn friends_pending_count(state: &AppState, email: &str) -> usize {
    let incoming = state
        .storage
        .list_incoming_friend_requests(email)
        .map(|items| items.len())
        .unwrap_or(0);
    let shares = state
        .storage
        .list_incoming_pet_shares(email)
        .map(|items| items.len())
        .unwrap_or(0);
    incoming + shares
}

pub fn friends_tab_badge_count(state: &AppState, email: &str) -> usize {
    let unread = state
        .storage
        .count_unread_friend_messages(email)
        .unwrap_or(0);
    let message_requests = state
        .storage
        .count_pending_message_requests(email)
        .unwrap_or(0);
    friends_pending_count(state, email) + unread + message_requests
}

pub fn render_friends_tab_label(state: &AppState, email: &str) -> String {
    let pending = friends_tab_badge_count(state, email);
    if pending > 0 {
        format!(
            r#"Friends <span class="friends-tab-badge" aria-label="{pending} pending">{pending}</span>"#
        )
    } else {
        "Friends".to_string()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FriendMessageItem {
    pub id: String,
    pub from_email: String,
    pub body: String,
    pub media_type: String,
    pub media_url: Option<String>,
    pub video_duration: Option<f32>,
    pub created_at: u64,
    pub is_mine: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FriendMessagePartner {
    pub email: String,
    pub username: String,
    pub photo_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FriendMessagesResponse {
    pub ok: bool,
    pub friend: Option<FriendMessagePartner>,
    pub messages: Vec<FriendMessageItem>,
    pub thread_status: Option<String>,
    pub can_compose: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FriendMessageSendResponse {
    pub ok: bool,
    pub message: Option<FriendMessageItem>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct FriendMessageSendForm {
    pub friend_email: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Deserialize)]
pub struct FriendMessageReadForm {
    pub friend_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendMessageDeletionNotice {
    pub notice_id: String,
    pub partner_label: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct FriendMessageDeleteForm {
    pub friend_email: String,
    #[serde(default)]
    pub message_id: Option<String>,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FriendMessageDeleteResponse {
    pub ok: bool,
    pub scope: String,
    pub conversation_cleared: bool,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct MessageRequestRespondForm {
    pub partner_email: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MessageRequestRespondResponse {
    pub ok: bool,
    pub status: Option<String>,
}

const FRIEND_MESSAGE_LIMIT: usize = 80;
const MESSAGE_PREVIEW_CHARS: usize = 72;

fn stored_friend_message_item(message: &StoredFriendMessage, viewer_email: &str) -> FriendMessageItem {
    FriendMessageItem {
        id: message.id.clone(),
        from_email: message.from_email.clone(),
        body: message.body.clone(),
        media_type: message.media_type.clone(),
        media_url: message.media_url.clone(),
        video_duration: message.video_duration,
        created_at: message.created_at,
        is_mine: message.from_email.eq_ignore_ascii_case(viewer_email),
    }
}

fn message_thread_status_for_viewer(
    thread: Option<&StoredMessageThread>,
    viewer_email: &str,
    are_friends: bool,
) -> (Option<String>, bool) {
    if are_friends {
        return (Some("accepted".to_string()), true);
    }
    let Some(thread) = thread else {
        return (None, false);
    };
    match thread.status.as_str() {
        "accepted" => (Some("accepted".to_string()), true),
        "declined" => (Some("declined".to_string()), false),
        "pending" if thread.initiated_by.eq_ignore_ascii_case(viewer_email) => {
            (Some("pending_outgoing".to_string()), true)
        }
        "pending" => (Some("pending_incoming".to_string()), false),
        _ => (Some(thread.status.clone()), false),
    }
}

pub fn friend_messages_for_conversation(
    state: &AppState,
    viewer_email: &str,
    friend_email: &str,
) -> Result<FriendMessagesResponse, StorageError> {
    let viewer_email = normalize_email(viewer_email);
    let friend_email = normalize_email(friend_email);
    if viewer_email == friend_email {
        return Err(StorageError::InvalidInput("cannot message yourself".into()));
    }
    if !state.storage.user_exists(&friend_email)? {
        return Err(StorageError::InvalidInput("user not found".into()));
    }

    let are_friends = state.storage.are_friends(&viewer_email, &friend_email)?;
    let thread = state
        .storage
        .get_message_thread(&viewer_email, &friend_email)?;
    let (thread_status, can_compose) =
        message_thread_status_for_viewer(thread.as_ref(), &viewer_email, are_friends);

    if thread.as_ref().is_some_and(|value| value.status == "declined") {
        let messages = state
            .storage
            .list_friend_conversation(&viewer_email, &friend_email, FRIEND_MESSAGE_LIMIT)?
            .iter()
            .map(|message| stored_friend_message_item(message, &viewer_email))
            .collect();
        return Ok(FriendMessagesResponse {
            ok: true,
            friend: Some(FriendMessagePartner {
                email: friend_email.clone(),
                username: user_label(state, &friend_email),
                photo_url: user_profile_photo_src(state, &friend_email),
            }),
            messages,
            thread_status,
            can_compose: false,
        });
    }

    let can_view = are_friends
        || thread.is_some()
        || thread_status.is_none();
    if !can_view {
        return Err(StorageError::InvalidInput("cannot view conversation".into()));
    }

    let messages = if are_friends || thread.is_some() {
        state
            .storage
            .list_friend_conversation(&viewer_email, &friend_email, FRIEND_MESSAGE_LIMIT)?
            .iter()
            .map(|message| stored_friend_message_item(message, &viewer_email))
            .collect()
    } else {
        Vec::new()
    };

    let can_compose = can_compose || (!are_friends && thread.is_none());

    Ok(FriendMessagesResponse {
        ok: true,
        friend: Some(FriendMessagePartner {
            email: friend_email.clone(),
            username: user_label(state, &friend_email),
            photo_url: user_profile_photo_src(state, &friend_email),
        }),
        messages,
        thread_status,
        can_compose,
    })
}

pub fn send_friend_message(
    state: &AppState,
    viewer_email: &str,
    friend_email: &str,
    body: &str,
    media_type: &str,
    media_url: Option<&str>,
    video_duration: Option<f32>,
    created_at: u64,
) -> Result<FriendMessageSendResponse, StorageError> {
    let viewer_email = normalize_email(viewer_email);
    let message = state.storage.send_friend_message(
        &viewer_email,
        friend_email,
        body,
        media_type,
        media_url,
        video_duration,
        created_at,
    )?;
    Ok(FriendMessageSendResponse {
        ok: true,
        message: Some(stored_friend_message_item(&message, &viewer_email)),
        status: None,
    })
}

pub fn respond_message_request(
    state: &AppState,
    viewer_email: &str,
    partner_email: &str,
    accept: bool,
    responded_at: u64,
) -> Result<MessageRequestRespondResponse, StorageError> {
    let viewer_email = normalize_email(viewer_email);
    let partner_email = normalize_email(partner_email);
    if accept {
        state
            .storage
            .accept_message_thread_between(&viewer_email, &partner_email, responded_at)?;
    } else {
        state
            .storage
            .decline_message_thread_between(&viewer_email, &partner_email, responded_at)?;
    }
    Ok(MessageRequestRespondResponse {
        ok: true,
        status: Some(if accept { "accepted" } else { "declined" }.to_string()),
    })
}

pub fn mark_friend_messages_read(
    state: &AppState,
    viewer_email: &str,
    friend_email: &str,
    read_at: u64,
) -> Result<(), StorageError> {
    state
        .storage
        .mark_friend_conversation_read(viewer_email, friend_email, read_at)
}

fn friend_message_partner_email(message: &StoredFriendMessage, viewer_email: &str) -> String {
    if message.from_email.eq_ignore_ascii_case(viewer_email) {
        message.to_email.clone()
    } else {
        message.from_email.clone()
    }
}

fn queue_friend_message_deletion_notice(
    state: &AppState,
    recipient_email: &str,
    deleter_email: &str,
    summary: &str,
    notice_id: String,
) {
    if recipient_email.eq_ignore_ascii_case(deleter_email) {
        return;
    }
    let Some(mut profile) = load_profile_by_email(state, recipient_email) else {
        return;
    };
    let created_at = Local::now().to_rfc3339();
    profile
        .friend_message_deletion_notices
        .retain(|notice| notice.notice_id != notice_id);
    profile
        .friend_message_deletion_notices
        .push(FriendMessageDeletionNotice {
            notice_id,
            partner_label: user_label(state, deleter_email),
            summary: summary.to_string(),
            created_at,
        });
    let _ = state.storage.save_profile(&profile);
}

fn notify_partner_message_deleted_for_all(
    state: &AppState,
    viewer_email: &str,
    friend_email: &str,
    message: &StoredFriendMessage,
    deleted_at: u64,
) {
    let partner = friend_message_partner_email(message, viewer_email);
    if !partner.eq_ignore_ascii_case(friend_email) {
        return;
    }
    let summary = if message.body.trim().is_empty() {
        "removed a shared photo or video for both of you.".to_string()
    } else {
        "deleted a message for both of you.".to_string()
    };
    queue_friend_message_deletion_notice(
        state,
        &partner,
        viewer_email,
        &summary,
        format!("fmd-msg-{deleted_at}-{}", message.id),
    );
}

fn notify_partner_conversation_deleted_for_all(
    state: &AppState,
    viewer_email: &str,
    friend_email: &str,
    deleted_at: u64,
) {
    queue_friend_message_deletion_notice(
        state,
        friend_email,
        viewer_email,
        "cleared your entire message thread for both of you.",
        format!("fmd-conv-{deleted_at}-{friend_email}"),
    );
}

pub fn delete_friend_message(
    state: &AppState,
    viewer_email: &str,
    friend_email: &str,
    message_id: Option<&str>,
    scope: &str,
    deleted_at: u64,
) -> Result<FriendMessageDeleteResponse, StorageError> {
    let viewer_email = normalize_email(viewer_email);
    let friend_email = normalize_email(friend_email);
    let scope = scope.trim();
    let conversation_cleared = matches!(scope, "conversation_me" | "conversation_both");

    match scope {
        "message_me" => {
            let Some(message_id) = message_id.map(str::trim).filter(|value| !value.is_empty()) else {
                return Err(StorageError::InvalidInput("message_id required".into()));
            };
            let Some(message) = state.storage.get_friend_message(message_id)? else {
                return Err(StorageError::InvalidInput("message not found".into()));
            };
            let partner = friend_message_partner_email(&message, &viewer_email);
            if !partner.eq_ignore_ascii_case(&friend_email) {
                return Err(StorageError::InvalidInput("message not in conversation".into()));
            }
            state
                .storage
                .hide_friend_message_for_user(message_id, &viewer_email, deleted_at)?;
        }
        "message_both" => {
            let Some(message_id) = message_id.map(str::trim).filter(|value| !value.is_empty()) else {
                return Err(StorageError::InvalidInput("message_id required".into()));
            };
            let message =
                state
                    .storage
                    .delete_friend_message_for_all(message_id, &viewer_email, deleted_at)?;
            let partner = friend_message_partner_email(&message, &viewer_email);
            if !partner.eq_ignore_ascii_case(&friend_email) {
                return Err(StorageError::InvalidInput("message not in conversation".into()));
            }
            notify_partner_message_deleted_for_all(
                state,
                &viewer_email,
                &friend_email,
                &message,
                deleted_at,
            );
        }
        "conversation_me" => {
            state.storage.hide_friend_conversation_for_user(
                &viewer_email,
                &friend_email,
                deleted_at,
            )?;
        }
        "conversation_both" => {
            state
                .storage
                .delete_friend_conversation_for_all(&viewer_email, &friend_email, deleted_at)?;
            notify_partner_conversation_deleted_for_all(
                state,
                &viewer_email,
                &friend_email,
                deleted_at,
            );
        }
        _ => return Err(StorageError::InvalidInput("invalid scope".into())),
    }

    Ok(FriendMessageDeleteResponse {
        ok: true,
        scope: scope.to_string(),
        conversation_cleared,
        status: None,
    })
}

fn message_preview(message: &StoredFriendMessage) -> String {
    if !message.body.trim().is_empty() {
        let trimmed = message.body.trim();
        if trimmed.chars().count() <= MESSAGE_PREVIEW_CHARS {
            return trimmed.to_string();
        }
        let mut preview = String::new();
        for ch in trimmed.chars().take(MESSAGE_PREVIEW_CHARS) {
            preview.push(ch);
        }
        preview.push('…');
        return preview;
    }
    match message.media_type.as_str() {
        "photo" => "📷 Photo".to_string(),
        "video" => "🎬 Video".to_string(),
        _ => "Say hi!".to_string(),
    }
}

fn conversation_partner_emails(state: &AppState, viewer_email: &str) -> Vec<String> {
    let mut partners = state
        .storage
        .list_friends(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(|friend| friend.friend_email)
        .collect::<Vec<_>>();
    for email in state
        .storage
        .list_message_thread_partners(viewer_email)
        .unwrap_or_default()
    {
        if !partners
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&email))
        {
            partners.push(email);
        }
    }
    partners
        .into_iter()
        .filter(|partner| !users_block_each_other(state, viewer_email, partner))
        .collect()
}

pub fn render_friend_messages_card(state: &AppState, viewer_email: &str) -> String {
    let partners = conversation_partner_emails(state, viewer_email);
    let incoming_requests = state
        .storage
        .count_pending_message_requests(viewer_email)
        .unwrap_or(0);

    let incoming_request_partners: Vec<String> = state
        .storage
        .list_message_thread_partners(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|partner| {
            if users_block_each_other(state, viewer_email, &partner) {
                return None;
            }
            let thread = state.storage.get_message_thread(viewer_email, &partner).ok()??;
            if thread.status == "pending"
                && !thread.initiated_by.eq_ignore_ascii_case(viewer_email)
            {
                Some(partner)
            } else {
                None
            }
        })
        .collect();

    let request_items: String = incoming_request_partners
        .iter()
        .filter_map(|partner| {
            let label = user_label(state, partner);
            let photo = user_profile_photo_src(state, partner);
            let preview = state
                .storage
                .last_friend_message_with(viewer_email, partner)
                .ok()
                .flatten()
                .map(|message| message_preview(&message))
                .unwrap_or_else(|| "Sent you a message".to_string());
            Some(format!(
                r#"<li><button type="button" class="friend-message-thread-btn friend-message-request-btn" data-friend-email="{email}" data-friend-label="{label}" data-thread-status="pending_incoming">
  <img class="friend-message-thread-photo" src="{photo}" alt="" width="44" height="44" loading="lazy" />
  <span class="friend-message-thread-meta">
    <strong class="friend-message-thread-name">{label}</strong>
    <span class="friend-message-thread-preview">{preview}</span>
    <span class="friend-message-request-badge">Message request</span>
  </span>
</button></li>"#,
                email = escape_html_attr(partner),
                label = escape_html_attr(&label),
                photo = escape_html_attr(&photo),
                preview = escape_html(&preview),
            ))
        })
        .collect::<Vec<_>>()
        .join("");

    let thread_buttons = partners
        .iter()
        .filter_map(|partner| {
            if incoming_request_partners
                .iter()
                .any(|email| email.eq_ignore_ascii_case(partner))
            {
                return None;
            }
            if users_block_each_other(state, viewer_email, &partner) {
                return None;
            }
            let are_friends = state
                .storage
                .are_friends(viewer_email, partner)
                .unwrap_or(false);
            let thread = state
                .storage
                .get_message_thread(viewer_email, partner)
                .ok()
                .flatten();
            if !are_friends {
                if thread.as_ref().is_none_or(|value| value.status == "declined") {
                    return None;
                }
            }
            if state
                .storage
                .friend_conversation_hidden_for_user(viewer_email, partner)
                .unwrap_or(false)
                && state
                    .storage
                    .last_friend_message_with(viewer_email, partner)
                    .ok()
                    .flatten()
                    .is_none()
            {
                return None;
            }
            let label = user_label(state, partner);
            let photo = user_profile_photo_src(state, partner);
            let unread = state
                .storage
                .count_unread_from_friend(viewer_email, partner)
                .unwrap_or(0);
            let preview = state
                .storage
                .last_friend_message_with(viewer_email, partner)
                .ok()
                .flatten()
                .map(|message| message_preview(&message))
                .unwrap_or_else(|| "Say hi!".to_string());
            let unread_badge = if unread > 0 {
                format!(
                    r#"<span class="friend-message-unread-badge" aria-label="{unread} unread">{unread}</span>"#,
                    unread = unread
                )
            } else {
                String::new()
            };
            let thread_status = message_thread_status_for_viewer(
                thread.as_ref(),
                viewer_email,
                are_friends,
            )
            .0
            .unwrap_or_else(|| "accepted".to_string());
            let request_badge = if thread_status == "pending_outgoing" {
                r#"<span class="friend-message-request-badge friend-message-request-badge-outgoing">Request sent</span>"#
            } else {
                ""
            };

            Some(format!(
                r#"<li><button type="button" class="friend-message-thread-btn" data-friend-email="{email}" data-friend-label="{label}" data-thread-status="{thread_status}">
  <img class="friend-message-thread-photo" src="{photo}" alt="" width="44" height="44" loading="lazy" />
  <span class="friend-message-thread-meta">
    <strong class="friend-message-thread-name">{label}</strong>
    <span class="friend-message-thread-preview">{preview}</span>
    {request_badge}
  </span>
  {unread_badge}
</button></li>"#,
                email = escape_html_attr(partner),
                label = escape_html_attr(&label),
                photo = escape_html_attr(&photo),
                preview = escape_html(&preview),
                thread_status = escape_html_attr(&thread_status),
                request_badge = request_badge,
                unread_badge = unread_badge,
            ))
        })
        .collect::<Vec<_>>()
        .join("");

    let requests_section = if request_items.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="friend-message-requests" aria-label="Message requests">
  <h4 class="friend-message-requests-title">Message requests ({count})</h4>
  <ul class="friend-message-thread-list friend-message-request-list">{items}</ul>
</section>"#,
            count = incoming_requests,
            items = request_items,
        )
    };

    format!(
        r#"<article class="dashboard-card friend-messages-card" id="friend-messages-card">
  <h3 class="friends-subhead">Messages</h3>
  <p class="field-hint">Text, photo, or video your friends — or send a message request to any WhiskerWatch parent.</p>
  <div class="friend-message-new-search" data-friend-message-search>
    <label class="visually-hidden" for="friend_message_search_query">Find someone to message</label>
    <input id="friend_message_search_query" type="search" autocomplete="off" placeholder="Search username to message…" aria-controls="friend_message_search_results" aria-expanded="false" />
    <div id="friend_message_search_results" class="friend-search-results friend-message-search-results" role="listbox" aria-label="Matching users" hidden></div>
  </div>
  {requests_section}
  <div class="friend-messages-shell">
    <aside class="friend-messages-sidebar" aria-label="Conversations">
      <ul class="friend-message-thread-list">{thread_buttons}</ul>
    </aside>
    <section class="friend-messages-panel" id="friend-messages-panel" hidden>
      <header class="friend-messages-header">
        <img class="friend-messages-header-photo" id="friend-messages-header-photo" src="/cinderanimate.png" alt="" width="40" height="40" />
        <strong class="friend-messages-header-name" id="friend-messages-header-name"></strong>
        <button type="button" class="user-block-btn friend-messages-block-btn" id="friend-messages-block-btn" data-block-user-email="" data-block-action="block" hidden>Block</button>
      </header>
      <div class="friend-message-request-actions" id="friend-message-request-actions" hidden>
        <p class="friend-message-request-copy">Accept to reply, or decline this message request.</p>
        <div class="friend-message-request-buttons">
          <button type="button" class="download-btn" id="friend-message-request-accept">Accept</button>
          <button type="button" class="onboarding-skip-btn" id="friend-message-request-decline">Decline</button>
        </div>
      </div>
      <div class="friend-messages-thread" id="friend-messages-thread" aria-live="polite"></div>
      <form class="friend-messages-compose" id="friend-messages-compose" enctype="multipart/form-data">
        <div class="friend-messages-compose-row">
          <label class="friend-messages-attach-btn" for="friend_message_media" title="Attach photo or video">📎</label>
          <input id="friend_message_media" name="media" type="file" accept="image/*,video/*" hidden />
          <div class="friend-messages-compose-fields">
            <div class="friend-messages-media-preview" id="friend-message-media-preview" hidden></div>
            <label class="visually-hidden" for="friend_message_body">Message</label>
            <textarea id="friend_message_body" name="body" rows="2" maxlength="2000" placeholder="Type a message…" data-emoji-picker></textarea>
          </div>
          <button type="submit" class="download-btn">Send</button>
        </div>
      </form>
    </section>
    <p class="friend-messages-placeholder" id="friend-messages-placeholder">Pick a conversation or search for someone to message.</p>
  </div>
</article>"#,
        requests_section = requests_section,
        thread_buttons = if thread_buttons.is_empty() {
            r#"<li class="friend-message-thread-empty"><p class="field-hint">No conversations yet — search above to send a message request.</p></li>"#.to_string()
        } else {
            thread_buttons
        },
    )
}

#[derive(Deserialize)]
pub struct FriendRespondForm {
    pub request_id: String,
    pub action: String,
}

#[derive(Deserialize)]
pub struct PetShareForm {
    pub friend_email: String,
    pub pet_id: String,
}

#[derive(Deserialize)]
pub struct PetShareRespondForm {
    pub share_id: String,
    pub action: String,
}

#[derive(Deserialize)]
pub struct PetShareRevokeForm {
    pub share_id: String,
}

pub fn normalize_email(value: &str) -> String {
    value.trim().to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendRelation {
    SelfUser,
    Friends,
    PendingOutgoing,
    PendingIncoming,
    Blocked,
    NotFriends,
}

#[derive(Debug, Deserialize)]
pub struct UserBlockForm {
    pub target_email: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserBlockResponse {
    pub ok: bool,
    pub action: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FriendQuickRequestResponse {
    pub ok: bool,
    pub status: String,
}

pub fn users_block_each_other(state: &AppState, left_email: &str, right_email: &str) -> bool {
    state
        .storage
        .users_block_each_other(left_email, right_email)
        .unwrap_or(false)
}

pub fn viewer_blocked_user(state: &AppState, viewer_email: &str, target_email: &str) -> bool {
    state
        .storage
        .is_user_blocked(viewer_email, target_email)
        .unwrap_or(false)
}

pub fn block_user_profile(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
    blocked_at: u64,
) -> Result<(), StorageError> {
    let viewer_email = normalize_email(viewer_email);
    let target_email = normalize_email(target_email);
    if viewer_email == target_email {
        return Err(StorageError::InvalidInput("cannot block yourself".into()));
    }
    state.storage.block_user(&viewer_email, &target_email, blocked_at)?;
    state
        .storage
        .remove_friendship_between(&viewer_email, &target_email)?;
    state
        .storage
        .cancel_pending_friend_requests_between(&viewer_email, &target_email)?;
    let _ = state.storage.hide_friend_conversation_for_user(
        &viewer_email,
        &target_email,
        blocked_at,
    );
    Ok(())
}

pub fn unblock_user_profile(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
) -> Result<(), StorageError> {
    let viewer_email = normalize_email(viewer_email);
    let target_email = normalize_email(target_email);
    state
        .storage
        .unblock_user(&viewer_email, &target_email)
}

pub fn apply_user_block_action(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
    action: &str,
    blocked_at: u64,
) -> Result<UserBlockResponse, StorageError> {
    let action = action.trim().to_lowercase();
    match action.as_str() {
        "block" => {
            block_user_profile(state, viewer_email, target_email, blocked_at)?;
            Ok(UserBlockResponse {
                ok: true,
                action: "block".to_string(),
                status: None,
            })
        }
        "unblock" => {
            unblock_user_profile(state, viewer_email, target_email)?;
            Ok(UserBlockResponse {
                ok: true,
                action: "unblock".to_string(),
                status: None,
            })
        }
        _ => Err(StorageError::InvalidInput("invalid action".into())),
    }
}

pub fn render_block_control(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
) -> String {
    let viewer_email = normalize_email(viewer_email);
    let target_email = normalize_email(target_email);
    if viewer_email.is_empty() || target_email.is_empty() || viewer_email == target_email {
        return String::new();
    }

    let label = user_label(state, &target_email);
    if viewer_blocked_user(state, &viewer_email, &target_email) {
        format!(
            r#"<button type="button" class="user-block-btn user-block-btn-unblock" data-block-user-email="{email}" data-block-action="unblock" aria-label="Unblock {label}">Unblock</button>"#,
            email = escape_html_attr(&target_email),
            label = escape_html_attr(&label),
        )
    } else {
        format!(
            r#"<button type="button" class="user-block-btn" data-block-user-email="{email}" data-block-action="block" aria-label="Block {label}">Block</button>"#,
            email = escape_html_attr(&target_email),
            label = escape_html_attr(&label),
        )
    }
}

pub fn friend_relation(state: &AppState, viewer_email: &str, target_email: &str) -> FriendRelation {
    let viewer_email = normalize_email(viewer_email);
    let target_email = normalize_email(target_email);
    if viewer_email.is_empty() || target_email.is_empty() {
        return FriendRelation::NotFriends;
    }
    if viewer_email == target_email {
        return FriendRelation::SelfUser;
    }
    if users_block_each_other(state, &viewer_email, &target_email) {
        return FriendRelation::Blocked;
    }
    if state
        .storage
        .are_friends(&viewer_email, &target_email)
        .unwrap_or(false)
    {
        return FriendRelation::Friends;
    }
    if state
        .storage
        .list_outgoing_friend_requests(&viewer_email)
        .unwrap_or_default()
        .into_iter()
        .any(|request| request.to_email.eq_ignore_ascii_case(&target_email))
    {
        return FriendRelation::PendingOutgoing;
    }
    if state
        .storage
        .list_incoming_friend_requests(&viewer_email)
        .unwrap_or_default()
        .into_iter()
        .any(|request| request.from_email.eq_ignore_ascii_case(&target_email))
    {
        return FriendRelation::PendingIncoming;
    }
    FriendRelation::NotFriends
}

pub fn can_see_personal_pet_details(
    state: &AppState,
    viewer_email: &str,
    owner_email: &str,
) -> bool {
    matches!(
        friend_relation(state, viewer_email, owner_email),
        FriendRelation::SelfUser | FriendRelation::Friends
    )
}

pub fn render_friend_add_control(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
) -> String {
    let target_email = normalize_email(target_email);
    if target_email.is_empty() {
        return String::new();
    }

    let label = user_label(state, &target_email);
    match friend_relation(state, viewer_email, &target_email) {
        FriendRelation::SelfUser => String::new(),
        FriendRelation::Blocked => String::new(),
        FriendRelation::Friends => format!(
            r#"<a href="/home?tab=friends&amp;chat={email}" class="friend-add-status friend-add-status-friends">Friends · Message</a>"#,
            email = escape_html_attr(&target_email),
        ),
        FriendRelation::PendingOutgoing => {
            r#"<span class="friend-add-status friend-add-status-pending">💌 Invite sent</span>"#.to_string()
        }
        FriendRelation::PendingIncoming => {
            r#"<a href="/home?tab=friends" class="friend-add-status friend-add-status-incoming">Respond on Friends tab</a>"#
                .to_string()
        }
        FriendRelation::NotFriends => format!(
            r#"<button type="button" class="friend-add-btn" data-friend-request-email="{email}" aria-label="Add {label} as friend">Add friend</button>
<a href="/home?tab=friends&amp;chat={email}" class="friend-add-status friend-add-status-message">Message</a>"#,
            email = escape_html_attr(&target_email),
            label = escape_html_attr(&label),
        ),
    }
}

fn parent_profile_path(username: &str) -> String {
    format!(
        "/home?tab=profile&parent={}",
        urlencoding::encode(username.trim())
    )
}

fn profile_interact_menu_item_link(href: &str, label: &str) -> String {
    format!(
        r#"<a href="{href}" class="profile-interact-menu-item" role="menuitem">{label}</a>"#,
        href = escape_html_attr(href),
        label = escape_html(label),
    )
}

fn profile_interact_menu_item_button(class_suffix: &str, attrs: &str, label: &str) -> String {
    format!(
        r#"<button type="button" class="profile-interact-menu-item{suffix}" role="menuitem"{attrs}>{label}</button>"#,
        suffix = class_suffix,
        attrs = attrs,
        label = escape_html(label),
    )
}

fn profile_interact_menu_item_status(label: &str) -> String {
    format!(
        r#"<span class="profile-interact-menu-item profile-interact-menu-item-muted" role="menuitem" aria-disabled="true">{label}</span>"#,
        label = escape_html(label),
    )
}

pub fn render_profile_interact_menu(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
    profile_username: &str,
    on_profile_page: bool,
) -> String {
    let viewer_email = normalize_email(viewer_email);
    let target_email = normalize_email(target_email);
    if viewer_email.is_empty() || target_email.is_empty() || viewer_email == target_email {
        return String::new();
    }

    let label = user_label(state, &target_email);
    let profile_url = parent_profile_path(profile_username);
    let chat_url = format!("/home?tab=friends&chat={}", urlencoding::encode(&target_email));
    let email_attr = escape_html_attr(&target_email);
    let label_attr = escape_html_attr(&label);
    let relation = friend_relation(state, &viewer_email, &target_email);
    let viewer_blocked = viewer_blocked_user(state, &viewer_email, &target_email);

    let mut items: Vec<String> = Vec::new();

    if !on_profile_page {
        items.push(profile_interact_menu_item_link(
            &profile_url,
            "View profile",
        ));
    }

    match relation {
        FriendRelation::SelfUser => return String::new(),
        FriendRelation::Friends => {
            items.push(profile_interact_menu_item_link(&chat_url, "Message"));
        }
        FriendRelation::PendingOutgoing => {
            items.push(profile_interact_menu_item_status("Invite sent"));
        }
        FriendRelation::PendingIncoming => {
            items.push(profile_interact_menu_item_link(
                "/home?tab=friends",
                "Respond on Friends tab",
            ));
        }
        FriendRelation::NotFriends => {
            items.push(profile_interact_menu_item_button(
                " friend-add-btn",
                &format!(r#" data-friend-request-email="{email_attr}""#),
                "Add friend",
            ));
            items.push(profile_interact_menu_item_link(&chat_url, "Message"));
        }
        FriendRelation::Blocked => {
            if !viewer_blocked && !on_profile_page {
                // They blocked the viewer — only profile link is shown above.
            }
        }
    }

    if viewer_blocked {
        items.push(profile_interact_menu_item_button(
            " user-block-btn user-block-btn-unblock",
            &format!(
                r#" data-block-user-email="{email_attr}" data-block-action="unblock" aria-label="Unblock {label_attr}""#
            ),
            "Unblock",
        ));
    } else if relation != FriendRelation::Blocked {
        items.push(format!(
            r#"<div class="profile-interact-menu-divider" role="separator" aria-hidden="true"></div>{block_item}"#,
            block_item = profile_interact_menu_item_button(
                " profile-interact-menu-item-danger user-block-btn",
                &format!(
                    r#" data-block-user-email="{email_attr}" data-block-action="block" aria-label="Block {label_attr}""#
                ),
                "Block",
            ),
        ));
    }

    if items.is_empty() {
        return String::new();
    }

    format!(
        r#"<div class="profile-interact-menu">
  <button type="button" class="profile-interact-menu-trigger" aria-haspopup="menu" aria-expanded="false" aria-label="Connect with {label_attr}" title="Connect with {label_attr}">🐾</button>
  <div class="profile-interact-menu-panel" role="menu" hidden>{items}</div>
</div>"#,
        label_attr = label_attr,
        items = items.join(""),
    )
}

pub fn quick_friend_request(
    state: &AppState,
    viewer_email: &str,
    target_email: &str,
    created_at: u64,
) -> FriendQuickRequestResponse {
    let viewer_email = normalize_email(viewer_email);
    let target_email = normalize_email(target_email);

    if target_email.is_empty() {
        return FriendQuickRequestResponse {
            ok: false,
            status: "invalid".to_string(),
        };
    }
    if viewer_email == target_email {
        return FriendQuickRequestResponse {
            ok: false,
            status: "self".to_string(),
        };
    }

    match friend_relation(state, &viewer_email, &target_email) {
        FriendRelation::Friends => FriendQuickRequestResponse {
            ok: true,
            status: "friends".to_string(),
        },
        FriendRelation::PendingOutgoing => FriendQuickRequestResponse {
            ok: true,
            status: "pending".to_string(),
        },
        FriendRelation::PendingIncoming => FriendQuickRequestResponse {
            ok: true,
            status: "incoming".to_string(),
        },
        FriendRelation::Blocked => FriendQuickRequestResponse {
            ok: false,
            status: "blocked".to_string(),
        },
        FriendRelation::SelfUser => FriendQuickRequestResponse {
            ok: false,
            status: "self".to_string(),
        },
        FriendRelation::NotFriends => {
            if !state.storage.user_exists(&target_email).unwrap_or(false) {
                return FriendQuickRequestResponse {
                    ok: false,
                    status: "not_found".to_string(),
                };
            }
            match state
                .storage
                .create_friend_request(&viewer_email, &target_email, created_at)
            {
                Ok(()) => FriendQuickRequestResponse {
                    ok: true,
                    status: "sent".to_string(),
                },
                Err(StorageError::InvalidInput(message))
                    if message.contains("already friends") =>
                {
                    FriendQuickRequestResponse {
                        ok: true,
                        status: "friends".to_string(),
                    }
                }
                Err(StorageError::InvalidInput(message))
                    if message.contains("request already pending") =>
                {
                    FriendQuickRequestResponse {
                        ok: true,
                        status: "pending".to_string(),
                    }
                }
                Err(_) => FriendQuickRequestResponse {
                    ok: false,
                    status: "error".to_string(),
                },
            }
        }
    }
}

pub fn user_label(state: &AppState, email: &str) -> String {
    user_for_email(state, email)
        .map(|user| {
            let username = user.username.trim();
            if username.is_empty() {
                email.to_string()
            } else {
                username.to_string()
            }
        })
        .unwrap_or_else(|| email.to_string())
}

pub fn user_profile_photo_src(state: &AppState, email: &str) -> String {
    load_profile_by_email(state, email)
        .map(|profile| crate::main_cat_photo_src(&profile))
        .unwrap_or_else(|| "/cinderanimate.png".to_string())
}

fn pending_friend_hint(state: &AppState, email: &str) -> String {
    let pet_name = load_profile_by_email(state, email)
        .as_ref()
        .map(|profile| profile.pet_name.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    match pet_name {
        Some(name) => format!("Can't wait to meet {name}! 🐾"),
        None => "Waiting for them to say hi back".to_string(),
    }
}

pub fn search_friend_candidates(
    state: &AppState,
    viewer_email: &str,
    query: &str,
) -> Vec<FriendSearchResult> {
    let viewer_email = normalize_email(viewer_email);
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let excluded = friend_search_excluded_emails(state, &viewer_email);
    let hits = state
        .storage
        .search_users_by_username(query, FRIEND_SEARCH_LIMIT)
        .unwrap_or_default();

    hits.into_iter()
        .filter(|hit| !excluded.contains(&normalize_email(&hit.email)))
        .map(|hit| friend_search_result_from_hit(state, hit))
        .collect()
}

pub fn search_message_candidates(
    state: &AppState,
    viewer_email: &str,
    query: &str,
) -> Vec<FriendSearchResult> {
    let viewer_email = normalize_email(viewer_email);
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let mut results = state
        .storage
        .search_users_by_username(query, FRIEND_SEARCH_LIMIT)
        .unwrap_or_default()
        .into_iter()
        .filter(|hit| !normalize_email(&hit.email).eq_ignore_ascii_case(&viewer_email))
        .map(|hit| friend_search_result_from_hit(state, hit))
        .collect::<Vec<_>>();

    if results.is_empty() {
        if let Ok(Some(email)) = state.storage.email_for_username(query) {
            let email = normalize_email(&email);
            if !email.eq_ignore_ascii_case(&viewer_email) {
                if let Ok(Some(user)) = state.storage.find_user_by_email(&email) {
                    results.push(friend_search_result_from_hit(
                        state,
                        StoredUserSearchHit {
                            email: user.email,
                            username: user.username,
                            first_name: user.first_name,
                            last_name: user.last_name,
                        },
                    ));
                }
            }
        }
    }

    results
}

fn friend_search_excluded_emails(
    state: &AppState,
    viewer_email: &str,
) -> std::collections::HashSet<String> {
    let mut excluded = std::collections::HashSet::new();
    excluded.insert(viewer_email.to_string());

    for friend in state.storage.list_friends(viewer_email).unwrap_or_default() {
        excluded.insert(normalize_email(&friend.friend_email));
    }
    for request in state
        .storage
        .list_incoming_friend_requests(viewer_email)
        .unwrap_or_default()
    {
        excluded.insert(normalize_email(&request.from_email));
    }
    for request in state
        .storage
        .list_outgoing_friend_requests(viewer_email)
        .unwrap_or_default()
    {
        excluded.insert(normalize_email(&request.to_email));
    }
    for blocked in state
        .storage
        .list_blocked_users(viewer_email)
        .unwrap_or_default()
    {
        excluded.insert(normalize_email(&blocked));
    }
    for blocker in state
        .storage
        .list_users_who_blocked(viewer_email)
        .unwrap_or_default()
    {
        excluded.insert(normalize_email(&blocker));
    }

    excluded
}

fn friend_search_result_from_hit(state: &AppState, hit: StoredUserSearchHit) -> FriendSearchResult {
    let email = normalize_email(&hit.email);

    FriendSearchResult {
        email,
        username: hit.username.trim().to_string(),
        photo_url: user_profile_photo_src(state, &hit.email),
        pet_name: None,
    }
}

pub fn active_pet_owner_email(profile: &UserProfile) -> &str {
    profile
        .active_pet_owner_email
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(profile.email.as_str())
}

pub fn is_viewing_shared_pet(profile: &UserProfile) -> bool {
    profile
        .active_pet_owner_email
        .as_deref()
        .is_some_and(|owner| !owner.eq_ignore_ascii_case(&profile.email))
}

pub fn pet_summaries_for_profile(profile: &UserProfile) -> Vec<(String, String)> {
    let mut pets = Vec::new();
    if profile_has_pet(profile) {
        pets.push((PRIMARY_PET_ID.to_string(), profile.pet_name.clone()));
    }
    for pet in &profile.additional_pets {
        if household_pet_is_complete(pet) {
            pets.push((pet.id.clone(), pet.pet_name.clone()));
        }
    }
    pets
}

pub fn owner_has_pet(profile: &UserProfile, pet_id: &str) -> bool {
    pet_snapshot(profile, pet_id).is_some()
}

fn stored_friend_request(req: StoredFriendRequest) -> FriendRequest {
    FriendRequest {
        id: req.id,
        from_email: req.from_email,
        to_email: req.to_email,
        status: req.status,
        created_at: req.created_at,
    }
}

fn stored_friend_summary(summary: StoredFriendSummary) -> FriendSummary {
    FriendSummary {
        friend_email: summary.friend_email,
    }
}

fn stored_pet_share(share: StoredPetShare) -> PetShare {
    PetShare {
        id: share.id,
        owner_email: share.owner_email,
        shared_with_email: share.shared_with_email,
        pet_id: share.pet_id,
        status: share.status,
        created_at: share.created_at,
    }
}

pub fn load_profile_by_email(state: &AppState, email: &str) -> Option<UserProfile> {
    state
        .storage
        .load_profile(email)
        .ok()
        .flatten()
        .map(|mut profile| {
            profile.email = email.to_string();
            profile
        })
}

pub fn pet_name_for_owner(state: &AppState, owner_email: &str, pet_id: &str) -> String {
    load_profile_by_email(state, owner_email)
        .and_then(|profile| pet_snapshot(&profile, pet_id))
        .map(|snapshot| snapshot.pet_name)
        .unwrap_or_else(|| "Cat".to_string())
}

pub fn list_accessible_pets(state: &AppState, viewer: &UserProfile) -> Vec<AccessiblePetSummary> {
    let mut pets = Vec::new();
    for (pet_id, pet_name) in pet_summaries_for_profile(viewer) {
        pets.push(AccessiblePetSummary {
            pet_id,
            pet_name,
            owner_email: viewer.email.clone(),
            owner_label: "You".to_string(),
            is_owned: true,
        });
    }

    if let Ok(shares) = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
    {
        for share in shares.into_iter().map(stored_pet_share) {
            let Some(owner_profile) = load_profile_by_email(state, &share.owner_email) else {
                continue;
            };
            let Some(snapshot) = pet_snapshot(&owner_profile, &share.pet_id) else {
                continue;
            };
            pets.push(AccessiblePetSummary {
                pet_id: share.pet_id,
                pet_name: snapshot.pet_name,
                owner_email: share.owner_email.clone(),
                owner_label: user_label(state, &share.owner_email),
                is_owned: false,
            });
        }
    }

    pets
}

pub fn can_access_shared_pet(
    state: &AppState,
    viewer_email: &str,
    owner_email: &str,
    pet_id: &str,
) -> bool {
    if viewer_email.eq_ignore_ascii_case(owner_email) {
        return true;
    }
    state
        .storage
        .has_accepted_pet_share(owner_email, viewer_email, pet_id)
        .unwrap_or(false)
}

pub fn resolve_pet_care_target(
    state: &AppState,
    viewer: &UserProfile,
    pet_id: &str,
    owner_email: Option<&str>,
) -> Option<PetCareTarget> {
    let pet_id = pet_id.trim();
    if pet_id.is_empty() {
        return None;
    }

    if let Some(owner) = owner_email {
        let owner = normalize_email(owner);
        if owner == normalize_email(&viewer.email) {
            if owner_has_pet(viewer, pet_id) {
                return Some(PetCareTarget {
                    viewer_email: viewer.email.clone(),
                    owner_email: viewer.email.clone(),
                    pet_id: pet_id.to_string(),
                    is_shared: false,
                });
            }
            return None;
        }
        if !can_access_shared_pet(state, &viewer.email, &owner, pet_id) {
            return None;
        }
        let owner_profile = load_profile_by_email(state, &owner)?;
        if !owner_has_pet(&owner_profile, pet_id) {
            return None;
        }
        return Some(PetCareTarget {
            viewer_email: viewer.email.clone(),
            owner_email: owner,
            pet_id: pet_id.to_string(),
            is_shared: true,
        });
    }

    if owner_has_pet(viewer, pet_id) {
        return Some(PetCareTarget {
            viewer_email: viewer.email.clone(),
            owner_email: viewer.email.clone(),
            pet_id: pet_id.to_string(),
            is_shared: false,
        });
    }

    None
}

pub fn resolve_active_pet_care_target(
    state: &AppState,
    viewer: &UserProfile,
) -> Option<PetCareTarget> {
    let owner = active_pet_owner_email(viewer);
    resolve_pet_care_target(state, viewer, &viewer.active_pet_id, Some(owner))
}

pub fn set_active_pet_selection(
    profile: &mut UserProfile,
    pet_id: &str,
    owner_email: Option<&str>,
) -> bool {
    let owner = owner_email
        .map(normalize_email)
        .filter(|value| !value.is_empty());
    let changed = profile.active_pet_id != pet_id
        || profile.active_pet_owner_email.as_deref() != owner.as_deref();
    profile.active_pet_id = pet_id.to_string();
    profile.active_pet_owner_email = owner;
    changed
}

pub fn accessible_pet_exists(
    state: &AppState,
    viewer: &UserProfile,
    pet_id: &str,
    owner_email: Option<&str>,
) -> bool {
    resolve_pet_care_target(state, viewer, pet_id, owner_email).is_some()
}

pub fn apply_snapshot_to_profile_view(mut view: UserProfile, snapshot: &crate::PetSnapshot) -> UserProfile {
    view.pet_name = snapshot.pet_name.clone();
    view.pet_breed = snapshot.pet_breed.clone();
    view.pet_color = snapshot.pet_color.clone();
    view.pet_mood = snapshot.pet_mood.clone();
    view.pet_age_weeks = snapshot.pet_age_weeks;
    view.pet_age_years = snapshot.pet_age_years;
    view.pet_birth_date = snapshot.pet_birth_date.clone();
    view.last_vet_date = snapshot.last_vet_date.clone();
    view.never_been_to_vet = snapshot.never_been_to_vet;
    view.pet_conditions = snapshot.pet_conditions.clone();
    view.pet_medications = snapshot.pet_medications.clone();
    view.pet_indoor_outdoor = snapshot.pet_indoor_outdoor.clone();
    view.vaccine_history = snapshot.vaccine_history.clone();
    view.pet_vaccines_unknown = snapshot.pet_vaccines_unknown;
    view.care_schedule = snapshot.care_schedule.clone();
    view.pet_photo_url = snapshot.pet_photo_url.clone();
    view.pet_video_url = snapshot.pet_video_url.clone();
    view.pet_video_clip_start = snapshot.pet_video_clip_start;
    view.pet_video_clip_duration = snapshot.pet_video_clip_duration;
    view.pet_video_zoom = snapshot.pet_video_zoom;
    view.pet_video_offset_x = snapshot.pet_video_offset_x;
    view.pet_video_offset_y = snapshot.pet_video_offset_y;
    view.deceased = snapshot.deceased;
    view.deceased_at = snapshot.deceased_at.clone();
    view.memorial_videos = snapshot.memorial_videos.clone();
    view.memorial_comfort_seen = snapshot.memorial_comfort_seen;
    view
}

pub fn account_tab_pet_view(state: &AppState, profile: &UserProfile) -> UserProfile {
    let mut scoped = profile.clone();
    if is_viewing_shared_pet(&scoped) || !owner_has_pet(&scoped, &scoped.active_pet_id) {
        if let Some((pet_id, _)) = pet_summaries_for_profile(&scoped).first() {
            scoped.active_pet_id = pet_id.clone();
            scoped.active_pet_owner_email = None;
        }
    }
    active_pet_view_profile(state, &scoped)
}

pub fn active_pet_view_profile(state: &AppState, viewer: &UserProfile) -> UserProfile {
    let Some(target) = resolve_active_pet_care_target(state, viewer) else {
        return viewer.clone();
    };
    let owner_profile = if target.is_shared {
        load_profile_by_email(state, &target.owner_email).unwrap_or_else(|| viewer.clone())
    } else {
        viewer.clone()
    };
    let Some(snapshot) = pet_snapshot(&owner_profile, &target.pet_id) else {
        return viewer.clone();
    };
    let mut view = owner_profile;
    view.active_pet_id = target.pet_id.clone();
    view.active_pet_owner_email = if target.is_shared {
        Some(target.owner_email.clone())
    } else {
        None
    };
    apply_snapshot_to_profile_view(view, &snapshot)
}

pub fn tasks_view_profile(state: &AppState, viewer: &UserProfile) -> UserProfile {
    let Some(target) = resolve_active_pet_care_target(state, viewer) else {
        return viewer.clone();
    };
    if !target.is_shared {
        return viewer.clone();
    }
    let Some(mut owner) = load_profile_by_email(state, &target.owner_email) else {
        return viewer.clone();
    };
    owner.active_pet_id = target.pet_id.clone();
    owner.active_pet_owner_email = Some(target.owner_email.clone());
    owner.tasks = owner
        .tasks
        .iter()
        .filter(|task| task.pet_id == target.pet_id)
        .cloned()
        .collect();
    owner
}

pub fn calendar_view_profile(state: &AppState, viewer: &UserProfile) -> UserProfile {
    let mut merged = viewer.clone();

    if let Ok(shares) = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
    {
        for share in shares {
            let Some(owner) = load_profile_by_email(state, &share.owner_email) else {
                continue;
            };
            for task in &owner.tasks {
                if task.pet_id == share.pet_id {
                    merged.tasks.push(task.clone());
                }
            }
            merged
                .user_calendar_events
                .extend(owner.user_calendar_events.clone());
        }
    }

    merged
}

pub fn visible_calendar_events_for_viewer(
    state: &AppState,
    viewer: &UserProfile,
    reference_date: NaiveDate,
) -> Vec<CalendarEvent> {
    let mut events = visible_calendar_events(viewer, reference_date);
    let today = Local::now().date_naive();
    let horizon = today + Duration::days(CALENDAR_PREVIEW_HORIZON_DAYS);

    if let Ok(shares) = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
    {
        for share in shares {
            let Some(owner) = load_profile_by_email(state, &share.owner_email) else {
                continue;
            };
            events.extend(owner.user_calendar_events.iter().cloned());
            let premium =
                crate::entitlements::can_access_health_records(owner.premium_unlocked, &owner.email);
            let Some(snapshot) = pet_snapshot(&owner, &share.pet_id) else {
                continue;
            };
            if premium {
                events.extend(crate::generate_vet_calendar_events_for_snapshot(
                    &snapshot,
                    reference_date,
                ));
                events.extend(crate::generate_vaccine_calendar_events_for_snapshot(
                    &snapshot,
                    reference_date,
                ));
            }
            events.extend(crate::generate_birthday_calendar_events_for_snapshot(
                &snapshot, today,
            ));
            events.extend(crate::generate_daily_care_calendar_events_for_snapshot(
                &snapshot, today, horizon,
            ));
        }
    }

    events.sort_by(|left, right| {
        (
            left.year,
            left.month,
            left.day,
            left.time_minutes,
            &left.title,
        )
            .cmp(&(
                right.year,
                right.month,
                right.day,
                right.time_minutes,
                &right.title,
            ))
    });
    events
}

pub fn render_shared_pet_banner(state: &AppState, viewer: &UserProfile) -> String {
    if !is_viewing_shared_pet(viewer) {
        return String::new();
    }
    let owner = active_pet_owner_email(viewer);
    let pet_name = pet_name_for_owner(state, owner, &viewer.active_pet_id);
    format!(
        r#"<p class="shared-pet-banner" role="status">Helping care for <strong>{pet}</strong> · shared by {owner}</p>"#,
        pet = escape_html(&pet_name),
        owner = escape_html(&user_label(state, owner)),
    )
}

pub fn render_pet_switcher(state: &AppState, profile: &UserProfile, return_tab: &str) -> String {
    let pets = list_accessible_pets(state, profile);
    if pets.len() <= 1 {
        return String::new();
    }

    let active_id = profile.active_pet_id.as_str();
    let active_owner = active_pet_owner_email(profile);
    let active_index = pets
        .iter()
        .position(|pet| pet.pet_id == active_id && pet.owner_email == active_owner)
        .unwrap_or(0);
    let prev_idx = if active_index == 0 {
        pets.len() - 1
    } else {
        active_index - 1
    };
    let next_idx = (active_index + 1) % pets.len();
    let prev = &pets[prev_idx];
    let next = &pets[next_idx];

    let tabs = pets
        .iter()
        .map(|pet| {
            let active = pet.pet_id == active_id && pet.owner_email == active_owner;
            let active_class = if active {
                " pet-switcher-tab-active"
            } else {
                ""
            };
            let angel = memorial::pet_switcher_angel_suffix(profile, &pet.pet_id, &pet.owner_email);
            let label = if pet.is_owned {
                format!("{}{}", escape_html(&pet.pet_name), angel)
            } else {
                format!(
                    "{} · {}",
                    escape_html(&pet.pet_name),
                    escape_html(&pet.owner_label)
                )
            };
            let href = if return_tab == "pet" {
                format!(
                    "/home?tab=pet&amp;pet={pet_id}&amp;pet_owner={owner}",
                    pet_id = escape_html_attr(&pet.pet_id),
                    owner = escape_html_attr(&pet.owner_email),
                )
            } else {
                format!(
                    "/home?tab={tab}&amp;pet={pet_id}&amp;pet_owner={owner}",
                    tab = escape_html_attr(return_tab),
                    pet_id = escape_html_attr(&pet.pet_id),
                    owner = escape_html_attr(&pet.owner_email),
                )
            };
            format!(
                r#"<a href="{href}" class="pet-switcher-tab{active_class}" aria-current="{current}">{label}</a>"#,
                href = href,
                active_class = active_class,
                current = if active { "page" } else { "false" },
                label = label,
            )
        })
        .collect::<String>();

    format!(
        r#"<nav class="pet-switcher" aria-label="Switch cat" data-return-tab="{return_tab}">
  <button type="button" class="pet-switcher-nav" data-pet-target="{prev_id}" data-pet-owner="{prev_owner}" aria-label="Previous cat">‹</button>
  <div class="pet-switcher-tabs">{tabs}</div>
  <button type="button" class="pet-switcher-nav" data-pet-target="{next_id}" data-pet-owner="{next_owner}" aria-label="Next cat">›</button>
  <p class="field-hint pet-switcher-count">{position} of {total} cats</p>
</nav>"#,
        return_tab = escape_html_attr(return_tab),
        prev_id = escape_html_attr(&prev.pet_id),
        prev_owner = escape_html_attr(&prev.owner_email),
        next_id = escape_html_attr(&next.pet_id),
        next_owner = escape_html_attr(&next.owner_email),
        tabs = tabs,
        position = active_index + 1,
        total = pets.len(),
    )
}

pub fn render_account_pet_switcher(state: &AppState, profile: &UserProfile) -> String {
    let pets = pet_summaries_for_profile(profile);
    if pets.len() <= 1 {
        return String::new();
    }

    let account_view = account_tab_pet_view(state, profile);
    let active_id = account_view.active_pet_id.as_str();
    let active_index = pets
        .iter()
        .position(|(id, _)| id == active_id)
        .unwrap_or(0);
    let prev_idx = if active_index == 0 {
        pets.len() - 1
    } else {
        active_index - 1
    };
    let next_idx = (active_index + 1) % pets.len();
    let (prev_id, _) = &pets[prev_idx];
    let (next_id, _) = &pets[next_idx];

    let tabs = pets
        .iter()
        .map(|(pet_id, pet_name)| {
            let active = pet_id == active_id;
            let active_class = if active {
                " pet-switcher-tab-active"
            } else {
                ""
            };
            let angel = memorial::pet_switcher_angel_suffix(profile, pet_id, &profile.email);
            format!(
                r#"<a href="/home?tab=account&amp;pet={pet_id}" class="pet-switcher-tab{active_class}" aria-current="{current}">{label}</a>"#,
                pet_id = escape_html_attr(pet_id),
                active_class = active_class,
                current = if active { "page" } else { "false" },
                label = format!("{}{}", escape_html(pet_name), angel),
            )
        })
        .collect::<String>();

    format!(
        r#"<nav class="pet-switcher account-pet-switcher" aria-label="Switch cat on account" data-return-tab="account">
  <button type="button" class="pet-switcher-nav" data-pet-target="{prev_id}" aria-label="Previous cat">‹</button>
  <div class="pet-switcher-tabs">{tabs}</div>
  <button type="button" class="pet-switcher-nav" data-pet-target="{next_id}" aria-label="Next cat">›</button>
  <p class="field-hint pet-switcher-count">{position} of {total} cats</p>
</nav>"#,
        prev_id = escape_html_attr(prev_id),
        next_id = escape_html_attr(next_id),
        tabs = tabs,
        position = active_index + 1,
        total = pets.len(),
    )
}

fn render_pet_share_options(owned_pets: &[(String, String)]) -> String {
    if owned_pets.is_empty() {
        return r#"<option value="">Set up a cat first</option>"#.to_string();
    }
    owned_pets
        .iter()
        .map(|(pet_id, pet_name)| {
            format!(
                r#"<option value="{}">{}</option>"#,
                escape_html_attr(pet_id),
                escape_html(pet_name),
            )
        })
        .collect()
}

pub fn render_tasks_shared_banner(profile: &UserProfile, state: &AppState) -> String {
    if !is_viewing_shared_pet(profile) {
        return String::new();
    }
    let owner = active_pet_owner_email(profile);
    let pet_name = pet_name_for_owner(state, owner, &profile.active_pet_id);
    format!(
        r#"<p class="shared-care-banner" role="status">You can complete <strong>{pet}</strong>'s care tasks and earn paw points — changes save to {owner}'s schedule.</p>"#,
        pet = escape_html(&pet_name),
        owner = escape_html(&user_label(state, owner)),
    )
}

pub fn render_calendar_shared_banner(state: &AppState, viewer: &UserProfile) -> String {
    let accepted = state
        .storage
        .list_accepted_pet_shares_for_recipient(&viewer.email)
        .unwrap_or_default();
    if accepted.is_empty() && !is_viewing_shared_pet(viewer) {
        return String::new();
    }
    if is_viewing_shared_pet(viewer) {
        let owner = active_pet_owner_email(viewer);
        let pet_name = pet_name_for_owner(state, owner, &viewer.active_pet_id);
        return format!(
            r#"<p class="shared-care-banner" role="status">Calendar shows <strong>{pet}</strong>'s feeding schedule, vet reminders, and shared events from {owner}.</p>"#,
            pet = escape_html(&pet_name),
            owner = escape_html(&user_label(state, owner)),
        );
    }
    format!(
        r#"<p class="shared-care-banner" role="status">Your calendar also includes care schedules and vet reminders for <strong>{}</strong> shared cat{}.</p>"#,
        accepted.len(),
        if accepted.len() == 1 { "" } else { "s" }
    )
}

pub fn render_parent_profile_friends_section(
    state: &AppState,
    subject_email: &str,
    is_self: bool,
) -> String {
    let friends = state
        .storage
        .list_friends(subject_email)
        .unwrap_or_default();
    if friends.is_empty() {
        if is_self {
            return r#"<section class="parent-profile-friends-section dashboard-card">
  <h2 class="parent-profile-friends-title">Friends</h2>
  <div class="parent-profile-friends-empty-state">
    <p class="field-hint parent-profile-friends-empty">No friends yet — find other cat parents and send a sweet invite.</p>
    <a href="/home?tab=friends" class="community-friends-cta">Find cat parents on Friends tab 🐾</a>
  </div>
</section>"#
                .to_string();
        }
        return String::new();
    }

    let items: String = friends
        .iter()
        .map(|friend| {
            let email = &friend.friend_email;
            let username = user_for_email(state, email)
                .map(|user| user.username)
                .unwrap_or_else(|| user_label(state, email));
            let photo = user_profile_photo_src(state, email);
            let profile_url = format!(
                "/home?tab=profile&parent={}",
                urlencoding::encode(username.trim())
            );
            format!(
                r#"<li class="parent-profile-friend-item">
  <a class="parent-profile-friend-link" href="{profile_url}">
    <img class="parent-profile-friend-photo" src="{photo}" alt="" width="56" height="56" loading="lazy" />
    <span class="parent-profile-friend-name">{label}</span>
  </a>
</li>"#,
                profile_url = escape_html_attr(&profile_url),
                photo = escape_html_attr(&photo),
                label = escape_html(&user_label(state, email)),
            )
        })
        .collect();

    format!(
        r#"<section class="parent-profile-friends-section dashboard-card">
  <h2 class="parent-profile-friends-title">Friends</h2>
  <p class="field-hint parent-profile-friends-intro">{intro}</p>
  <ul class="parent-profile-friends-list">{items}</ul>
</section>"#,
        intro = if is_self {
            "Cat parents you're connected with on WhiskerWatch."
        } else {
            "Cat parents connected with this profile."
        },
        items = items,
    )
}

pub fn render_account_friends_section(
    state: &AppState,
    viewer_email: &str,
    owned_pets: &[(String, String)],
) -> String {
    format!(
        "{}{}{}",
        render_friends_card(state, viewer_email),
        render_friend_messages_card(state, viewer_email),
        render_pet_sharing_card(state, viewer_email, owned_pets),
    )
}

fn render_friends_card(state: &AppState, viewer_email: &str) -> String {
    let friends = state
        .storage
        .list_friends(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_summary)
        .collect::<Vec<_>>();
    let incoming = state
        .storage
        .list_incoming_friend_requests(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_request)
        .collect::<Vec<_>>();
    let outgoing = state
        .storage
        .list_outgoing_friend_requests(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_request)
        .collect::<Vec<_>>();
    let incoming_friend_html: String = incoming
        .iter()
        .map(|req| {
            format!(
                r#"<li class="friend-request-item"><span>{from}</span>
  <form action="/home/friends/respond" method="post" class="inline-action-form">
    <input type="hidden" name="request_id" value="{id}" />
    <input type="hidden" name="action" value="accept" />
    <button type="submit" class="download-btn">Accept</button>
  </form>
  <form action="/home/friends/respond" method="post" class="inline-action-form">
    <input type="hidden" name="request_id" value="{id}" />
    <input type="hidden" name="action" value="decline" />
    <button type="submit" class="onboarding-skip-btn">Decline</button>
  </form></li>"#,
                from = escape_html(&user_label(state, &req.from_email)),
                id = escape_html_attr(&req.id),
            )
        })
        .collect();

    let pending_friend_items: String = outgoing
        .iter()
        .map(|req| {
            let label = user_label(state, &req.to_email);
            let photo = user_profile_photo_src(state, &req.to_email);
            let hint = pending_friend_hint(state, &req.to_email);
            format!(
                r#"<li class="friend-list-item friend-list-item-pending">
  <img class="friend-pending-photo" src="{photo}" alt="" width="48" height="48" loading="lazy" />
  <div class="friend-pending-meta">
    <strong class="friend-pending-name">{label}</strong>
    <span class="friend-pending-hint">{hint}</span>
  </div>
  <span class="friend-pending-badge" aria-label="Friend invite sent">💌 Invite sent</span>
</li>"#,
                label = escape_html(&label),
                photo = escape_html_attr(&photo),
                hint = escape_html(&hint),
            )
        })
        .collect();

    let accepted_friend_items: String = friends
        .iter()
        .map(|friend| {
            let label = user_label(state, &friend.friend_email);
            format!(
                r#"<li class="friend-list-item">
  <strong>{label}</strong>
  <button type="button" class="friend-list-message-btn onboarding-skip-btn" data-open-friend-chat="{email}">Message</button>
</li>"#,
                label = escape_html(&label),
                email = escape_html_attr(&friend.friend_email),
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let friends_list_html = if friends.is_empty() && outgoing.is_empty() {
        r#"<p class="friends-empty-hint">No pals yet — tap <button type="button" class="friends-empty-cta" data-open-friends-add>Find cat parents</button> above and search by username. 🐱💕</p>"#.to_string()
    } else {
        let pending_intro = if outgoing.is_empty() {
            String::new()
        } else {
            r#"<p class="friend-list-pending-intro">On their way 💌</p>"#.to_string()
        };
        format!(
            "{pending_intro}<ul class=\"friend-list\">{pending_friend_items}{accepted_friend_items}</ul>",
        )
    };

    let add_friends_open = if friends.is_empty() && incoming.is_empty() {
        " open"
    } else {
        ""
    };

    format!(
        r#"<article class="dashboard-card friends-sharing-card">
  <details id="friends-add-card" class="friends-add-card"{add_friends_open}>
    <summary class="friends-add-summary">
      <span class="friends-add-summary-badge" aria-hidden="true">🐾</span>
      <span class="friends-add-summary-copy">
        <span class="friends-add-summary-text">Find cat parents</span>
        <span class="friends-add-summary-hint">Search by username &amp; send a friend invite</span>
      </span>
    </summary>
    <div class="friends-add-body">
      <p class="friends-add-intro">Look up a fellow cat parent, pick the right profile, and send a sweet invite. Once you're connected, you can message, share posts, and swap cat care schedules. ✨</p>
      <form class="login-form add-friend-form" action="/home/friends/request" method="post" data-friend-search-form>
        <div class="friend-search-field">
          <label class="friends-add-search-label" for="friend_search_query">
            <span class="friends-add-search-label-icon" aria-hidden="true">🔎</span>
            Who should we look up?
          </label>
          <div class="friends-add-search-wrap">
            <input id="friend_search_query" type="search" autocomplete="off" placeholder="Start typing a username…" aria-controls="friend_search_results" aria-expanded="false" aria-autocomplete="list" />
          </div>
          <div id="friend_search_results" class="friend-search-results" role="listbox" aria-label="Matching users" hidden></div>
        </div>
        <div id="friend_search_selected" class="friend-search-selected friends-add-selected" hidden>
          <span class="friends-add-selected-badge" aria-hidden="true">💕</span>
          <img class="friend-search-selected-photo" alt="" width="48" height="48" />
          <div class="friend-search-selected-meta">
            <strong class="friend-search-selected-name"></strong>
            <span class="field-hint friend-search-selected-pet"></span>
          </div>
          <button type="button" class="friend-search-clear onboarding-skip-btn">Pick someone else</button>
        </div>
        <input id="friend_email" name="friend_email" type="hidden" value="" required />
        <button type="submit" class="download-btn login-submit friends-add-submit" id="friend_request_submit" disabled>Send friend invite 💌</button>
      </form>
    </div>
  </details>
  {incoming_friends}
  <h3 class="friends-subhead">Your friends</h3>
  {friends_list}
</article>"#,
        add_friends_open = add_friends_open,
        incoming_friends = if incoming_friend_html.is_empty() {
            String::new()
        } else {
            format!(
                "<h3 class=\"friends-subhead\">Friend requests for you</h3><ul class=\"friend-request-list\">{incoming_friend_html}</ul>"
            )
        },
        friends_list = friends_list_html,
    )
}

fn render_pet_sharing_card(
    state: &AppState,
    viewer_email: &str,
    owned_pets: &[(String, String)],
) -> String {
    let friends = state
        .storage
        .list_friends(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_friend_summary)
        .collect::<Vec<_>>();
    let incoming_shares = state
        .storage
        .list_incoming_pet_shares(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_pet_share)
        .collect::<Vec<_>>();
    let accepted_shared = state
        .storage
        .list_accepted_pet_shares_for_recipient(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_pet_share)
        .collect::<Vec<_>>();
    let outgoing_shares = state
        .storage
        .list_outgoing_pet_shares(viewer_email)
        .unwrap_or_default()
        .into_iter()
        .map(stored_pet_share)
        .collect::<Vec<_>>();

    let pet_options = render_pet_share_options(owned_pets);

    let friend_options: String = if friends.is_empty() {
        r#"<option value="">Add a friend first</option>"#.to_string()
    } else {
        friends
            .iter()
            .map(|friend| {
                format!(
                    r#"<option value="{}">{}</option>"#,
                    escape_html_attr(&friend.friend_email),
                    escape_html(&user_label(state, &friend.friend_email)),
                )
            })
            .collect()
    };

    let per_friend_share_html = if owned_pets.is_empty() || friends.is_empty() {
        String::new()
    } else {
        format!(
            "<ul class=\"friend-share-list\">{}</ul>",
            friends
                .iter()
                .map(|friend| {
                    format!(
                        r#"<li class="friend-share-item">
  <span class="friend-share-label">Share with <strong>{label}</strong></span>
  <form class="login-form friend-quick-share-form" action="/home/pets/share" method="post">
    <input type="hidden" name="friend_email" value="{email}" />
    <label class="visually-hidden" for="share_pet_{email_id}">Cat to share with {label}</label>
    <select id="share_pet_{email_id}" name="pet_id" required>{pet_options}</select>
    <button type="submit" class="download-btn">Share tasks &amp; schedule</button>
  </form>
</li>"#,
                        label = escape_html(&user_label(state, &friend.friend_email)),
                        email = escape_html_attr(&friend.friend_email),
                        email_id = escape_html_attr(&friend.friend_email.replace('@', "_at_")),
                        pet_options = pet_options,
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    let share_form = if owned_pets.is_empty() {
        r#"<p class="field-hint">Set up a cat on the My Pet tab before sharing care schedules.</p>"#.to_string()
    } else if friends.is_empty() {
        r#"<p class="field-hint">Connect with another cat parent first, then you can share a specific cat's tasks, feeding schedule, and calendar.</p>
<a href="/home?tab=friends" class="community-friends-cta friends-open-add-card" data-open-friends-add>Find cat parents 🐾</a>"#.to_string()
    } else {
        format!(
            r#"<form class="login-form share-cat-form" action="/home/pets/share" method="post">
  <label for="share_friend_email">Share with</label>
  <select id="share_friend_email" name="friend_email" required>{friend_options}</select>
  <label for="share_pet_id">Which cat</label>
  <select id="share_pet_id" name="pet_id" required>{pet_options}</select>
  <p class="field-hint">Your friend will see this cat's care tasks, feeding times, vet reminders, health records, and calendar events. They can complete tasks on your behalf.</p>
  <button type="submit" class="download-btn login-submit">Share cat care 🐾</button>
</form>
{per_friend_share}"#,
            friend_options = friend_options,
            pet_options = pet_options,
            per_friend_share = if per_friend_share_html.is_empty() {
                String::new()
            } else {
                format!(
                    "<h3 class=\"friends-subhead\">Quick share with a friend</h3>{per_friend_share_html}"
                )
            },
        )
    };

    let incoming_share_html: String = incoming_shares
        .iter()
        .map(|share| {
            let cat_name = pet_name_for_owner(state, &share.owner_email, &share.pet_id);
            format!(
                r#"<li class="share-request-item">
  <div class="share-request-copy">
    <strong>{cat}</strong> from {owner}
    <span class="field-hint">Includes care tasks, feeding schedule, and calendar reminders</span>
  </div>
  <form action="/home/pets/share/respond" method="post" class="inline-action-form">
    <input type="hidden" name="share_id" value="{id}" />
    <input type="hidden" name="action" value="accept" />
    <button type="submit" class="download-btn">Accept care access</button>
  </form>
  <form action="/home/pets/share/respond" method="post" class="inline-action-form">
    <input type="hidden" name="share_id" value="{id}" />
    <input type="hidden" name="action" value="decline" />
    <button type="submit" class="onboarding-skip-btn">Decline</button>
  </form>
</li>"#,
                cat = escape_html(&cat_name),
                owner = escape_html(&user_label(state, &share.owner_email)),
                id = escape_html_attr(&share.id),
            )
        })
        .collect();

    let shared_with_me_html = if accepted_shared.is_empty() {
        r#"<p class="field-hint">No cats shared with you yet. When a friend shares, you'll see their tasks and schedule here.</p>"#.to_string()
    } else {
        format!(
            "<ul class=\"shared-pet-list\">{}</ul>",
            accepted_shared
                .iter()
                .map(|share| {
                    let cat_name = pet_name_for_owner(state, &share.owner_email, &share.pet_id);
                    let owner_label = user_label(state, &share.owner_email);
                    format!(
                        r#"<li class="shared-pet-list-item">
  <a class="shared-pet-main-link" href="/home?tab=pet&amp;pet={pet_id}&amp;pet_owner={owner}"><strong>{cat}</strong> · shared by {owner_label}</a>
  <span class="shared-pet-links">
    <a class="shared-pet-link" href="/home?tab=tasks&amp;pet={pet_id}&amp;pet_owner={owner}">Tasks</a>
    <a class="shared-pet-link" href="/home?tab=calendar&amp;pet={pet_id}&amp;pet_owner={owner}">Calendar</a>
    <a class="shared-pet-link" href="/home?tab=health&amp;pet={pet_id}&amp;pet_owner={owner}">Health</a>
  </span>
</li>"#,
                        pet_id = escape_html_attr(&share.pet_id),
                        owner = escape_html_attr(&share.owner_email),
                        cat = escape_html(&cat_name),
                        owner_label = escape_html(&owner_label),
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    let outgoing_share_html = if outgoing_shares.is_empty() {
        r#"<p class="field-hint">You have not shared any cats yet.</p>"#.to_string()
    } else {
        format!(
            "<ul class=\"outgoing-share-list\">{}</ul>",
            outgoing_shares
                .iter()
                .map(|share| {
                    let cat_name = pet_name_for_owner(state, viewer_email, &share.pet_id);
                    let status_label = if share.status == SHARE_STATUS_PENDING {
                        "Invite pending"
                    } else {
                        "Sharing tasks & schedule"
                    };
                    format!(
                        r#"<li class="outgoing-share-item">
  <div class="outgoing-share-copy">
    <strong>{cat}</strong> with {friend}
    <span class="field-hint">{status}</span>
  </div>
  <form action="/home/pets/share/revoke" method="post" class="inline-action-form">
    <input type="hidden" name="share_id" value="{id}" />
    <button type="submit" class="onboarding-skip-btn">Stop sharing</button>
  </form>
</li>"#,
                        cat = escape_html(&cat_name),
                        friend = escape_html(&user_label(state, &share.shared_with_email)),
                        status = escape_html(status_label),
                        id = escape_html_attr(&share.id),
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };

    format!(
        r#"<article class="dashboard-card pet-sharing-card">
  <h2>Share pets, tasks &amp; schedules</h2>
  <p class="field-hint">Pick a specific cat to share its care tasks, feeding times, vet reminders, and calendar with a friend — perfect for co-parents, sitters, and family helpers.</p>
  {share_form}
  <h3 class="friends-subhead">Cats you're sharing</h3>
  {outgoing_shares}
  <h3 class="friends-subhead">Cats shared with you</h3>
  {shared_with_me}
  {incoming_shares}
</article>"#,
        share_form = share_form,
        outgoing_shares = outgoing_share_html,
        shared_with_me = shared_with_me_html,
        incoming_shares = if incoming_share_html.is_empty() {
            String::new()
        } else {
            format!(
                "<h3 class=\"friends-subhead\">Care share invites for you</h3><ul class=\"share-request-list\">{incoming_share_html}</ul>"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::User;

    fn test_state_with_users() -> AppState {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = crate::storage::Storage::open_at(temp.path().to_path_buf()).expect("storage");
        let state = AppState { storage };
        state
            .storage
            .save_user(&User {
                username: "mochi_mom".to_string(),
                first_name: "Mochi".to_string(),
                last_name: "Mom".to_string(),
                email: "mom@example.com".to_string(),
                password: "secret1!".to_string(),
                created_at: 1,
            })
            .expect("save mom");
        state
            .storage
            .save_user(&User {
                username: "catdad".to_string(),
                first_name: "Cat".to_string(),
                last_name: "Dad".to_string(),
                email: "dad@example.com".to_string(),
                password: "secret2!".to_string(),
                created_at: 2,
            })
            .expect("save dad");
        state
    }

    #[test]
    fn resolve_friend_identifier_accepts_username_or_email() {
        let state = test_state_with_users();
        assert_eq!(
            resolve_friend_identifier(&state, "catdad").expect("username"),
            "dad@example.com"
        );
        assert_eq!(
            resolve_friend_identifier(&state, "DAD@example.com").expect("email"),
            "dad@example.com"
        );
        assert_eq!(
            resolve_friend_identifier(&state, "missing").err(),
            Some(FriendLookupError::NotFound)
        );
    }
}
