use crate::{ContactSubmission, FeedbackComment, FeedbackSubmission, ForumPost, ForumReply, User, UserProfile};
use rusqlite::{params, Connection};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct FeedbackForumEntry {
    pub submission: FeedbackSubmission,
    pub upvotes: u32,
    pub downvotes: u32,
    pub user_vote: Option<i8>,
    #[allow(dead_code)]
    pub reward_granted: bool,
    pub comments: Vec<FeedbackComment>,
}

#[derive(Debug, Clone)]
pub struct PushSubscription {
    pub email: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    #[allow(dead_code)]
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackVoteCounts {
    pub upvotes: u32,
    pub downvotes: u32,
    pub user_vote: Option<i8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ForumDeleteOutcome {
    Deleted,
    NotFound,
    NotAuthorized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFriendRequest {
    pub id: String,
    pub from_email: String,
    pub to_email: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFriendSummary {
    pub friend_email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUserSearchHit {
    pub email: String,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredSocialPostMedia {
    pub media_type: String,
    pub media_url: String,
    pub video_duration: Option<f32>,
    pub sort_order: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredSocialPostComment {
    pub id: String,
    pub post_id: String,
    pub user_id: String,
    pub author_username: String,
    pub body: String,
    pub created_at: u64,
    pub upvotes: u32,
    pub viewer_upvoted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredSocialPost {
    pub id: String,
    pub user_id: String,
    pub author_username: String,
    pub body: String,
    pub media_type: String,
    pub media_url: Option<String>,
    pub video_duration: Option<f32>,
    pub is_private: bool,
    pub created_at: u64,
    pub post_kind: String,
    pub wrapped_payload: Option<String>,
    pub wrapped_year: Option<u32>,
    pub wrapped_month: Option<u32>,
    pub media_items: Vec<StoredSocialPostMedia>,
    pub upvotes: u32,
    pub viewer_upvoted: bool,
    pub comments: Vec<StoredSocialPostComment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialPostUpvoteSummary {
    pub upvotes: u32,
    pub viewer_upvoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialCommentUpvoteSummary {
    pub upvotes: u32,
    pub viewer_upvoted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredFriendMessage {
    pub id: String,
    pub from_email: String,
    pub to_email: String,
    pub body: String,
    pub media_type: String,
    pub media_url: Option<String>,
    pub video_duration: Option<f32>,
    pub created_at: u64,
    pub read_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMessageThread {
    pub id: String,
    pub participant_a: String,
    pub participant_b: String,
    pub status: String,
    pub initiated_by: String,
    pub created_at: u64,
    pub responded_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPetShare {
    pub id: String,
    pub owner_email: String,
    pub shared_with_email: String,
    pub pet_id: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
    PasswordHash(bcrypt::BcryptError),
    EmailTaken,
    UsernameTaken,
    InvalidResetToken,
    InvalidInput(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "database error: {error}"),
            Self::Json(error) => write!(f, "json error: {error}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::PasswordHash(error) => write!(f, "password hash error: {error}"),
            Self::EmailTaken => write!(f, "email already registered"),
            Self::UsernameTaken => write!(f, "username already taken"),
            Self::InvalidResetToken => write!(f, "invalid or expired reset token"),
            Self::InvalidInput(message) => write!(f, "{message}"),
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<bcrypt::BcryptError> for StorageError {
    fn from(error: bcrypt::BcryptError) -> Self {
        Self::PasswordHash(error)
    }
}

#[derive(Clone)]
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
    data_dir: PathBuf,
}

fn map_social_post_row(row: &rusqlite::Row<'_>) -> Result<StoredSocialPost, rusqlite::Error> {
    Ok(StoredSocialPost {
        id: row.get(0)?,
        user_id: row.get(1)?,
        author_username: row.get(2)?,
        body: row.get(3)?,
        media_type: row.get(4)?,
        media_url: row.get(5)?,
        video_duration: row.get::<_, Option<f64>>(6)?.map(|value| value as f32),
        is_private: row.get::<_, i64>(7)? != 0,
        created_at: row.get::<_, i64>(8)? as u64,
        post_kind: row
            .get::<_, Option<String>>(9)?
            .unwrap_or_else(|| "standard".to_string()),
        wrapped_payload: row.get(10)?,
        wrapped_year: row.get::<_, Option<i64>>(11)?.map(|value| value as u32),
        wrapped_month: row.get::<_, Option<i64>>(12)?.map(|value| value as u32),
        media_items: Vec::new(),
        upvotes: 0,
        viewer_upvoted: false,
        comments: Vec::new(),
    })
}

const SOCIAL_POST_SELECT: &str = "id, user_id, author_username, body, media_type, media_url, video_duration, is_private, created_at, post_kind, wrapped_payload, wrapped_year, wrapped_month";

fn map_forum_post_row(row: &rusqlite::Row<'_>) -> Result<ForumPost, rusqlite::Error> {
    Ok(ForumPost {
        id: row.get(0)?,
        user_id: row.get(1)?,
        author_username: row.get(2)?,
        title: row.get(3)?,
        body: row.get(4)?,
        created_at: row.get::<_, i64>(5)? as u64,
        breed_slug: row.get(6)?,
    })
}

fn find_project_root(mut start: PathBuf) -> Option<PathBuf> {
    loop {
        if start.join("Cargo.toml").exists() {
            return Some(start);
        }
        if !start.pop() {
            return None;
        }
    }
}

fn project_root_from_candidates() -> Option<PathBuf> {
    // Baked in at compile time so static/templates resolve regardless of process cwd.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join("Cargo.toml").is_file() {
        return Some(manifest_dir);
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    for start in candidates {
        if let Some(root) = find_project_root(start) {
            return Some(root);
        }
    }
    None
}

/// Directory for CSS, JS, and marketing images (`static/`).
pub fn static_dir() -> PathBuf {
    path_in_project("static")
}

fn default_data_dir() -> PathBuf {
    if let Some(root) = project_root_from_candidates() {
        return root.join("data");
    }

    eprintln!(
        "warning: could not find project root (Cargo.toml); using ./data under the current directory"
    );
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
}

fn resolve_relative_data_dir(dir: PathBuf) -> PathBuf {
    if let Some(root) = project_root_from_candidates() {
        root.join(dir)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(dir)
    }
}

/// Resolve a path relative to the project root (directory containing `Cargo.toml`).
pub fn path_in_project(relative: impl AsRef<Path>) -> PathBuf {
    if let Some(root) = project_root_from_candidates() {
        root.join(relative.as_ref())
    } else {
        relative.as_ref().to_path_buf()
    }
}

pub fn data_dir_from_env() -> PathBuf {
    match std::env::var("DATA_DIR") {
        Ok(path) if !path.trim().is_empty() => {
            let dir = PathBuf::from(path.trim());
            if dir.is_absolute() {
                dir
            } else {
                resolve_relative_data_dir(dir)
            }
        }
        _ => default_data_dir(),
    }
}

fn is_unique_constraint(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => {
            code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                || code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT
        }
        _ => false,
    }
}

fn is_bcrypt_hash(password: &str) -> bool {
    password.starts_with("$2a$") || password.starts_with("$2b$") || password.starts_with("$2y$")
}

fn hash_password(plain: &str) -> Result<String, StorageError> {
    Ok(bcrypt::hash(plain, bcrypt::DEFAULT_COST)?)
}

impl Storage {
    pub fn open() -> Result<Self, StorageError> {
        let data_dir = data_dir_from_env();
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("whiskerwatch.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS users (
                 email TEXT PRIMARY KEY COLLATE NOCASE,
                 username TEXT NOT NULL COLLATE NOCASE,
                 first_name TEXT NOT NULL,
                 last_name TEXT NOT NULL,
                 password TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS user_profiles (
                 email TEXT PRIMARY KEY COLLATE NOCASE,
                 profile_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS contact_messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 email TEXT NOT NULL,
                 subject TEXT NOT NULL,
                 message TEXT NOT NULL,
                 submitted_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS feedback (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 email TEXT NOT NULL,
                 category TEXT NOT NULL,
                 message TEXT NOT NULL,
                 submitted_at INTEGER NOT NULL,
                 user_id TEXT,
                 author_username TEXT NOT NULL DEFAULT '',
                 reward_granted INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS feedback_votes (
                 feedback_id INTEGER NOT NULL,
                 user_id TEXT NOT NULL,
                 vote INTEGER NOT NULL,
                 PRIMARY KEY (feedback_id, user_id),
                 FOREIGN KEY (feedback_id) REFERENCES feedback(id)
             );
             CREATE TABLE IF NOT EXISTS forum_posts (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 user_id TEXT NOT NULL,
                 author_username TEXT NOT NULL,
                 title TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS forum_replies (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 post_id INTEGER NOT NULL,
                 user_id TEXT NOT NULL,
                 author_username TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 FOREIGN KEY (post_id) REFERENCES forum_posts(id)
             );
             CREATE INDEX IF NOT EXISTS idx_forum_replies_post_id
                 ON forum_replies(post_id);
             CREATE TABLE IF NOT EXISTS stripe_fulfilled_sessions (
                 session_id TEXT PRIMARY KEY,
                 user_email TEXT NOT NULL,
                 paw_points INTEGER NOT NULL,
                 fulfilled_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS password_reset_tokens (
                 token TEXT PRIMARY KEY,
                 email TEXT NOT NULL COLLATE NOCASE,
                 expires_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_email
                 ON password_reset_tokens(email);",
        )?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        };
        storage.migrate_user_columns()?;
        storage.migrate_password_reset_tokens_table()?;
        storage.migrate_auth_sessions_table()?;
        storage.migrate_forum_tables()?;
        storage.migrate_forum_breed_slug()?;
        storage.migrate_submission_tables()?;
        storage.migrate_feedback_comments_table()?;
        storage.migrate_push_subscriptions_table()?;
        storage.migrate_social_tables()?;
        storage.migrate_friend_messages_table()?;
        storage.migrate_message_threads_table()?;
        storage.migrate_blocked_users_table()?;
        storage.migrate_social_posts_table()?;
        storage.migrate_social_post_media_table()?;
        storage.migrate_social_post_engagement_tables()?;
        storage.migrate_from_jsonl()?;
        let _ = storage.purge_expired_auth_sessions();
        Ok(storage)
    }

    #[allow(dead_code)]
    pub fn open_at(data_dir: PathBuf) -> Result<Self, StorageError> {
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("whiskerwatch.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS users (
                 email TEXT PRIMARY KEY COLLATE NOCASE,
                 username TEXT NOT NULL COLLATE NOCASE,
                 first_name TEXT NOT NULL,
                 last_name TEXT NOT NULL,
                 password TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS password_reset_tokens (
                 token TEXT PRIMARY KEY,
                 email TEXT NOT NULL COLLATE NOCASE,
                 expires_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_email
                 ON password_reset_tokens(email);",
        )?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        };
        storage.migrate_user_columns()?;
        storage.migrate_password_reset_tokens_table()?;
        storage.migrate_auth_sessions_table()?;
        storage.migrate_forum_tables()?;
        storage.migrate_forum_breed_slug()?;
        storage.migrate_submission_tables()?;
        storage.migrate_feedback_comments_table()?;
        storage.migrate_push_subscriptions_table()?;
        storage.migrate_social_tables()?;
        storage.migrate_friend_messages_table()?;
        storage.migrate_message_threads_table()?;
        storage.migrate_blocked_users_table()?;
        storage.migrate_social_posts_table()?;
        storage.migrate_social_post_media_table()?;
        storage.migrate_social_post_engagement_tables()?;
        storage.migrate_from_jsonl()?;
        let _ = storage.purge_expired_auth_sessions();
        Ok(storage)
    }

    fn migrate_social_tables(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS friend_requests (
                 id TEXT PRIMARY KEY,
                 from_email TEXT NOT NULL COLLATE NOCASE,
                 to_email TEXT NOT NULL COLLATE NOCASE,
                 status TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_friend_requests_to_email
                 ON friend_requests(to_email);
             CREATE INDEX IF NOT EXISTS idx_friend_requests_from_email
                 ON friend_requests(from_email);
             CREATE TABLE IF NOT EXISTS pet_shares (
                 id TEXT PRIMARY KEY,
                 owner_email TEXT NOT NULL COLLATE NOCASE,
                 shared_with_email TEXT NOT NULL COLLATE NOCASE,
                 pet_id TEXT NOT NULL,
                 status TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_pet_shares_recipient
                 ON pet_shares(shared_with_email);
             CREATE INDEX IF NOT EXISTS idx_pet_shares_owner
                 ON pet_shares(owner_email);",
        )?;
        Ok(())
    }

    fn migrate_social_posts_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS social_posts (
                 id TEXT PRIMARY KEY,
                 user_id TEXT NOT NULL COLLATE NOCASE,
                 author_username TEXT NOT NULL,
                 body TEXT NOT NULL DEFAULT '',
                 media_type TEXT NOT NULL,
                 media_url TEXT,
                 video_duration REAL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_social_posts_created
                 ON social_posts(created_at DESC);
             CREATE INDEX IF NOT EXISTS idx_social_posts_user
                 ON social_posts(user_id);",
        )?;
        if !Self::table_has_column(&conn, "social_posts", "is_private")? {
            conn.execute(
                "ALTER TABLE social_posts ADD COLUMN is_private INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !Self::table_has_column(&conn, "social_posts", "post_kind")? {
            conn.execute(
                "ALTER TABLE social_posts ADD COLUMN post_kind TEXT NOT NULL DEFAULT 'standard'",
                [],
            )?;
        }
        if !Self::table_has_column(&conn, "social_posts", "wrapped_payload")? {
            conn.execute("ALTER TABLE social_posts ADD COLUMN wrapped_payload TEXT", [])?;
        }
        if !Self::table_has_column(&conn, "social_posts", "wrapped_year")? {
            conn.execute("ALTER TABLE social_posts ADD COLUMN wrapped_year INTEGER", [])?;
        }
        if !Self::table_has_column(&conn, "social_posts", "wrapped_month")? {
            conn.execute("ALTER TABLE social_posts ADD COLUMN wrapped_month INTEGER", [])?;
        }
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_social_posts_wrapped_month
                 ON social_posts(user_id, wrapped_year, wrapped_month)
                 WHERE post_kind = 'monthly_wrapped';",
        )?;
        Ok(())
    }

    fn migrate_social_post_media_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS social_post_media (
                 id TEXT PRIMARY KEY,
                 post_id TEXT NOT NULL,
                 sort_order INTEGER NOT NULL,
                 media_type TEXT NOT NULL,
                 media_url TEXT NOT NULL,
                 video_duration REAL
             );
             CREATE INDEX IF NOT EXISTS idx_social_post_media_post
                 ON social_post_media(post_id, sort_order);",
        )?;
        Ok(())
    }

    fn migrate_friend_messages_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS friend_messages (
                 id TEXT PRIMARY KEY,
                 from_email TEXT NOT NULL COLLATE NOCASE,
                 to_email TEXT NOT NULL COLLATE NOCASE,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 read_at INTEGER
             );
             CREATE INDEX IF NOT EXISTS idx_friend_messages_participants_created
                 ON friend_messages(from_email, to_email, created_at);
             CREATE INDEX IF NOT EXISTS idx_friend_messages_recipient_read
                 ON friend_messages(to_email, read_at);",
        )?;
        if !Self::table_has_column(&conn, "friend_messages", "media_type")? {
            conn.execute(
                "ALTER TABLE friend_messages ADD COLUMN media_type TEXT NOT NULL DEFAULT 'none'",
                [],
            )?;
        }
        if !Self::table_has_column(&conn, "friend_messages", "media_url")? {
            conn.execute("ALTER TABLE friend_messages ADD COLUMN media_url TEXT", [])?;
        }
        if !Self::table_has_column(&conn, "friend_messages", "video_duration")? {
            conn.execute("ALTER TABLE friend_messages ADD COLUMN video_duration REAL", [])?;
        }
        if !Self::table_has_column(&conn, "friend_messages", "deleted_for_all_at")? {
            conn.execute(
                "ALTER TABLE friend_messages ADD COLUMN deleted_for_all_at INTEGER",
                [],
            )?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS friend_message_hidden (
                 message_id TEXT NOT NULL,
                 user_email TEXT NOT NULL COLLATE NOCASE,
                 deleted_at INTEGER NOT NULL,
                 PRIMARY KEY (message_id, user_email)
             );
             CREATE INDEX IF NOT EXISTS idx_friend_message_hidden_user
                 ON friend_message_hidden(user_email);
             CREATE TABLE IF NOT EXISTS friend_conversation_hidden (
                 user_email TEXT NOT NULL COLLATE NOCASE,
                 partner_email TEXT NOT NULL COLLATE NOCASE,
                 deleted_at INTEGER NOT NULL,
                 PRIMARY KEY (user_email, partner_email)
             );",
        )?;
        Ok(())
    }

    fn migrate_message_threads_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS message_threads (
                 id TEXT PRIMARY KEY,
                 participant_a TEXT NOT NULL COLLATE NOCASE,
                 participant_b TEXT NOT NULL COLLATE NOCASE,
                 status TEXT NOT NULL,
                 initiated_by TEXT NOT NULL COLLATE NOCASE,
                 created_at INTEGER NOT NULL,
                 responded_at INTEGER,
                 UNIQUE(participant_a, participant_b)
             );
             CREATE INDEX IF NOT EXISTS idx_message_threads_participant_a
                 ON message_threads(participant_a);
             CREATE INDEX IF NOT EXISTS idx_message_threads_participant_b
                 ON message_threads(participant_b);",
        )?;
        Ok(())
    }

    fn migrate_blocked_users_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS blocked_users (
                 blocker_email TEXT NOT NULL COLLATE NOCASE,
                 blocked_email TEXT NOT NULL COLLATE NOCASE,
                 created_at INTEGER NOT NULL,
                 PRIMARY KEY (blocker_email, blocked_email)
             );
             CREATE INDEX IF NOT EXISTS idx_blocked_users_blocked
                 ON blocked_users(blocked_email);",
        )?;
        Ok(())
    }

    fn migrate_social_post_engagement_tables(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS social_post_votes (
                 post_id TEXT NOT NULL,
                 user_id TEXT NOT NULL COLLATE NOCASE,
                 created_at INTEGER NOT NULL,
                 PRIMARY KEY (post_id, user_id)
             );
             CREATE INDEX IF NOT EXISTS idx_social_post_votes_post
                 ON social_post_votes(post_id);
             CREATE TABLE IF NOT EXISTS social_post_comments (
                 id TEXT PRIMARY KEY,
                 post_id TEXT NOT NULL,
                 user_id TEXT NOT NULL COLLATE NOCASE,
                 author_username TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_social_post_comments_post
                 ON social_post_comments(post_id, created_at);
             CREATE TABLE IF NOT EXISTS social_comment_votes (
                 comment_id TEXT NOT NULL,
                 user_id TEXT NOT NULL COLLATE NOCASE,
                 created_at INTEGER NOT NULL,
                 PRIMARY KEY (comment_id, user_id)
             );
             CREATE INDEX IF NOT EXISTS idx_social_comment_votes_comment
                 ON social_comment_votes(comment_id);",
        )?;
        Ok(())
    }

    fn migrate_push_subscriptions_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS push_subscriptions (
                 endpoint TEXT PRIMARY KEY,
                 email TEXT NOT NULL COLLATE NOCASE,
                 p256dh TEXT NOT NULL,
                 auth TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_push_subscriptions_email
                 ON push_subscriptions(email);",
        )?;
        Ok(())
    }

    fn migrate_forum_breed_slug(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare("PRAGMA table_info(forum_posts)")?;
        let mut has_breed_slug = false;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for name in rows {
            if name? == "breed_slug" {
                has_breed_slug = true;
                break;
            }
        }

        if !has_breed_slug {
            conn.execute(
                "ALTER TABLE forum_posts ADD COLUMN breed_slug TEXT NOT NULL DEFAULT ''",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_forum_posts_breed_slug ON forum_posts(breed_slug)",
                [],
            )?;
        }
        Ok(())
    }

    fn migrate_submission_tables(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_profiles (
                 email TEXT PRIMARY KEY COLLATE NOCASE,
                 profile_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS contact_messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 email TEXT NOT NULL,
                 subject TEXT NOT NULL,
                 message TEXT NOT NULL,
                 submitted_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS feedback (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 email TEXT NOT NULL,
                 category TEXT NOT NULL,
                 message TEXT NOT NULL,
                 submitted_at INTEGER NOT NULL,
                 user_id TEXT,
                 author_username TEXT NOT NULL DEFAULT '',
                 reward_granted INTEGER NOT NULL DEFAULT 0
             );",
        )?;
        if !Self::table_has_column(&conn, "feedback", "user_id")? {
            conn.execute("ALTER TABLE feedback ADD COLUMN user_id TEXT", [])?;
        }
        if !Self::table_has_column(&conn, "feedback", "author_username")? {
            conn.execute(
                "ALTER TABLE feedback ADD COLUMN author_username TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !Self::table_has_column(&conn, "feedback", "reward_granted")? {
            conn.execute(
                "ALTER TABLE feedback ADD COLUMN reward_granted INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS feedback_votes (
                 feedback_id INTEGER NOT NULL,
                 user_id TEXT NOT NULL,
                 vote INTEGER NOT NULL,
                 PRIMARY KEY (feedback_id, user_id),
                 FOREIGN KEY (feedback_id) REFERENCES feedback(id)
             );",
        )?;
        Ok(())
    }

    fn migrate_feedback_comments_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS feedback_comments (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 feedback_id INTEGER NOT NULL,
                 parent_id INTEGER,
                 user_id TEXT NOT NULL,
                 author_username TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 FOREIGN KEY (feedback_id) REFERENCES feedback(id),
                 FOREIGN KEY (parent_id) REFERENCES feedback_comments(id)
             );
             CREATE INDEX IF NOT EXISTS idx_feedback_comments_feedback_id
                 ON feedback_comments(feedback_id);
             CREATE INDEX IF NOT EXISTS idx_feedback_comments_parent_id
                 ON feedback_comments(parent_id);",
        )?;
        Ok(())
    }

    fn migrate_forum_tables(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS forum_posts (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 user_id TEXT NOT NULL,
                 author_username TEXT NOT NULL,
                 title TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS forum_replies (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 post_id INTEGER NOT NULL,
                 user_id TEXT NOT NULL,
                 author_username TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 FOREIGN KEY (post_id) REFERENCES forum_posts(id)
             );
             CREATE INDEX IF NOT EXISTS idx_forum_replies_post_id
                 ON forum_replies(post_id);",
        )?;
        Ok(())
    }

    fn migrate_password_reset_tokens_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS password_reset_tokens (
                 token TEXT PRIMARY KEY,
                 email TEXT NOT NULL COLLATE NOCASE,
                 expires_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_email
                 ON password_reset_tokens(email);",
        )?;
        Ok(())
    }

    fn migrate_auth_sessions_table(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS auth_sessions (
                 session_id TEXT PRIMARY KEY,
                 kind TEXT NOT NULL CHECK(kind IN ('user', 'admin')),
                 email TEXT,
                 created_at INTEGER NOT NULL,
                 expires_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires
                 ON auth_sessions(expires_at);",
        )?;
        Ok(())
    }

    fn auth_timestamp_now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    pub fn save_auth_session(
        &self,
        session_id: &str,
        kind: &str,
        email: Option<&str>,
        created_at: u64,
        expires_at: u64,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO auth_sessions (session_id, kind, email, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                kind,
                email,
                created_at as i64,
                expires_at as i64
            ],
        )?;
        Ok(())
    }

    pub fn delete_auth_session(&self, session_id: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "DELETE FROM auth_sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn lookup_user_session(&self, session_id: &str) -> Result<Option<String>, StorageError> {
        let now = Self::auth_timestamp_now();
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT email FROM auth_sessions
             WHERE session_id = ?1 AND kind = 'user' AND expires_at > ?2",
        )?;
        let mut rows = stmt.query(params![session_id, now])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn admin_session_valid(&self, session_id: &str) -> Result<bool, StorageError> {
        let now = Self::auth_timestamp_now();
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM auth_sessions
             WHERE session_id = ?1 AND kind = 'admin' AND expires_at > ?2",
            params![session_id, now],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn purge_expired_auth_sessions(&self) -> Result<usize, StorageError> {
        let now = Self::auth_timestamp_now();
        let conn = self.conn.lock().expect("storage lock");
        let deleted = conn.execute(
            "DELETE FROM auth_sessions WHERE expires_at <= ?1",
            params![now],
        )?;
        Ok(deleted)
    }

    fn ensure_username_index(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);",
        )?;
        Ok(())
    }

    fn table_has_column(
        conn: &Connection,
        table: &str,
        column: &str,
    ) -> Result<bool, StorageError> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(columns.iter().any(|name| name == column))
    }

    fn email_local_part(email: &str) -> String {
        email.split('@').next().unwrap_or(email).trim().to_string()
    }

    fn escape_like_pattern(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    }

    fn unique_username_from_base(conn: &Connection, base: &str) -> Result<String, StorageError> {
        let base = base.trim();
        let base = if base.is_empty() { "user" } else { base };
        let mut candidate = base.to_string();
        let mut suffix = 0_u32;
        loop {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE username = ?1 COLLATE NOCASE",
                params![candidate],
                |row| row.get(0),
            )?;
            if count == 0 {
                return Ok(candidate);
            }
            suffix += 1;
            candidate = format!("{base}{suffix}");
        }
    }

    fn drop_legacy_name_column(conn: &Connection) -> Result<(), StorageError> {
        if !Self::table_has_column(conn, "users", "name")? {
            return Ok(());
        }

        conn.execute_batch(
            "CREATE TABLE users_new (
                 email TEXT PRIMARY KEY COLLATE NOCASE,
                 username TEXT NOT NULL COLLATE NOCASE,
                 first_name TEXT NOT NULL,
                 last_name TEXT NOT NULL,
                 password TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO users_new (email, username, first_name, last_name, password, created_at)
             SELECT email,
                    COALESCE(NULLIF(TRIM(username), ''), 'user'),
                    COALESCE(NULLIF(TRIM(first_name), ''), ''),
                    COALESCE(last_name, ''),
                    password,
                    created_at
             FROM users;
             DROP TABLE users;
             ALTER TABLE users_new RENAME TO users;
             CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);",
        )?;
        Ok(())
    }

    fn migrate_user_columns(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let has_name = Self::table_has_column(&conn, "users", "name")?;
        let has_username = Self::table_has_column(&conn, "users", "username")?;

        if !has_username {
            conn.execute_batch(
                "ALTER TABLE users ADD COLUMN username TEXT;
                 ALTER TABLE users ADD COLUMN first_name TEXT;
                 ALTER TABLE users ADD COLUMN last_name TEXT;",
            )?;

            if has_name {
                let mut stmt =
                    conn.prepare("SELECT email, name, username, first_name, last_name FROM users")?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                for (email, legacy_name, username, first_name, last_name) in rows {
                    let legacy_name = legacy_name.unwrap_or_default();
                    let username_base = username
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| {
                            if legacy_name.trim().is_empty() {
                                Self::email_local_part(&email)
                            } else {
                                legacy_name.trim().to_string()
                            }
                        });
                    let username = Self::unique_username_from_base(&conn, &username_base)?;
                    let first = first_name
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| legacy_name.trim().to_string());
                    let last = last_name.unwrap_or_default();
                    conn.execute(
                        "UPDATE users SET username = ?1, first_name = ?2, last_name = ?3 WHERE email = ?4 COLLATE NOCASE",
                        params![username, first, last, email],
                    )?;
                }
            } else {
                conn.execute(
                    "UPDATE users SET username = ?1, first_name = ?2, last_name = ?3
                     WHERE username IS NULL OR TRIM(username) = ''",
                    params!["user", "", ""],
                )?;
            }

            conn.execute_batch(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);",
            )?;
        } else {
            Self::ensure_username_index(&conn)?;
        }

        Self::drop_legacy_name_column(&conn)?;

        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("whiskerwatch.db")
    }

    pub fn load_users(&self) -> Result<Vec<User>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT username, first_name, last_name, email, password, created_at
             FROM users ORDER BY created_at ASC",
        )?;
        let users = stmt
            .query_map([], |row| {
                Ok(User {
                    username: row.get(0)?,
                    first_name: row.get(1)?,
                    last_name: row.get(2)?,
                    email: row.get(3)?,
                    password: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(users)
    }

    pub fn save_user(&self, user: &User) -> Result<(), StorageError> {
        if self.user_exists(&user.email)? {
            return Err(StorageError::EmailTaken);
        }
        if self.username_exists(&user.username)? {
            return Err(StorageError::UsernameTaken);
        }

        let stored_password = if is_bcrypt_hash(&user.password) {
            user.password.clone()
        } else {
            hash_password(&user.password)?
        };
        let conn = self.conn.lock().expect("storage lock");
        match conn.execute(
            "INSERT INTO users (email, username, first_name, last_name, password, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                user.email,
                user.username,
                user.first_name,
                user.last_name,
                stored_password,
                user.created_at as i64
            ],
        ) {
            Ok(_) => Ok(()),
            Err(error) if is_unique_constraint(&error) => {
                let email_taken: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM users WHERE email = ?1 COLLATE NOCASE",
                    params![user.email],
                    |row| row.get(0),
                )?;
                if email_taken > 0 {
                    Err(StorageError::EmailTaken)
                } else {
                    Err(StorageError::UsernameTaken)
                }
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn username_exists(&self, username: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE username = ?1 COLLATE NOCASE",
            params![username],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn user_exists(&self, email: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE email = ?1 COLLATE NOCASE",
            params![email],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn email_for_username(&self, username: &str) -> Result<Option<String>, StorageError> {
        let username = username.trim();
        if username.is_empty() {
            return Ok(None);
        }
        let conn = self.conn.lock().expect("storage lock");
        let result = conn.query_row(
            "SELECT email FROM users WHERE username = ?1 COLLATE NOCASE",
            params![username],
            |row| row.get(0),
        );
        match result {
            Ok(email) => Ok(Some(email)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn search_users_by_username(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<StoredUserSearchHit>, StorageError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let escaped = Self::escape_like_pattern(query);
        let pattern = format!("%{escaped}%");
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT email, username, first_name, last_name
             FROM users
             WHERE username LIKE ?1 ESCAPE '\\' COLLATE NOCASE
             ORDER BY
               CASE WHEN username LIKE ?2 ESCAPE '\\' COLLATE NOCASE THEN 0 ELSE 1 END,
               username COLLATE NOCASE
             LIMIT ?3",
        )?;
        let prefix = format!("{escaped}%");
        let limit = limit.min(20) as i64;
        let rows = stmt.query_map(params![pattern, prefix, limit], |row| {
            Ok(StoredUserSearchHit {
                email: row.get(0)?,
                username: row.get(1)?,
                first_name: row.get(2)?,
                last_name: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_user_password(&self, email: &str, new_password: &str) -> Result<(), StorageError> {
        let hashed = hash_password(new_password)?;
        self.update_password_hash(email, &hashed)
    }

    fn update_password_hash(&self, email: &str, password_hash: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let updated = conn.execute(
            "UPDATE users SET password = ?1 WHERE email = ?2 COLLATE NOCASE",
            params![password_hash, email],
        )?;
        if updated == 0 {
            return Err(StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
        }
        Ok(())
    }

    pub fn create_password_reset_token(&self, email: &str) -> Result<String, StorageError> {
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + 3600;
        let token = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "DELETE FROM password_reset_tokens WHERE email = ?1 COLLATE NOCASE",
            params![email],
        )?;
        conn.execute(
            "INSERT INTO password_reset_tokens (token, email, expires_at) VALUES (?1, ?2, ?3)",
            params![token, email, expires_at],
        )?;
        Ok(token)
    }

    pub fn find_valid_reset_token(&self, token: &str) -> Result<Option<String>, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT email FROM password_reset_tokens WHERE token = ?1 AND expires_at > ?2",
        )?;
        let mut rows = stmt.query(params![token, now])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn reset_password_with_token(
        &self,
        token: &str,
        new_password: &str,
    ) -> Result<(), StorageError> {
        let email = self
            .find_valid_reset_token(token)?
            .ok_or(StorageError::InvalidResetToken)?;
        let hashed = hash_password(new_password)?;
        let conn = self.conn.lock().expect("storage lock");
        let updated = conn.execute(
            "UPDATE users SET password = ?1 WHERE email = ?2 COLLATE NOCASE",
            params![hashed, email],
        )?;
        if updated == 0 {
            return Err(StorageError::InvalidResetToken);
        }
        conn.execute(
            "DELETE FROM password_reset_tokens WHERE token = ?1",
            params![token],
        )?;
        Ok(())
    }

    pub fn validate_login(&self, email: &str, password: &str) -> Result<bool, StorageError> {
        let Some(user) = self.find_user_by_email(email)? else {
            return Ok(false);
        };

        if is_bcrypt_hash(&user.password) {
            return Ok(bcrypt::verify(password, &user.password).unwrap_or(false));
        }

        if user.password == password {
            let hashed = hash_password(password)?;
            self.update_password_hash(email, &hashed)?;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn find_user_by_email(&self, email: &str) -> Result<Option<User>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT username, first_name, last_name, email, password, created_at
             FROM users WHERE email = ?1 COLLATE NOCASE",
        )?;
        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                username: row.get(0)?,
                first_name: row.get(1)?,
                last_name: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                created_at: row.get::<_, i64>(5)? as u64,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn load_profile(&self, email: &str) -> Result<Option<UserProfile>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt =
            conn.prepare("SELECT profile_json FROM user_profiles WHERE email = ?1 COLLATE NOCASE")?;
        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            Ok(Some(serde_json::from_str(&json)?))
        } else {
            Ok(None)
        }
    }

    pub fn save_profile(&self, profile: &UserProfile) -> Result<(), StorageError> {
        let json = serde_json::to_string(profile)?;
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO user_profiles (email, profile_json) VALUES (?1, ?2)
             ON CONFLICT(email) DO UPDATE SET profile_json = excluded.profile_json",
            params![profile.email, json],
        )?;
        Ok(())
    }

    pub fn save_contact(&self, submission: &ContactSubmission) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO contact_messages (name, email, subject, message, submitted_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                submission.name,
                submission.email,
                submission.subject,
                submission.message,
                submission.submitted_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn load_contacts(&self) -> Result<Vec<ContactSubmission>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT name, email, subject, message, submitted_at
             FROM contact_messages ORDER BY submitted_at ASC",
        )?;
        let contacts = stmt
            .query_map([], |row| {
                Ok(ContactSubmission {
                    name: row.get(0)?,
                    email: row.get(1)?,
                    subject: row.get(2)?,
                    message: row.get(3)?,
                    submitted_at: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(contacts)
    }

    pub fn save_feedback(&self, submission: &FeedbackSubmission) -> Result<i64, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO feedback (name, email, category, message, submitted_at, user_id, author_username)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                submission.name,
                submission.email,
                submission.category,
                submission.message,
                submission.submitted_at as i64,
                submission.user_id,
                submission.author_username,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_feedback(&self) -> Result<Vec<FeedbackSubmission>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, name, email, category, message, submitted_at, user_id, author_username
             FROM feedback ORDER BY submitted_at ASC",
        )?;
        let feedback = stmt
            .query_map([], |row| {
                let author_username: String = row.get(7)?;
                let name: String = row.get(1)?;
                Ok(FeedbackSubmission {
                    id: row.get(0)?,
                    name: name.clone(),
                    email: row.get(2)?,
                    category: row.get(3)?,
                    message: row.get(4)?,
                    submitted_at: row.get::<_, i64>(5)? as u64,
                    user_id: row.get(6)?,
                    author_username: if author_username.is_empty() {
                        name
                    } else {
                        author_username
                    },
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(feedback)
    }

    pub fn load_feedback_forum(
        &self,
        viewer_email: Option<&str>,
    ) -> Result<Vec<FeedbackForumEntry>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, name, email, category, message, submitted_at, user_id, author_username,
                    reward_granted
             FROM feedback ORDER BY submitted_at ASC",
        )?;
        let mut entries = stmt
            .query_map([], |row| {
                let author_username: String = row.get(7)?;
                let name: String = row.get(1)?;
                let reward_granted: i64 = row.get(8)?;
                Ok(FeedbackForumEntry {
                    submission: FeedbackSubmission {
                        id: row.get(0)?,
                        name: name.clone(),
                        email: row.get(2)?,
                        category: row.get(3)?,
                        message: row.get(4)?,
                        submitted_at: row.get::<_, i64>(5)? as u64,
                        user_id: row.get(6)?,
                        author_username: if author_username.is_empty() {
                            name
                        } else {
                            author_username
                        },
                    },
                    upvotes: 0,
                    downvotes: 0,
                    user_vote: None,
                    reward_granted: reward_granted != 0,
                    comments: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for entry in &mut entries {
            let (upvotes, downvotes) = Self::feedback_vote_totals(&conn, entry.submission.id)?;
            entry.upvotes = upvotes;
            entry.downvotes = downvotes;
            if let Some(email) = viewer_email {
                entry.user_vote = Self::feedback_user_vote(&conn, entry.submission.id, email)?;
            }
            entry.comments = Self::list_feedback_comments_on_conn(&conn, entry.submission.id)?;
        }

        entries.sort_by(|left, right| {
            let left_score = left.upvotes as i32 - left.downvotes as i32;
            let right_score = right.upvotes as i32 - right.downvotes as i32;
            right_score.cmp(&left_score).then_with(|| {
                right
                    .submission
                    .submitted_at
                    .cmp(&left.submission.submitted_at)
            })
        });

        Ok(entries)
    }

    fn feedback_vote_totals(
        conn: &rusqlite::Connection,
        feedback_id: i64,
    ) -> Result<(u32, u32), StorageError> {
        let upvotes: u32 = conn.query_row(
            "SELECT COUNT(*) FROM feedback_votes WHERE feedback_id = ?1 AND vote = 1",
            [feedback_id],
            |row| row.get(0),
        )?;
        let downvotes: u32 = conn.query_row(
            "SELECT COUNT(*) FROM feedback_votes WHERE feedback_id = ?1 AND vote = -1",
            [feedback_id],
            |row| row.get(0),
        )?;
        Ok((upvotes, downvotes))
    }

    fn feedback_user_vote(
        conn: &rusqlite::Connection,
        feedback_id: i64,
        user_id: &str,
    ) -> Result<Option<i8>, StorageError> {
        let vote: Result<i8, rusqlite::Error> = conn.query_row(
            "SELECT vote FROM feedback_votes WHERE feedback_id = ?1 AND user_id = ?2",
            params![feedback_id, user_id],
            |row| row.get(0),
        );
        match vote {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn get_feedback_submission(
        &self,
        feedback_id: i64,
    ) -> Result<Option<FeedbackSubmission>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let row = conn.query_row(
            "SELECT id, name, email, category, message, submitted_at, user_id, author_username
             FROM feedback WHERE id = ?1",
            [feedback_id],
            |row| {
                let author_username: String = row.get(7)?;
                let name: String = row.get(1)?;
                Ok(FeedbackSubmission {
                    id: row.get(0)?,
                    name: name.clone(),
                    email: row.get(2)?,
                    category: row.get(3)?,
                    message: row.get(4)?,
                    submitted_at: row.get::<_, i64>(5)? as u64,
                    user_id: row.get(6)?,
                    author_username: if author_username.is_empty() {
                        name
                    } else {
                        author_username
                    },
                })
            },
        );
        match row {
            Ok(submission) => Ok(Some(submission)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn cast_feedback_vote(
        &self,
        feedback_id: i64,
        user_id: &str,
        vote: i8,
    ) -> Result<FeedbackVoteCounts, StorageError> {
        if vote != 1 && vote != -1 {
            return Err(StorageError::InvalidInput(
                "vote must be 1 or -1".to_string(),
            ));
        }

        let conn = self.conn.lock().expect("storage lock");
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM feedback WHERE id = ?1",
                [feedback_id],
                |_| Ok(()),
            )
            .is_ok();
        if !exists {
            return Err(StorageError::InvalidInput("feedback not found".to_string()));
        }

        let existing = Self::feedback_user_vote(&conn, feedback_id, user_id)?;
        match existing {
            Some(current) if current == vote => {
                conn.execute(
                    "DELETE FROM feedback_votes WHERE feedback_id = ?1 AND user_id = ?2",
                    params![feedback_id, user_id],
                )?;
            }
            Some(_) => {
                conn.execute(
                    "UPDATE feedback_votes SET vote = ?1 WHERE feedback_id = ?2 AND user_id = ?3",
                    params![vote, feedback_id, user_id],
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO feedback_votes (feedback_id, user_id, vote) VALUES (?1, ?2, ?3)",
                    params![feedback_id, user_id, vote],
                )?;
            }
        }

        let (upvotes, downvotes) = Self::feedback_vote_totals(&conn, feedback_id)?;
        let user_vote = Self::feedback_user_vote(&conn, feedback_id, user_id)?;
        Ok(FeedbackVoteCounts {
            upvotes,
            downvotes,
            user_vote,
        })
    }

    pub fn feedback_reward_granted(&self, feedback_id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let granted: i64 = conn.query_row(
            "SELECT reward_granted FROM feedback WHERE id = ?1",
            [feedback_id],
            |row| row.get(0),
        )?;
        Ok(granted != 0)
    }

    pub fn mark_feedback_reward_granted(&self, feedback_id: i64) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE feedback SET reward_granted = 1 WHERE id = ?1",
            [feedback_id],
        )?;
        Ok(())
    }

    pub fn delete_feedback_owned(
        &self,
        feedback_id: i64,
        user_email: &str,
    ) -> Result<ForumDeleteOutcome, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let row: Result<(Option<String>, String), rusqlite::Error> = conn.query_row(
            "SELECT user_id, email FROM feedback WHERE id = ?1",
            params![feedback_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        let (stored_user_id, stored_email) = match row {
            Ok(values) => values,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(ForumDeleteOutcome::NotFound);
            }
            Err(error) => return Err(error.into()),
        };

        let authorized = stored_user_id
            .as_deref()
            .is_some_and(|id| id.eq_ignore_ascii_case(user_email))
            || stored_email.eq_ignore_ascii_case(user_email);
        if !authorized {
            return Ok(ForumDeleteOutcome::NotAuthorized);
        }

        conn.execute(
            "DELETE FROM feedback_votes WHERE feedback_id = ?1",
            params![feedback_id],
        )?;
        conn.execute(
            "DELETE FROM feedback_comments WHERE feedback_id = ?1",
            params![feedback_id],
        )?;
        conn.execute("DELETE FROM feedback WHERE id = ?1", params![feedback_id])?;
        Ok(ForumDeleteOutcome::Deleted)
    }

    fn list_feedback_comments_on_conn(
        conn: &rusqlite::Connection,
        feedback_id: i64,
    ) -> Result<Vec<FeedbackComment>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, feedback_id, parent_id, user_id, author_username, body, created_at
             FROM feedback_comments
             WHERE feedback_id = ?1
             ORDER BY created_at ASC",
        )?;
        let comments = stmt
            .query_map(params![feedback_id], |row| {
                Ok(FeedbackComment {
                    id: row.get(0)?,
                    feedback_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    user_id: row.get(3)?,
                    author_username: row.get(4)?,
                    body: row.get(5)?,
                    created_at: row.get::<_, i64>(6)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(comments)
    }

    pub fn list_feedback_comments(
        &self,
        feedback_id: i64,
    ) -> Result<Vec<FeedbackComment>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        Self::list_feedback_comments_on_conn(&conn, feedback_id)
    }

    pub fn get_feedback_comment(
        &self,
        comment_id: i64,
    ) -> Result<Option<FeedbackComment>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let row = conn.query_row(
            "SELECT id, feedback_id, parent_id, user_id, author_username, body, created_at
             FROM feedback_comments WHERE id = ?1",
            params![comment_id],
            |row| {
                Ok(FeedbackComment {
                    id: row.get(0)?,
                    feedback_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    user_id: row.get(3)?,
                    author_username: row.get(4)?,
                    body: row.get(5)?,
                    created_at: row.get::<_, i64>(6)? as u64,
                })
            },
        );
        match row {
            Ok(comment) => Ok(Some(comment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn create_feedback_comment(
        &self,
        feedback_id: i64,
        parent_id: Option<i64>,
        user_id: &str,
        author_username: &str,
        body: &str,
        created_at: u64,
    ) -> Result<i64, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let feedback_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM feedback WHERE id = ?1",
            params![feedback_id],
            |row| row.get(0),
        )?;
        if feedback_exists == 0 {
            return Err(StorageError::InvalidInput(
                "feedback post not found".to_string(),
            ));
        }

        if let Some(parent_id) = parent_id {
            let parent = Self::get_feedback_comment_on_conn(&conn, parent_id)?;
            let Some(parent) = parent else {
                return Err(StorageError::InvalidInput(
                    "parent comment not found".to_string(),
                ));
            };
            if parent.feedback_id != feedback_id {
                return Err(StorageError::InvalidInput(
                    "parent comment belongs to another post".to_string(),
                ));
            }
        }

        conn.execute(
            "INSERT INTO feedback_comments
             (feedback_id, parent_id, user_id, author_username, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                feedback_id,
                parent_id,
                user_id,
                author_username,
                body,
                created_at as i64,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_feedback_comment_on_conn(
        conn: &rusqlite::Connection,
        comment_id: i64,
    ) -> Result<Option<FeedbackComment>, StorageError> {
        let row = conn.query_row(
            "SELECT id, feedback_id, parent_id, user_id, author_username, body, created_at
             FROM feedback_comments WHERE id = ?1",
            params![comment_id],
            |row| {
                Ok(FeedbackComment {
                    id: row.get(0)?,
                    feedback_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    user_id: row.get(3)?,
                    author_username: row.get(4)?,
                    body: row.get(5)?,
                    created_at: row.get::<_, i64>(6)? as u64,
                })
            },
        );
        match row {
            Ok(comment) => Ok(Some(comment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn delete_feedback_comment_branch_on_conn(
        conn: &rusqlite::Connection,
        comment_id: i64,
    ) -> Result<(), StorageError> {
        let mut child_ids = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT id FROM feedback_comments WHERE parent_id = ?1",
        )?;
        let rows = stmt.query_map(params![comment_id], |row| row.get(0))?;
        for child_id in rows {
            child_ids.push(child_id?);
        }
        for child_id in child_ids {
            Self::delete_feedback_comment_branch_on_conn(conn, child_id)?;
        }
        conn.execute(
            "DELETE FROM feedback_comments WHERE id = ?1",
            params![comment_id],
        )?;
        Ok(())
    }

    pub fn delete_feedback_comment_owned(
        &self,
        comment_id: i64,
        user_email: &str,
    ) -> Result<ForumDeleteOutcome, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let owner: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT user_id FROM feedback_comments WHERE id = ?1",
            params![comment_id],
            |row| row.get(0),
        );
        let owner = match owner {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(ForumDeleteOutcome::NotFound);
            }
            Err(error) => return Err(error.into()),
        };
        if !owner.eq_ignore_ascii_case(user_email) {
            return Ok(ForumDeleteOutcome::NotAuthorized);
        }
        Self::delete_feedback_comment_branch_on_conn(&conn, comment_id)?;
        Ok(ForumDeleteOutcome::Deleted)
    }

    pub fn create_forum_post(
        &self,
        user_id: &str,
        author_username: &str,
        title: &str,
        body: &str,
        breed_slug: &str,
        created_at: u64,
    ) -> Result<i64, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO forum_posts (user_id, author_username, title, body, breed_slug, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                user_id,
                author_username,
                title,
                body,
                breed_slug,
                created_at as i64,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_forum_posts(
        &self,
        breed_slug: Option<&str>,
    ) -> Result<Vec<ForumPost>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let filter = breed_slug.map(str::trim).filter(|value| !value.is_empty());
        let posts = if let Some(slug) = filter {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, author_username, title, body, created_at, breed_slug
                 FROM forum_posts WHERE breed_slug = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![slug], map_forum_post_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, author_username, title, body, created_at, breed_slug
                 FROM forum_posts ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], map_forum_post_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        Ok(posts)
    }

    pub fn list_profile_emails(&self) -> Result<Vec<String>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare("SELECT email FROM user_profiles ORDER BY email ASC")?;
        let emails = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(emails)
    }

    pub fn upsert_push_subscription(
        &self,
        email: &str,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        created_at: u64,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO push_subscriptions (endpoint, email, p256dh, auth, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(endpoint) DO UPDATE SET
                 email = excluded.email,
                 p256dh = excluded.p256dh,
                 auth = excluded.auth,
                 created_at = excluded.created_at",
            params![endpoint, email, p256dh, auth, created_at as i64],
        )?;
        Ok(())
    }

    pub fn delete_push_subscription(&self, endpoint: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "DELETE FROM push_subscriptions WHERE endpoint = ?1",
            params![endpoint],
        )?;
        Ok(())
    }

    pub fn list_push_subscriptions(&self) -> Result<Vec<PushSubscription>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT email, endpoint, p256dh, auth, created_at
             FROM push_subscriptions ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PushSubscription {
                email: row.get(0)?,
                endpoint: row.get(1)?,
                p256dh: row.get(2)?,
                auth: row.get(3)?,
                created_at: row.get::<_, i64>(4)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_forum_post(&self, post_id: i64) -> Result<Option<ForumPost>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, user_id, author_username, title, body, created_at, breed_slug
             FROM forum_posts WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![post_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_forum_post_row(&row)?))
        } else {
            Ok(None)
        }
    }

    pub fn count_forum_replies(&self, post_id: i64) -> Result<u32, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM forum_replies WHERE post_id = ?1",
            params![post_id],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }

    pub fn list_forum_replies(&self, post_id: i64) -> Result<Vec<ForumReply>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, post_id, user_id, author_username, body, created_at
             FROM forum_replies WHERE post_id = ?1 ORDER BY created_at ASC",
        )?;
        let replies = stmt
            .query_map(params![post_id], |row| {
                Ok(ForumReply {
                    id: row.get(0)?,
                    post_id: row.get(1)?,
                    user_id: row.get(2)?,
                    author_username: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(replies)
    }

    pub fn create_forum_reply(
        &self,
        post_id: i64,
        user_id: &str,
        author_username: &str,
        body: &str,
        created_at: u64,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO forum_replies (post_id, user_id, author_username, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![post_id, user_id, author_username, body, created_at as i64,],
        )?;
        Ok(())
    }

    pub fn delete_forum_post_owned(
        &self,
        post_id: i64,
        user_id: &str,
    ) -> Result<ForumDeleteOutcome, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let owner: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT user_id FROM forum_posts WHERE id = ?1",
            params![post_id],
            |row| row.get(0),
        );
        let owner = match owner {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(ForumDeleteOutcome::NotFound);
            }
            Err(error) => return Err(error.into()),
        };
        if !owner.eq_ignore_ascii_case(user_id) {
            return Ok(ForumDeleteOutcome::NotAuthorized);
        }
        conn.execute(
            "DELETE FROM forum_replies WHERE post_id = ?1",
            params![post_id],
        )?;
        conn.execute("DELETE FROM forum_posts WHERE id = ?1", params![post_id])?;
        Ok(ForumDeleteOutcome::Deleted)
    }

    pub fn delete_forum_reply_owned(
        &self,
        reply_id: i64,
        user_id: &str,
    ) -> Result<ForumDeleteOutcome, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let owner: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT user_id FROM forum_replies WHERE id = ?1",
            params![reply_id],
            |row| row.get(0),
        );
        let owner = match owner {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(ForumDeleteOutcome::NotFound);
            }
            Err(error) => return Err(error.into()),
        };
        if !owner.eq_ignore_ascii_case(user_id) {
            return Ok(ForumDeleteOutcome::NotAuthorized);
        }
        conn.execute("DELETE FROM forum_replies WHERE id = ?1", params![reply_id])?;
        Ok(ForumDeleteOutcome::Deleted)
    }

    /// Returns true if this session was newly recorded (caller should credit points).
    pub fn try_record_stripe_fulfillment(
        &self,
        session_id: &str,
        user_email: &str,
        paw_points: u32,
    ) -> Result<bool, StorageError> {
        let fulfilled_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn.lock().expect("storage lock");
        let rows = conn.execute(
            "INSERT OR IGNORE INTO stripe_fulfilled_sessions (session_id, user_email, paw_points, fulfilled_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![session_id, user_email, paw_points as i64, fulfilled_at],
        )?;
        Ok(rows > 0)
    }

    fn migrate_from_jsonl(&self) -> Result<(), StorageError> {
        #[derive(Deserialize)]
        struct LegacyUserJson {
            #[serde(default)]
            name: String,
            #[serde(default)]
            username: String,
            #[serde(default)]
            first_name: String,
            #[serde(default)]
            last_name: String,
            email: String,
            password: String,
            created_at: u64,
        }

        let users_path = self.data_dir.join("users.jsonl");
        if self.load_users()?.is_empty() && users_path.exists() {
            let contents = std::fs::read_to_string(&users_path)?;
            for line in contents.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(legacy) = serde_json::from_str::<LegacyUserJson>(line) {
                    let username = if !legacy.username.trim().is_empty() {
                        legacy.username.trim().to_string()
                    } else if !legacy.name.trim().is_empty() {
                        legacy.name.trim().to_string()
                    } else {
                        Self::email_local_part(&legacy.email)
                    };
                    let first_name = if !legacy.first_name.trim().is_empty() {
                        legacy.first_name.trim().to_string()
                    } else {
                        legacy.name.trim().to_string()
                    };
                    let user = User {
                        username,
                        first_name,
                        last_name: legacy.last_name.trim().to_string(),
                        email: legacy.email,
                        password: legacy.password,
                        created_at: legacy.created_at,
                    };
                    let _ = self.save_user(&user);
                }
            }
            eprintln!("Migrated users from {} into SQLite", users_path.display());
        }

        let profiles_path = self.data_dir.join("user_profiles.jsonl");
        if profiles_path.exists() {
            let contents = std::fs::read_to_string(&profiles_path)?;
            for line in contents.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(profile) = serde_json::from_str::<UserProfile>(line) {
                    if self.load_profile(&profile.email)?.is_none() {
                        let _ = self.save_profile(&profile);
                    }
                }
            }
            eprintln!(
                "Migrated profiles from {} into SQLite (missing entries only)",
                profiles_path.display()
            );
        }

        let contacts_path = self.data_dir.join("contact_messages.jsonl");
        if self.load_contacts()?.is_empty() && contacts_path.exists() {
            let contents = std::fs::read_to_string(&contacts_path)?;
            for line in contents.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(submission) = serde_json::from_str::<ContactSubmission>(line) {
                    let _ = self.save_contact(&submission);
                }
            }
        }

        let feedback_path = self.data_dir.join("feedback.jsonl");
        if self.load_feedback()?.is_empty() && feedback_path.exists() {
            let contents = std::fs::read_to_string(&feedback_path)?;
            let mut migrated = 0_u32;
            for line in contents.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(submission) = serde_json::from_str::<FeedbackSubmission>(line) {
                    if self.save_feedback(&submission).is_ok() {
                        migrated += 1;
                    }
                }
            }
            if migrated > 0 {
                eprintln!(
                    "Migrated {migrated} feedback entries from {} into SQLite",
                    feedback_path.display()
                );
            }
        }

        Ok(())
    }

    pub fn persisted_counts(&self) -> Result<(usize, usize, usize, usize, usize), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        let forum_posts: i64 =
            conn.query_row("SELECT COUNT(*) FROM forum_posts", [], |row| row.get(0))?;
        let forum_replies: i64 =
            conn.query_row("SELECT COUNT(*) FROM forum_replies", [], |row| row.get(0))?;
        let feedback: i64 =
            conn.query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))?;
        let contacts: i64 = conn.query_row("SELECT COUNT(*) FROM contact_messages", [], |row| {
            row.get(0)
        })?;
        Ok((
            users as usize,
            forum_posts as usize,
            forum_replies as usize,
            feedback as usize,
            contacts as usize,
        ))
    }

    fn normalize_social_email(email: &str) -> String {
        email.trim().to_lowercase()
    }

    pub fn are_friends(&self, left: &str, right: &str) -> Result<bool, StorageError> {
        let left = Self::normalize_social_email(left);
        let right = Self::normalize_social_email(right);
        if left == right {
            return Ok(false);
        }
        if self.users_block_each_other(&left, &right)? {
            return Ok(false);
        }
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_requests
             WHERE status = 'accepted'
               AND ((from_email = ?1 AND to_email = ?2) OR (from_email = ?2 AND to_email = ?1))",
            params![left, right],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn has_pending_friend_request(&self, left: &str, right: &str) -> Result<bool, StorageError> {
        let left = Self::normalize_social_email(left);
        let right = Self::normalize_social_email(right);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_requests
             WHERE status = 'pending'
               AND ((from_email = ?1 AND to_email = ?2) OR (from_email = ?2 AND to_email = ?1))",
            params![left, right],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn create_friend_request(
        &self,
        from_email: &str,
        to_email: &str,
        created_at: u64,
    ) -> Result<(), StorageError> {
        let from_email = Self::normalize_social_email(from_email);
        let to_email = Self::normalize_social_email(to_email);
        if from_email == to_email {
            return Err(StorageError::InvalidInput("cannot friend yourself".into()));
        }
        if !self.user_exists(&to_email)? {
            return Err(StorageError::InvalidInput("user not found".into()));
        }
        if self.are_friends(&from_email, &to_email)? {
            return Err(StorageError::InvalidInput("already friends".into()));
        }
        if self.has_pending_friend_request(&from_email, &to_email)? {
            return Err(StorageError::InvalidInput("request already pending".into()));
        }
        if self.users_block_each_other(&from_email, &to_email)? {
            return Err(StorageError::InvalidInput("user blocked".into()));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO friend_requests (id, from_email, to_email, status, created_at)
             VALUES (?1, ?2, ?3, 'pending', ?4)",
            params![id, from_email, to_email, created_at as i64],
        )?;
        Ok(())
    }

    pub fn respond_friend_request(
        &self,
        request_id: &str,
        recipient_email: &str,
        accept: bool,
    ) -> Result<(), StorageError> {
        let recipient_email = Self::normalize_social_email(recipient_email);
        let status = if accept { "accepted" } else { "declined" };
        let conn = self.conn.lock().expect("storage lock");
        let participants = conn.query_row(
            "SELECT from_email, to_email FROM friend_requests
             WHERE id = ?1 AND to_email = ?2 COLLATE NOCASE AND status = 'pending'",
            params![request_id, recipient_email],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        let (from_email, to_email) = match participants {
            Ok(value) => value,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(StorageError::InvalidInput("request not found".into()));
            }
            Err(error) => return Err(error.into()),
        };
        let updated = conn.execute(
            "UPDATE friend_requests SET status = ?1
             WHERE id = ?2 AND to_email = ?3 COLLATE NOCASE AND status = 'pending'",
            params![status, request_id, recipient_email],
        )?;
        if updated == 0 {
            return Err(StorageError::InvalidInput("request not found".into()));
        }
        drop(conn);
        if accept {
            let _ = self.accept_message_threads_for_friends(
                &from_email,
                &to_email,
                Self::timestamp_now_fallback(),
            );
        }
        Ok(())
    }

    fn timestamp_now_fallback() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    pub fn list_incoming_friend_requests(
        &self,
        email: &str,
    ) -> Result<Vec<StoredFriendRequest>, StorageError> {
        let email = Self::normalize_social_email(email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, from_email, to_email, status, created_at
             FROM friend_requests
             WHERE to_email = ?1 COLLATE NOCASE AND status = 'pending'
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![email], |row| {
            Ok(StoredFriendRequest {
                id: row.get(0)?,
                from_email: row.get(1)?,
                to_email: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get::<_, i64>(4)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn list_outgoing_friend_requests(
        &self,
        email: &str,
    ) -> Result<Vec<StoredFriendRequest>, StorageError> {
        let email = Self::normalize_social_email(email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, from_email, to_email, status, created_at
             FROM friend_requests
             WHERE from_email = ?1 COLLATE NOCASE AND status = 'pending'
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![email], |row| {
            Ok(StoredFriendRequest {
                id: row.get(0)?,
                from_email: row.get(1)?,
                to_email: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get::<_, i64>(4)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn is_user_blocked(&self, blocker_email: &str, blocked_email: &str) -> Result<bool, StorageError> {
        let blocker_email = Self::normalize_social_email(blocker_email);
        let blocked_email = Self::normalize_social_email(blocked_email);
        if blocker_email == blocked_email {
            return Ok(false);
        }
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM blocked_users
             WHERE blocker_email = ?1 AND blocked_email = ?2",
            params![blocker_email, blocked_email],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn users_block_each_other(&self, left_email: &str, right_email: &str) -> Result<bool, StorageError> {
        let left_email = Self::normalize_social_email(left_email);
        let right_email = Self::normalize_social_email(right_email);
        if left_email == right_email {
            return Ok(false);
        }
        Ok(self.is_user_blocked(&left_email, &right_email)?
            || self.is_user_blocked(&right_email, &left_email)?)
    }

    pub fn block_user(
        &self,
        blocker_email: &str,
        blocked_email: &str,
        created_at: u64,
    ) -> Result<(), StorageError> {
        let blocker_email = Self::normalize_social_email(blocker_email);
        let blocked_email = Self::normalize_social_email(blocked_email);
        if blocker_email == blocked_email {
            return Err(StorageError::InvalidInput("cannot block yourself".into()));
        }
        if !self.user_exists(&blocked_email)? {
            return Err(StorageError::InvalidInput("user not found".into()));
        }
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO blocked_users (blocker_email, blocked_email, created_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(blocker_email, blocked_email) DO UPDATE SET created_at = excluded.created_at",
            params![blocker_email, blocked_email, created_at as i64],
        )?;
        Ok(())
    }

    pub fn unblock_user(&self, blocker_email: &str, blocked_email: &str) -> Result<(), StorageError> {
        let blocker_email = Self::normalize_social_email(blocker_email);
        let blocked_email = Self::normalize_social_email(blocked_email);
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "DELETE FROM blocked_users
             WHERE blocker_email = ?1 AND blocked_email = ?2",
            params![blocker_email, blocked_email],
        )?;
        Ok(())
    }

    pub fn list_blocked_users(&self, blocker_email: &str) -> Result<Vec<String>, StorageError> {
        let blocker_email = Self::normalize_social_email(blocker_email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT blocked_email FROM blocked_users
             WHERE blocker_email = ?1
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![blocker_email], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn list_users_who_blocked(&self, blocked_email: &str) -> Result<Vec<String>, StorageError> {
        let blocked_email = Self::normalize_social_email(blocked_email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT blocker_email FROM blocked_users
             WHERE blocked_email = ?1
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![blocked_email], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn remove_friendship_between(&self, left_email: &str, right_email: &str) -> Result<(), StorageError> {
        let left_email = Self::normalize_social_email(left_email);
        let right_email = Self::normalize_social_email(right_email);
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE friend_requests SET status = 'declined'
             WHERE status = 'accepted'
               AND ((from_email = ?1 AND to_email = ?2) OR (from_email = ?2 AND to_email = ?1))",
            params![left_email, right_email],
        )?;
        Ok(())
    }

    pub fn cancel_pending_friend_requests_between(
        &self,
        left_email: &str,
        right_email: &str,
    ) -> Result<(), StorageError> {
        let left_email = Self::normalize_social_email(left_email);
        let right_email = Self::normalize_social_email(right_email);
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE friend_requests SET status = 'declined'
             WHERE status = 'pending'
               AND ((from_email = ?1 AND to_email = ?2) OR (from_email = ?2 AND to_email = ?1))",
            params![left_email, right_email],
        )?;
        Ok(())
    }

    pub fn list_friends(&self, email: &str) -> Result<Vec<StoredFriendSummary>, StorageError> {
        let email = Self::normalize_social_email(email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT from_email, to_email FROM friend_requests
             WHERE status = 'accepted' AND (from_email = ?1 OR to_email = ?1)
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![email], |row| {
            let from_email: String = row.get(0)?;
            let to_email: String = row.get(1)?;
            let friend_email = if from_email.eq_ignore_ascii_case(&email) {
                to_email
            } else {
                from_email
            };
            Ok(StoredFriendSummary { friend_email })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    fn sorted_message_participants(left: &str, right: &str) -> (String, String) {
        let left = Self::normalize_social_email(left);
        let right = Self::normalize_social_email(right);
        if left <= right {
            (left, right)
        } else {
            (right, left)
        }
    }

    fn other_message_participant(viewer_email: &str, participant_a: &str, participant_b: &str) -> String {
        if participant_a.eq_ignore_ascii_case(viewer_email) {
            participant_b.to_string()
        } else {
            participant_a.to_string()
        }
    }

    pub fn get_message_thread(
        &self,
        left_email: &str,
        right_email: &str,
    ) -> Result<Option<StoredMessageThread>, StorageError> {
        let (participant_a, participant_b) = Self::sorted_message_participants(left_email, right_email);
        let conn = self.conn.lock().expect("storage lock");
        let result = conn.query_row(
            "SELECT id, participant_a, participant_b, status, initiated_by, created_at, responded_at
             FROM message_threads
             WHERE participant_a = ?1 AND participant_b = ?2",
            params![participant_a, participant_b],
            |row| {
                Ok(StoredMessageThread {
                    id: row.get(0)?,
                    participant_a: row.get(1)?,
                    participant_b: row.get(2)?,
                    status: row.get(3)?,
                    initiated_by: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u64,
                    responded_at: row.get::<_, Option<i64>>(6)?.map(|value| value as u64),
                })
            },
        );
        match result {
            Ok(thread) => Ok(Some(thread)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn accept_message_thread_between(
        &self,
        viewer_email: &str,
        other_email: &str,
        responded_at: u64,
    ) -> Result<(), StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let other_email = Self::normalize_social_email(other_email);
        let (participant_a, participant_b) =
            Self::sorted_message_participants(&viewer_email, &other_email);
        let conn = self.conn.lock().expect("storage lock");
        let updated = conn.execute(
            "UPDATE message_threads
             SET status = 'accepted', responded_at = ?1
             WHERE participant_a = ?2 AND participant_b = ?3 AND status = 'pending'
               AND initiated_by != ?4 COLLATE NOCASE",
            params![responded_at as i64, participant_a, participant_b, viewer_email],
        )?;
        if updated == 0 {
            return Err(StorageError::InvalidInput("message request not found".into()));
        }
        Ok(())
    }

    pub fn decline_message_thread_between(
        &self,
        viewer_email: &str,
        other_email: &str,
        responded_at: u64,
    ) -> Result<(), StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let other_email = Self::normalize_social_email(other_email);
        let (participant_a, participant_b) =
            Self::sorted_message_participants(&viewer_email, &other_email);
        let conn = self.conn.lock().expect("storage lock");
        let updated = conn.execute(
            "UPDATE message_threads
             SET status = 'declined', responded_at = ?1
             WHERE participant_a = ?2 AND participant_b = ?3 AND status = 'pending'
               AND initiated_by != ?4 COLLATE NOCASE",
            params![responded_at as i64, participant_a, participant_b, viewer_email],
        )?;
        if updated == 0 {
            return Err(StorageError::InvalidInput("message request not found".into()));
        }
        Ok(())
    }

    pub fn accept_message_threads_for_friends(
        &self,
        left_email: &str,
        right_email: &str,
        responded_at: u64,
    ) -> Result<(), StorageError> {
        let (participant_a, participant_b) =
            Self::sorted_message_participants(left_email, right_email);
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE message_threads
             SET status = 'accepted', responded_at = ?1
             WHERE participant_a = ?2 AND participant_b = ?3 AND status = 'pending'",
            params![responded_at as i64, participant_a, participant_b],
        )?;
        Ok(())
    }

    pub fn list_message_thread_partners(&self, viewer_email: &str) -> Result<Vec<String>, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT participant_a, participant_b
             FROM message_threads
             WHERE participant_a = ?1 OR participant_b = ?1",
        )?;
        let rows = stmt.query_map(params![viewer_email], |row| {
            let participant_a: String = row.get(0)?;
            let participant_b: String = row.get(1)?;
            Ok(Self::other_message_participant(
                &viewer_email,
                &participant_a,
                &participant_b,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn count_pending_message_requests(&self, viewer_email: &str) -> Result<usize, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM message_threads
             WHERE status = 'pending'
               AND initiated_by != ?1 COLLATE NOCASE
               AND (participant_a = ?1 OR participant_b = ?1)",
            params![viewer_email],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    fn can_view_message_conversation(
        &self,
        viewer_email: &str,
        other_email: &str,
    ) -> Result<bool, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let other_email = Self::normalize_social_email(other_email);
        if viewer_email == other_email {
            return Ok(false);
        }
        if self.users_block_each_other(&viewer_email, &other_email)? {
            return Ok(false);
        }
        if self.are_friends(&viewer_email, &other_email)? {
            return Ok(true);
        }
        Ok(self.get_message_thread(&viewer_email, &other_email)?.is_some())
    }

    fn ensure_message_send_allowed(
        &self,
        from_email: &str,
        to_email: &str,
        created_at: u64,
    ) -> Result<(), StorageError> {
        let from_email = Self::normalize_social_email(from_email);
        let to_email = Self::normalize_social_email(to_email);
        if from_email == to_email {
            return Err(StorageError::InvalidInput("cannot message yourself".into()));
        }
        if !self.user_exists(&to_email)? {
            return Err(StorageError::InvalidInput("user not found".into()));
        }
        if self.users_block_each_other(&from_email, &to_email)? {
            return Err(StorageError::InvalidInput("user blocked".into()));
        }
        if self.are_friends(&from_email, &to_email)? {
            let _ = self.accept_message_threads_for_friends(&from_email, &to_email, created_at);
            return Ok(());
        }

        let thread = self.get_message_thread(&from_email, &to_email)?;
        match thread {
            None => {
                let (participant_a, participant_b) =
                    Self::sorted_message_participants(&from_email, &to_email);
                let id = uuid::Uuid::new_v4().to_string();
                let conn = self.conn.lock().expect("storage lock");
                conn.execute(
                    "INSERT INTO message_threads (id, participant_a, participant_b, status, initiated_by, created_at, responded_at)
                     VALUES (?1, ?2, ?3, 'pending', ?4, ?5, NULL)",
                    params![id, participant_a, participant_b, from_email, created_at as i64],
                )?;
                Ok(())
            }
            Some(thread) if thread.status == "accepted" => Ok(()),
            Some(thread) if thread.status == "pending" && thread.initiated_by.eq_ignore_ascii_case(&from_email) => {
                Ok(())
            }
            Some(thread) if thread.status == "pending" => {
                Err(StorageError::InvalidInput(
                    "accept the message request before replying".into(),
                ))
            }
            Some(_) => Err(StorageError::InvalidInput(
                "message request declined".into(),
            )),
        }
    }

    pub fn send_friend_message(
        &self,
        from_email: &str,
        to_email: &str,
        body: &str,
        media_type: &str,
        media_url: Option<&str>,
        video_duration: Option<f32>,
        created_at: u64,
    ) -> Result<StoredFriendMessage, StorageError> {
        let from_email = Self::normalize_social_email(from_email);
        let to_email = Self::normalize_social_email(to_email);
        let body = body.trim();
        let media_type = media_type.trim();
        if body.is_empty()
            && media_url.map(str::trim).filter(|value| !value.is_empty()).is_none()
        {
            return Err(StorageError::InvalidInput("message required".into()));
        }
        if body.chars().count() > 2000 {
            return Err(StorageError::InvalidInput("message too long".into()));
        }
        if media_type != "none" && media_url.map(str::trim).filter(|v| !v.is_empty()).is_none() {
            return Err(StorageError::InvalidInput("media required".into()));
        }
        if media_type == "none" && media_url.is_some() {
            return Err(StorageError::InvalidInput("invalid media".into()));
        }

        self.ensure_message_send_allowed(&from_email, &to_email, created_at)?;

        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO friend_messages (id, from_email, to_email, body, media_type, media_url, video_duration, created_at, read_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
            params![
                id,
                from_email,
                to_email,
                body,
                if media_type.is_empty() { "none" } else { media_type },
                media_url,
                video_duration,
                created_at as i64
            ],
        )?;
        Ok(StoredFriendMessage {
            id,
            from_email,
            to_email,
            body: body.to_string(),
            media_type: if media_type.is_empty() {
                "none".to_string()
            } else {
                media_type.to_string()
            },
            media_url: media_url.map(str::to_string),
            video_duration,
            created_at,
            read_at: None,
        })
    }

    pub fn list_friend_conversation(
        &self,
        viewer_email: &str,
        friend_email: &str,
        limit: usize,
    ) -> Result<Vec<StoredFriendMessage>, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let friend_email = Self::normalize_social_email(friend_email);
        if !self.can_view_message_conversation(&viewer_email, &friend_email)? {
            return Err(StorageError::InvalidInput("cannot view conversation".into()));
        }

        let limit = limit.min(100) as i64;
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT m.id, m.from_email, m.to_email, m.body, m.media_type, m.media_url, m.video_duration, m.created_at, m.read_at
             FROM friend_messages m
             WHERE ((m.from_email = ?1 AND m.to_email = ?2) OR (m.from_email = ?2 AND m.to_email = ?1))
               AND m.deleted_for_all_at IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM friend_message_hidden h
                 WHERE h.message_id = m.id AND h.user_email = ?1 COLLATE NOCASE
               )
               AND NOT EXISTS (
                 SELECT 1 FROM friend_conversation_hidden c
                 WHERE c.user_email = ?1 COLLATE NOCASE
                   AND c.partner_email = ?2 COLLATE NOCASE
                   AND m.created_at <= c.deleted_at
               )
             ORDER BY m.created_at ASC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![viewer_email, friend_email, limit], |row| {
            Ok(StoredFriendMessage {
                id: row.get(0)?,
                from_email: row.get(1)?,
                to_email: row.get(2)?,
                body: row.get(3)?,
                media_type: row.get(4)?,
                media_url: row.get(5)?,
                video_duration: row.get(6)?,
                created_at: row.get::<_, i64>(7)? as u64,
                read_at: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn mark_friend_conversation_read(
        &self,
        viewer_email: &str,
        friend_email: &str,
        read_at: u64,
    ) -> Result<(), StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let friend_email = Self::normalize_social_email(friend_email);
        if !self.can_view_message_conversation(&viewer_email, &friend_email)? {
            return Err(StorageError::InvalidInput("cannot view conversation".into()));
        }

        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE friend_messages
             SET read_at = ?1
             WHERE from_email = ?2 AND to_email = ?3 AND read_at IS NULL",
            params![read_at as i64, friend_email, viewer_email],
        )?;
        Ok(())
    }

    pub fn count_unread_friend_messages(&self, viewer_email: &str) -> Result<usize, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_messages m
             WHERE m.to_email = ?1 AND m.read_at IS NULL
               AND m.deleted_for_all_at IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM friend_message_hidden h
                 WHERE h.message_id = m.id AND h.user_email = ?1 COLLATE NOCASE
               )
               AND NOT EXISTS (
                 SELECT 1 FROM friend_conversation_hidden c
                 WHERE c.user_email = ?1 COLLATE NOCASE
                   AND c.partner_email = m.from_email COLLATE NOCASE
                   AND m.created_at <= c.deleted_at
               )",
            params![viewer_email],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn count_unread_from_friend(
        &self,
        viewer_email: &str,
        friend_email: &str,
    ) -> Result<usize, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let friend_email = Self::normalize_social_email(friend_email);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_messages m
             WHERE m.from_email = ?1 AND m.to_email = ?2 AND m.read_at IS NULL
               AND m.deleted_for_all_at IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM friend_message_hidden h
                 WHERE h.message_id = m.id AND h.user_email = ?2 COLLATE NOCASE
               )
               AND NOT EXISTS (
                 SELECT 1 FROM friend_conversation_hidden c
                 WHERE c.user_email = ?2 COLLATE NOCASE
                   AND c.partner_email = ?1 COLLATE NOCASE
                   AND m.created_at <= c.deleted_at
               )",
            params![friend_email, viewer_email],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn last_friend_message_with(
        &self,
        viewer_email: &str,
        friend_email: &str,
    ) -> Result<Option<StoredFriendMessage>, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let friend_email = Self::normalize_social_email(friend_email);
        let conn = self.conn.lock().expect("storage lock");
        let result = conn.query_row(
            "SELECT m.id, m.from_email, m.to_email, m.body, m.media_type, m.media_url, m.video_duration, m.created_at, m.read_at
             FROM friend_messages m
             WHERE ((m.from_email = ?1 AND m.to_email = ?2) OR (m.from_email = ?2 AND m.to_email = ?1))
               AND m.deleted_for_all_at IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM friend_message_hidden h
                 WHERE h.message_id = m.id AND h.user_email = ?1 COLLATE NOCASE
               )
               AND NOT EXISTS (
                 SELECT 1 FROM friend_conversation_hidden c
                 WHERE c.user_email = ?1 COLLATE NOCASE
                   AND c.partner_email = ?2 COLLATE NOCASE
                   AND m.created_at <= c.deleted_at
               )
             ORDER BY m.created_at DESC
             LIMIT 1",
            params![viewer_email, friend_email],
            |row| {
                Ok(StoredFriendMessage {
                    id: row.get(0)?,
                    from_email: row.get(1)?,
                    to_email: row.get(2)?,
                    body: row.get(3)?,
                    media_type: row.get(4)?,
                    media_url: row.get(5)?,
                    video_duration: row.get(6)?,
                    created_at: row.get::<_, i64>(7)? as u64,
                    read_at: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
                })
            },
        );
        match result {
            Ok(message) => Ok(Some(message)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn friend_conversation_hidden_for_user(
        &self,
        viewer_email: &str,
        partner_email: &str,
    ) -> Result<bool, StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let partner_email = Self::normalize_social_email(partner_email);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_conversation_hidden
             WHERE user_email = ?1 AND partner_email = ?2",
            params![viewer_email, partner_email],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn friend_message_participant(
        message: &StoredFriendMessage,
        user_email: &str,
    ) -> bool {
        message.from_email.eq_ignore_ascii_case(user_email)
            || message.to_email.eq_ignore_ascii_case(user_email)
    }

    pub fn get_friend_message(&self, message_id: &str) -> Result<Option<StoredFriendMessage>, StorageError> {
        let message_id = message_id.trim();
        if message_id.is_empty() {
            return Ok(None);
        }
        let conn = self.conn.lock().expect("storage lock");
        let result = conn.query_row(
            "SELECT id, from_email, to_email, body, media_type, media_url, video_duration, created_at, read_at
             FROM friend_messages
             WHERE id = ?1",
            params![message_id],
            |row| {
                Ok(StoredFriendMessage {
                    id: row.get(0)?,
                    from_email: row.get(1)?,
                    to_email: row.get(2)?,
                    body: row.get(3)?,
                    media_type: row.get(4)?,
                    media_url: row.get(5)?,
                    video_duration: row.get(6)?,
                    created_at: row.get::<_, i64>(7)? as u64,
                    read_at: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
                })
            },
        );
        match result {
            Ok(message) => Ok(Some(message)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn hide_friend_message_for_user(
        &self,
        message_id: &str,
        user_email: &str,
        deleted_at: u64,
    ) -> Result<(), StorageError> {
        let user_email = Self::normalize_social_email(user_email);
        let Some(message) = self.get_friend_message(message_id)? else {
            return Err(StorageError::InvalidInput("message not found".into()));
        };
        if !Self::friend_message_participant(&message, &user_email) {
            return Err(StorageError::InvalidInput("not allowed".into()));
        }
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO friend_message_hidden (message_id, user_email, deleted_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(message_id, user_email) DO UPDATE SET deleted_at = excluded.deleted_at",
            params![message.id, user_email, deleted_at as i64],
        )?;
        Ok(())
    }

    pub fn delete_friend_message_for_all(
        &self,
        message_id: &str,
        user_email: &str,
        deleted_at: u64,
    ) -> Result<StoredFriendMessage, StorageError> {
        let user_email = Self::normalize_social_email(user_email);
        let Some(message) = self.get_friend_message(message_id)? else {
            return Err(StorageError::InvalidInput("message not found".into()));
        };
        if !Self::friend_message_participant(&message, &user_email) {
            return Err(StorageError::InvalidInput("not allowed".into()));
        }
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE friend_messages SET deleted_for_all_at = ?1 WHERE id = ?2",
            params![deleted_at as i64, message.id],
        )?;
        Ok(message)
    }

    pub fn hide_friend_conversation_for_user(
        &self,
        viewer_email: &str,
        partner_email: &str,
        deleted_at: u64,
    ) -> Result<(), StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let partner_email = Self::normalize_social_email(partner_email);
        if !self.can_view_message_conversation(&viewer_email, &partner_email)? {
            return Err(StorageError::InvalidInput("cannot view conversation".into()));
        }
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO friend_conversation_hidden (user_email, partner_email, deleted_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(user_email, partner_email) DO UPDATE SET deleted_at = excluded.deleted_at",
            params![viewer_email, partner_email, deleted_at as i64],
        )?;
        Ok(())
    }

    pub fn delete_friend_conversation_for_all(
        &self,
        viewer_email: &str,
        partner_email: &str,
        deleted_at: u64,
    ) -> Result<(), StorageError> {
        let viewer_email = Self::normalize_social_email(viewer_email);
        let partner_email = Self::normalize_social_email(partner_email);
        if !self.can_view_message_conversation(&viewer_email, &partner_email)? {
            return Err(StorageError::InvalidInput("cannot view conversation".into()));
        }
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE friend_messages
             SET deleted_for_all_at = ?1
             WHERE deleted_for_all_at IS NULL
               AND ((from_email = ?2 AND to_email = ?3) OR (from_email = ?3 AND to_email = ?2))",
            params![deleted_at as i64, viewer_email, partner_email],
        )?;
        Ok(())
    }

    fn legacy_media_item(post: &StoredSocialPost) -> Option<StoredSocialPostMedia> {
        let url = post.media_url.as_deref().filter(|value| !value.is_empty())?;
        if post.media_type == "none" {
            return None;
        }
        Some(StoredSocialPostMedia {
            media_type: post.media_type.clone(),
            media_url: url.to_string(),
            video_duration: post.video_duration,
            sort_order: 0,
        })
    }

    pub fn hydrate_social_posts_media(
        &self,
        posts: &mut [StoredSocialPost],
    ) -> Result<(), StorageError> {
        if posts.is_empty() {
            return Ok(());
        }
        let placeholders = std::iter::repeat("?")
            .take(posts.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT post_id, sort_order, media_type, media_url, video_duration
             FROM social_post_media
             WHERE post_id IN ({placeholders})
             ORDER BY post_id, sort_order ASC"
        );
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(&sql)?;
        let ids: Vec<String> = posts.iter().map(|post| post.id.clone()).collect();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                StoredSocialPostMedia {
                    sort_order: row.get::<_, i64>(1)? as u32,
                    media_type: row.get(2)?,
                    media_url: row.get(3)?,
                    video_duration: row.get::<_, Option<f64>>(4)?.map(|value| value as f32),
                },
            ))
        })?;
        let mut grouped: std::collections::HashMap<String, Vec<StoredSocialPostMedia>> =
            std::collections::HashMap::new();
        for row in rows {
            let (post_id, item) = row?;
            grouped.entry(post_id).or_default().push(item);
        }
        for post in posts.iter_mut() {
            if let Some(items) = grouped.remove(&post.id) {
                post.media_items = items;
            } else if let Some(item) = Self::legacy_media_item(post) {
                post.media_items = vec![item];
            }
        }
        Ok(())
    }

    pub fn create_social_post(
        &self,
        user_id: &str,
        author_username: &str,
        body: &str,
        media_items: &[StoredSocialPostMedia],
        is_private: bool,
        created_at: u64,
    ) -> Result<StoredSocialPost, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let body = body.trim();
        if body.is_empty() && media_items.is_empty() {
            return Err(StorageError::InvalidInput("post requires text or media".into()));
        }
        if body.chars().count() > 2000 {
            return Err(StorageError::InvalidInput("caption too long".into()));
        }
        if media_items.len() > crate::social_posts::MAX_SOCIAL_PHOTOS_PER_POST {
            return Err(StorageError::InvalidInput(format!(
                "posts can include up to {} photos",
                crate::social_posts::MAX_SOCIAL_PHOTOS_PER_POST
            )));
        }

        let photo_count = media_items
            .iter()
            .filter(|item| item.media_type == "photo")
            .count();
        let video_count = media_items
            .iter()
            .filter(|item| item.media_type == "video")
            .count();
        if photo_count > 0 && video_count > 0 {
            return Err(StorageError::InvalidInput(
                "choose photos or one video, not both".into(),
            ));
        }
        if video_count > 1 {
            return Err(StorageError::InvalidInput(
                "posts can include only one video".into(),
            ));
        }
        if photo_count > crate::social_posts::MAX_SOCIAL_PHOTOS_PER_POST {
            return Err(StorageError::InvalidInput(format!(
                "posts can include up to {} photos",
                crate::social_posts::MAX_SOCIAL_PHOTOS_PER_POST
            )));
        }
        for item in media_items {
            if item.media_url.trim().is_empty() {
                return Err(StorageError::InvalidInput("media required".into()));
            }
            if !matches!(item.media_type.as_str(), "photo" | "video") {
                return Err(StorageError::InvalidInput("invalid media type".into()));
            }
            if item.media_type == "video" {
                let duration = item.video_duration.unwrap_or(0.0);
                if duration <= 0.0 || duration > crate::social_posts::MAX_SOCIAL_VIDEO_SECONDS {
                    return Err(StorageError::InvalidInput(
                        "video must be 10 seconds or less".into(),
                    ));
                }
            }
        }

        let primary = media_items.first();
        let media_type = primary
            .map(|item| item.media_type.as_str())
            .unwrap_or("none")
            .to_string();
        let media_url = primary.map(|item| item.media_url.clone());
        let video_duration = primary.and_then(|item| item.video_duration);

        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO social_posts (id, user_id, author_username, body, media_type, media_url, video_duration, is_private, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                user_id,
                author_username.trim(),
                body,
                media_type,
                media_url.as_deref(),
                video_duration,
                if is_private { 1_i64 } else { 0_i64 },
                created_at as i64
            ],
        )?;
        for item in media_items {
            let media_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO social_post_media (id, post_id, sort_order, media_type, media_url, video_duration)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    media_id,
                    id,
                    item.sort_order as i64,
                    item.media_type,
                    item.media_url,
                    item.video_duration
                ],
            )?;
        }
        Ok(StoredSocialPost {
            id,
            user_id,
            author_username: author_username.trim().to_string(),
            body: body.to_string(),
            media_type: if media_items.is_empty() {
                "none".to_string()
            } else {
                media_type
            },
            media_url,
            video_duration,
            is_private,
            created_at,
            post_kind: "standard".to_string(),
            wrapped_payload: None,
            wrapped_year: None,
            wrapped_month: None,
            media_items: media_items.to_vec(),
            upvotes: 0,
            viewer_upvoted: false,
            comments: Vec::new(),
        })
    }

    fn social_post_upvote_count(conn: &Connection, post_id: &str) -> Result<u32, rusqlite::Error> {
        conn.query_row(
            "SELECT COUNT(*) FROM social_post_votes WHERE post_id = ?1",
            params![post_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count as u32)
    }

    fn social_post_viewer_upvoted(
        conn: &Connection,
        post_id: &str,
        viewer_email: &str,
    ) -> Result<bool, rusqlite::Error> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM social_post_votes WHERE post_id = ?1 AND user_id = ?2",
            params![post_id, viewer_email],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn social_comment_upvote_count(conn: &Connection, comment_id: &str) -> Result<u32, rusqlite::Error> {
        conn.query_row(
            "SELECT COUNT(*) FROM social_comment_votes WHERE comment_id = ?1",
            params![comment_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count as u32)
    }

    fn social_comment_viewer_upvoted(
        conn: &Connection,
        comment_id: &str,
        viewer_email: &str,
    ) -> Result<bool, rusqlite::Error> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM social_comment_votes WHERE comment_id = ?1 AND user_id = ?2",
            params![comment_id, viewer_email],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn hydrate_social_posts_engagement(
        &self,
        posts: &mut [StoredSocialPost],
        viewer_email: Option<&str>,
    ) -> Result<(), StorageError> {
        if posts.is_empty() {
            return Ok(());
        }
        let viewer_email = viewer_email.map(Self::normalize_social_email);
        let conn = self.conn.lock().expect("storage lock");
        for post in posts.iter_mut() {
            post.upvotes = Self::social_post_upvote_count(&conn, &post.id)?;
            post.viewer_upvoted = viewer_email
                .as_ref()
                .map(|email| Self::social_post_viewer_upvoted(&conn, &post.id, email))
                .transpose()?
                .unwrap_or(false);

            let mut comment_stmt = conn.prepare(
                "SELECT id, post_id, user_id, author_username, body, created_at
                 FROM social_post_comments
                 WHERE post_id = ?1
                 ORDER BY created_at ASC",
            )?;
            let rows = comment_stmt.query_map(params![post.id], |row| {
                Ok(StoredSocialPostComment {
                    id: row.get(0)?,
                    post_id: row.get(1)?,
                    user_id: row.get(2)?,
                    author_username: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u64,
                    upvotes: 0,
                    viewer_upvoted: false,
                })
            })?;
            let mut comments = rows.collect::<Result<Vec<_>, _>>()?;
            for comment in &mut comments {
                comment.upvotes = Self::social_comment_upvote_count(&conn, &comment.id)?;
                comment.viewer_upvoted = viewer_email
                    .as_ref()
                    .map(|email| Self::social_comment_viewer_upvoted(&conn, &comment.id, email))
                    .transpose()?
                    .unwrap_or(false);
            }
            comments.sort_by(|left, right| {
                right
                    .upvotes
                    .cmp(&left.upvotes)
                    .then_with(|| left.created_at.cmp(&right.created_at))
            });
            post.comments = comments;
        }
        Ok(())
    }

    pub fn toggle_social_post_upvote(
        &self,
        post_id: &str,
        user_id: &str,
        created_at: u64,
    ) -> Result<SocialPostUpvoteSummary, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let conn = self.conn.lock().expect("storage lock");
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM social_posts WHERE id = ?1",
            params![post_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::InvalidInput("post not found".into()));
        }

        let already_voted = Self::social_post_viewer_upvoted(&conn, post_id, &user_id)?;
        if already_voted {
            conn.execute(
                "DELETE FROM social_post_votes WHERE post_id = ?1 AND user_id = ?2",
                params![post_id, user_id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO social_post_votes (post_id, user_id, created_at) VALUES (?1, ?2, ?3)",
                params![post_id, user_id, created_at as i64],
            )?;
        }

        Ok(SocialPostUpvoteSummary {
            upvotes: Self::social_post_upvote_count(&conn, post_id)?,
            viewer_upvoted: !already_voted,
        })
    }

    pub fn toggle_social_comment_upvote(
        &self,
        comment_id: &str,
        user_id: &str,
        created_at: u64,
    ) -> Result<SocialCommentUpvoteSummary, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let conn = self.conn.lock().expect("storage lock");
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM social_post_comments WHERE id = ?1",
            params![comment_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::InvalidInput("comment not found".into()));
        }

        let already_voted =
            Self::social_comment_viewer_upvoted(&conn, comment_id, &user_id)?;
        if already_voted {
            conn.execute(
                "DELETE FROM social_comment_votes WHERE comment_id = ?1 AND user_id = ?2",
                params![comment_id, user_id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO social_comment_votes (comment_id, user_id, created_at) VALUES (?1, ?2, ?3)",
                params![comment_id, user_id, created_at as i64],
            )?;
        }

        Ok(SocialCommentUpvoteSummary {
            upvotes: Self::social_comment_upvote_count(&conn, comment_id)?,
            viewer_upvoted: !already_voted,
        })
    }

    pub fn get_social_post_id_for_comment(
        &self,
        comment_id: &str,
    ) -> Result<Option<String>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        match conn.query_row(
            "SELECT post_id FROM social_post_comments WHERE id = ?1",
            params![comment_id],
            |row| row.get(0),
        ) {
            Ok(post_id) => Ok(Some(post_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn create_social_post_comment(
        &self,
        post_id: &str,
        user_id: &str,
        author_username: &str,
        body: &str,
        created_at: u64,
    ) -> Result<StoredSocialPostComment, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let body = body.trim();
        if body.is_empty() {
            return Err(StorageError::InvalidInput("comment required".into()));
        }
        if body.chars().count() > 1000 {
            return Err(StorageError::InvalidInput("comment too long".into()));
        }

        let conn = self.conn.lock().expect("storage lock");
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM social_posts WHERE id = ?1",
            params![post_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::InvalidInput("post not found".into()));
        }

        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO social_post_comments (id, post_id, user_id, author_username, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                post_id,
                user_id,
                author_username.trim(),
                body,
                created_at as i64
            ],
        )?;

        Ok(StoredSocialPostComment {
            id,
            post_id: post_id.to_string(),
            user_id,
            author_username: author_username.trim().to_string(),
            body: body.to_string(),
            created_at,
            upvotes: 0,
            viewer_upvoted: false,
        })
    }

    pub fn delete_social_post_comment_owned(
        &self,
        comment_id: &str,
        user_id: &str,
    ) -> Result<ForumDeleteOutcome, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let conn = self.conn.lock().expect("storage lock");
        let owner: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT user_id FROM social_post_comments WHERE id = ?1",
            params![comment_id],
            |row| row.get(0),
        );
        let owner = match owner {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(ForumDeleteOutcome::NotFound);
            }
            Err(error) => return Err(error.into()),
        };
        if !owner.eq_ignore_ascii_case(&user_id) {
            return Ok(ForumDeleteOutcome::NotAuthorized);
        }

        conn.execute(
            "DELETE FROM social_comment_votes WHERE comment_id = ?1",
            params![comment_id],
        )?;
        conn.execute(
            "DELETE FROM social_post_comments WHERE id = ?1",
            params![comment_id],
        )?;
        Ok(ForumDeleteOutcome::Deleted)
    }

    pub fn get_social_post_by_id(
        &self,
        post_id: &str,
        viewer_email: Option<&str>,
    ) -> Result<Option<StoredSocialPost>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let sql = format!(
            "SELECT {SOCIAL_POST_SELECT} FROM social_posts WHERE id = ?1"
        );
        let result = conn.query_row(&sql, params![post_id], map_social_post_row);
        let mut post = match result {
            Ok(post) => post,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        drop(conn);
        self.hydrate_social_posts_media(std::slice::from_mut(&mut post))?;
        self.hydrate_social_posts_engagement(std::slice::from_mut(&mut post), viewer_email)?;
        Ok(Some(post))
    }

    pub fn list_social_posts(&self, limit: usize) -> Result<Vec<StoredSocialPost>, StorageError> {
        let limit = limit.min(100) as i64;
        let mut posts = {
            let conn = self.conn.lock().expect("storage lock");
            let sql = format!(
                "SELECT {SOCIAL_POST_SELECT}
                 FROM social_posts
                 ORDER BY created_at DESC
                 LIMIT ?1"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![limit], map_social_post_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        self.hydrate_social_posts_media(&mut posts)?;
        Ok(posts)
    }

    pub fn list_social_posts_from_users(
        &self,
        user_ids: &[String],
        limit: usize,
    ) -> Result<Vec<StoredSocialPost>, StorageError> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.min(100) as i64;
        let placeholders = std::iter::repeat("?")
            .take(user_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {SOCIAL_POST_SELECT}
             FROM social_posts
             WHERE LOWER(user_id) IN ({placeholders})
             ORDER BY created_at DESC
             LIMIT ?{}",
            user_ids.len() + 1
        );
        let mut posts = {
            let conn = self.conn.lock().expect("storage lock");
            let mut stmt = conn.prepare(&sql)?;
            let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = user_ids
                .iter()
                .map(|email| {
                    Box::new(Self::normalize_social_email(email)) as Box<dyn rusqlite::types::ToSql>
                })
                .collect();
            params_vec.push(Box::new(limit));
            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|value| value.as_ref()).collect();
            let rows = stmt.query_map(params_refs.as_slice(), map_social_post_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        self.hydrate_social_posts_media(&mut posts)?;
        Ok(posts)
    }

    pub fn list_social_posts_from_users_with_engagement(
        &self,
        user_ids: &[String],
        limit: usize,
        viewer_email: Option<&str>,
    ) -> Result<Vec<StoredSocialPost>, StorageError> {
        let mut posts = self.list_social_posts_from_users(user_ids, limit)?;
        self.hydrate_social_posts_engagement(&mut posts, viewer_email)?;
        Ok(posts)
    }

    pub fn list_social_posts_with_engagement(
        &self,
        limit: usize,
        viewer_email: Option<&str>,
    ) -> Result<Vec<StoredSocialPost>, StorageError> {
        let mut posts = self.list_social_posts(limit)?;
        self.hydrate_social_posts_engagement(&mut posts, viewer_email)?;
        Ok(posts)
    }

    pub fn delete_social_post_owned(
        &self,
        post_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<String>>, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let conn = self.conn.lock().expect("storage lock");
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM social_posts WHERE id = ?1 AND user_id = ?2",
                params![post_id, user_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);
        if !exists {
            return Ok(None);
        }

        let mut media_urls: Vec<String> = conn
            .prepare("SELECT media_url FROM social_post_media WHERE post_id = ?1 ORDER BY sort_order ASC")?
            .query_map(params![post_id], |row| row.get::<_, String>(0))?
            .filter_map(Result::ok)
            .collect();
        if media_urls.is_empty() {
            if let Ok(url) = conn.query_row(
                "SELECT media_url FROM social_posts WHERE id = ?1 AND user_id = ?2",
                params![post_id, user_id],
                |row| row.get::<_, Option<String>>(0),
            ) {
                if let Some(url) = url.filter(|value| !value.is_empty()) {
                    media_urls.push(url);
                }
            }
        }

        conn.execute(
            "DELETE FROM social_comment_votes
             WHERE comment_id IN (SELECT id FROM social_post_comments WHERE post_id = ?1)",
            params![post_id],
        )?;
        conn.execute(
            "DELETE FROM social_post_comments WHERE post_id = ?1",
            params![post_id],
        )?;
        conn.execute(
            "DELETE FROM social_post_votes WHERE post_id = ?1",
            params![post_id],
        )?;
        conn.execute(
            "DELETE FROM social_post_media WHERE post_id = ?1",
            params![post_id],
        )?;
        let deleted = conn.execute(
            "DELETE FROM social_posts WHERE id = ?1 AND user_id = ?2",
            params![post_id, user_id],
        )?;
        if deleted == 0 {
            return Ok(None);
        }
        Ok(Some(media_urls))
    }

    pub fn has_monthly_wrapped(
        &self,
        user_id: &str,
        year: u32,
        month: u32,
    ) -> Result<bool, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM social_posts
             WHERE user_id = ?1 AND post_kind = 'monthly_wrapped'
               AND wrapped_year = ?2 AND wrapped_month = ?3",
            params![user_id, year as i64, month as i64],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn list_user_standard_posts_in_range(
        &self,
        user_id: &str,
        start_ts: u64,
        end_ts: u64,
    ) -> Result<Vec<StoredSocialPost>, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let sql = format!(
            "SELECT {SOCIAL_POST_SELECT}
             FROM social_posts
             WHERE user_id = ?1
               AND created_at >= ?2
               AND created_at <= ?3
               AND post_kind = 'standard'
             ORDER BY created_at ASC"
        );
        let mut posts = {
            let conn = self.conn.lock().expect("storage lock");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                params![user_id, start_ts as i64, end_ts as i64],
                map_social_post_row,
            )?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        self.hydrate_social_posts_media(&mut posts)?;
        Ok(posts)
    }

    pub fn create_monthly_wrapped_post(
        &self,
        user_id: &str,
        author_username: &str,
        body: &str,
        wrapped_payload: &str,
        year: u32,
        month: u32,
        created_at: u64,
    ) -> Result<StoredSocialPost, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let body = body.trim();
        if body.is_empty() {
            return Err(StorageError::InvalidInput("wrapped post requires text".into()));
        }
        if wrapped_payload.trim().is_empty() {
            return Err(StorageError::InvalidInput("wrapped payload required".into()));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO social_posts (
                 id, user_id, author_username, body, media_type, media_url, video_duration,
                 is_private, created_at, post_kind, wrapped_payload, wrapped_year, wrapped_month
             ) VALUES (?1, ?2, ?3, ?4, 'none', NULL, NULL, 0, ?5, 'monthly_wrapped', ?6, ?7, ?8)",
            params![
                id,
                user_id,
                author_username.trim(),
                body,
                created_at as i64,
                wrapped_payload,
                year as i64,
                month as i64
            ],
        )?;
        Ok(StoredSocialPost {
            id,
            user_id,
            author_username: author_username.trim().to_string(),
            body: body.to_string(),
            media_type: "none".to_string(),
            media_url: None,
            video_duration: None,
            is_private: false,
            created_at,
            post_kind: "monthly_wrapped".to_string(),
            wrapped_payload: Some(wrapped_payload.to_string()),
            wrapped_year: Some(year),
            wrapped_month: Some(month),
            media_items: Vec::new(),
            upvotes: 0,
            viewer_upvoted: false,
            comments: Vec::new(),
        })
    }

    pub fn has_pet_id_post(&self, user_id: &str, pet_id: &str) -> Result<bool, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT wrapped_payload FROM social_posts
             WHERE user_id = ?1 AND post_kind = 'pet_id'",
        )?;
        let rows = stmt.query_map(params![user_id], |row| row.get::<_, Option<String>>(0))?;
        for row in rows {
            let json = row?;
            let Some(json) = json else { continue };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
                continue;
            };
            if value
                .get("pet_id")
                .and_then(|part| part.as_str())
                .is_some_and(|stored| stored == pet_id)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn create_pet_id_post(
        &self,
        user_id: &str,
        author_username: &str,
        body: &str,
        wrapped_payload: &str,
        created_at: u64,
    ) -> Result<StoredSocialPost, StorageError> {
        let user_id = Self::normalize_social_email(user_id);
        let body = body.trim();
        if body.is_empty() {
            return Err(StorageError::InvalidInput("pet id post requires text".into()));
        }
        if wrapped_payload.trim().is_empty() {
            return Err(StorageError::InvalidInput("pet id payload required".into()));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO social_posts (
                 id, user_id, author_username, body, media_type, media_url, video_duration,
                 is_private, created_at, post_kind, wrapped_payload, wrapped_year, wrapped_month
             ) VALUES (?1, ?2, ?3, ?4, 'none', NULL, NULL, 0, ?5, 'pet_id', ?6, NULL, NULL)",
            params![
                id,
                user_id,
                author_username.trim(),
                body,
                created_at as i64,
                wrapped_payload,
            ],
        )?;
        Ok(StoredSocialPost {
            id,
            user_id,
            author_username: author_username.trim().to_string(),
            body: body.to_string(),
            media_type: "none".to_string(),
            media_url: None,
            video_duration: None,
            is_private: false,
            created_at,
            post_kind: "pet_id".to_string(),
            wrapped_payload: Some(wrapped_payload.to_string()),
            wrapped_year: None,
            wrapped_month: None,
            media_items: Vec::new(),
            upvotes: 0,
            viewer_upvoted: false,
            comments: Vec::new(),
        })
    }

    pub fn create_pet_share(
        &self,
        owner_email: &str,
        shared_with_email: &str,
        pet_id: &str,
        created_at: u64,
    ) -> Result<(), StorageError> {
        let owner_email = Self::normalize_social_email(owner_email);
        let shared_with_email = Self::normalize_social_email(shared_with_email);
        let pet_id = pet_id.trim();
        if owner_email == shared_with_email {
            return Err(StorageError::InvalidInput("cannot share with yourself".into()));
        }
        if pet_id.is_empty() {
            return Err(StorageError::InvalidInput("pet required".into()));
        }
        if !self.are_friends(&owner_email, &shared_with_email)? {
            return Err(StorageError::InvalidInput("not friends".into()));
        }

        let conn = self.conn.lock().expect("storage lock");
        let existing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pet_shares
             WHERE owner_email = ?1 AND shared_with_email = ?2 AND pet_id = ?3
               AND status IN ('pending', 'accepted')",
            params![owner_email, shared_with_email, pet_id],
            |row| row.get(0),
        )?;
        if existing > 0 {
            return Err(StorageError::InvalidInput("already shared".into()));
        }

        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO pet_shares (id, owner_email, shared_with_email, pet_id, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
            params![id, owner_email, shared_with_email, pet_id, created_at as i64],
        )?;
        Ok(())
    }

    pub fn respond_pet_share(
        &self,
        share_id: &str,
        recipient_email: &str,
        accept: bool,
    ) -> Result<(), StorageError> {
        let recipient_email = Self::normalize_social_email(recipient_email);
        let status = if accept { "accepted" } else { "declined" };
        let conn = self.conn.lock().expect("storage lock");
        let updated = conn.execute(
            "UPDATE pet_shares SET status = ?1
             WHERE id = ?2 AND shared_with_email = ?3 COLLATE NOCASE AND status = 'pending'",
            params![status, share_id, recipient_email],
        )?;
        if updated == 0 {
            return Err(StorageError::InvalidInput("share not found".into()));
        }
        Ok(())
    }

    pub fn list_incoming_pet_shares(
        &self,
        email: &str,
    ) -> Result<Vec<StoredPetShare>, StorageError> {
        let email = Self::normalize_social_email(email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, owner_email, shared_with_email, pet_id, status, created_at
             FROM pet_shares
             WHERE shared_with_email = ?1 COLLATE NOCASE AND status = 'pending'
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![email], |row| {
            Ok(StoredPetShare {
                id: row.get(0)?,
                owner_email: row.get(1)?,
                shared_with_email: row.get(2)?,
                pet_id: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get::<_, i64>(5)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn list_accepted_pet_shares_for_recipient(
        &self,
        email: &str,
    ) -> Result<Vec<StoredPetShare>, StorageError> {
        let email = Self::normalize_social_email(email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, owner_email, shared_with_email, pet_id, status, created_at
             FROM pet_shares
             WHERE shared_with_email = ?1 COLLATE NOCASE AND status = 'accepted'
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![email], |row| {
            Ok(StoredPetShare {
                id: row.get(0)?,
                owner_email: row.get(1)?,
                shared_with_email: row.get(2)?,
                pet_id: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get::<_, i64>(5)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn has_accepted_pet_share(
        &self,
        owner_email: &str,
        recipient_email: &str,
        pet_id: &str,
    ) -> Result<bool, StorageError> {
        let owner_email = Self::normalize_social_email(owner_email);
        let recipient_email = Self::normalize_social_email(recipient_email);
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pet_shares
             WHERE owner_email = ?1 AND shared_with_email = ?2 AND pet_id = ?3 AND status = 'accepted'",
            params![owner_email, recipient_email, pet_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn list_outgoing_pet_shares(
        &self,
        owner_email: &str,
    ) -> Result<Vec<StoredPetShare>, StorageError> {
        let owner_email = Self::normalize_social_email(owner_email);
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, owner_email, shared_with_email, pet_id, status, created_at
             FROM pet_shares
             WHERE owner_email = ?1 COLLATE NOCASE AND status IN ('pending', 'accepted')
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![owner_email], |row| {
            Ok(StoredPetShare {
                id: row.get(0)?,
                owner_email: row.get(1)?,
                shared_with_email: row.get(2)?,
                pet_id: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get::<_, i64>(5)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }

    pub fn revoke_pet_share(&self, share_id: &str, owner_email: &str) -> Result<(), StorageError> {
        let owner_email = Self::normalize_social_email(owner_email);
        let conn = self.conn.lock().expect("storage lock");
        let removed = conn.execute(
            "DELETE FROM pet_shares
             WHERE id = ?1 AND owner_email = ?2 COLLATE NOCASE AND status IN ('pending', 'accepted')",
            params![share_id, owner_email],
        )?;
        if removed == 0 {
            return Err(StorageError::InvalidInput("share not found".into()));
        }
        Ok(())
    }

    pub fn revoke_pet_shares_for_pet(
        &self,
        owner_email: &str,
        pet_id: &str,
    ) -> Result<u32, StorageError> {
        let owner_email = Self::normalize_social_email(owner_email);
        let conn = self.conn.lock().expect("storage lock");
        let removed = conn.execute(
            "DELETE FROM pet_shares
             WHERE owner_email = ?1 COLLATE NOCASE AND pet_id = ?2
               AND status IN ('pending', 'accepted')",
            params![owner_email, pet_id],
        )?;
        Ok(removed as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_user(email: &str, password: &str) -> User {
        User {
            username: "testuser".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            email: email.to_string(),
            password: password.to_string(),
            created_at: 1,
        }
    }

    #[test]
    fn user_save_login_and_reopen_persists() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let email = "persist@test.local";

        storage
            .save_user(&test_user(email, "SecretPass1!"))
            .expect("save user");
        assert!(storage
            .validate_login(email, "SecretPass1!")
            .expect("validate"));
        assert!(!storage
            .validate_login(email, "wrong")
            .expect("validate wrong"));

        drop(storage);

        let storage = Storage::open_at(temp.path().to_path_buf()).expect("reopen storage");
        assert!(storage.user_exists(email).expect("exists"));
        assert!(storage
            .validate_login(email, "SecretPass1!")
            .expect("validate after reopen"));
    }

    #[test]
    fn passwords_are_stored_hashed_not_plaintext() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let email = "hash@test.local";
        let plain = "MyPassword99";

        storage
            .save_user(&test_user(email, plain))
            .expect("save user");

        let user = storage
            .find_user_by_email(email)
            .expect("find")
            .expect("user");
        assert!(is_bcrypt_hash(&user.password));
        assert_ne!(user.password, plain);
    }

    #[test]
    fn legacy_plaintext_password_upgrades_on_login() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let email = "legacy@test.local";
        let plain = "OldPlainPass";

        {
            let conn = storage.conn.lock().expect("lock");
            conn.execute(
                "INSERT INTO users (email, username, first_name, last_name, password, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![email, "legacyuser", "Legacy", "", plain, 1_i64],
            )
            .expect("insert legacy user");
        }

        assert!(storage.validate_login(email, plain).expect("legacy login"));

        let user = storage
            .find_user_by_email(email)
            .expect("find")
            .expect("user");
        assert!(is_bcrypt_hash(&user.password));
    }

    #[test]
    fn legacy_name_column_migrates_and_allows_new_signups() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_dir = temp.path().to_path_buf();
        {
            let db_path = data_dir.join("whiskerwatch.db");
            let conn = Connection::open(db_path).expect("open legacy db");
            conn.execute_batch(
                "CREATE TABLE users (
                     email TEXT PRIMARY KEY COLLATE NOCASE,
                     name TEXT NOT NULL,
                     password TEXT NOT NULL,
                     created_at INTEGER NOT NULL
                 );",
            )
            .expect("create legacy table");
            conn.execute(
                "INSERT INTO users (email, name, password, created_at) VALUES (?1, ?2, ?3, ?4)",
                params!["legacy@test.local", "Legacy Name", "plainpass", 1_i64],
            )
            .expect("seed legacy user");
        }

        let storage = Storage::open_at(data_dir.clone()).expect("open migrated storage");
        storage
            .save_user(&test_user("new@test.local", "NewPass1!"))
            .expect("save after legacy migration");

        let conn = Connection::open(data_dir.join("whiskerwatch.db")).expect("inspect db");
        assert!(
            !Storage::table_has_column(&conn, "users", "name").expect("check name column"),
            "legacy name column should be removed"
        );
        assert!(storage
            .user_exists("new@test.local")
            .expect("new user exists"));
        assert!(storage
            .validate_login("legacy@test.local", "plainpass")
            .expect("legacy login"));
    }

    #[test]
    fn password_reset_token_updates_password_and_is_single_use() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let email = "reset@test.local";
        let original = "OriginalPass1!";

        storage
            .save_user(&test_user(email, original))
            .expect("save user");

        let token = storage
            .create_password_reset_token(email)
            .expect("create token");
        assert!(storage
            .find_valid_reset_token(&token)
            .expect("lookup")
            .is_some());

        storage
            .reset_password_with_token(&token, "NewSecure1!")
            .expect("reset password");

        assert!(storage
            .validate_login(email, "NewSecure1!")
            .expect("new login"));
        assert!(!storage.validate_login(email, original).expect("old login"));
        assert!(storage
            .find_valid_reset_token(&token)
            .expect("token consumed")
            .is_none());

        let err = storage
            .reset_password_with_token(&token, "AnotherPass1!")
            .expect_err("reuse token");
        assert!(matches!(err, StorageError::InvalidResetToken));
    }

    #[test]
    fn expired_reset_token_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let email = "expired@test.local";

        storage
            .save_user(&test_user(email, "OldPass1!"))
            .expect("save user");

        let token = storage
            .create_password_reset_token(email)
            .expect("create token");

        {
            let conn = storage.conn.lock().expect("lock");
            conn.execute(
                "UPDATE password_reset_tokens SET expires_at = ?1 WHERE token = ?2",
                params![1_i64, token],
            )
            .expect("expire token");
        }

        assert!(storage
            .find_valid_reset_token(&token)
            .expect("lookup")
            .is_none());
    }

    #[test]
    fn auth_sessions_survive_storage_reopen() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_dir = temp.path().to_path_buf();
        let session_id = "test-session-id";
        let email = "user@test.local";
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now + 30 * 24 * 3600;

        {
            let storage = Storage::open_at(data_dir.clone()).expect("open storage");
            storage
                .save_auth_session(session_id, "user", Some(email), now, expires_at)
                .expect("save user session");
            storage
                .save_auth_session("admin-session", "admin", None, now, expires_at)
                .expect("save admin session");
        }

        let storage = Storage::open_at(data_dir).expect("reopen storage");
        assert_eq!(
            storage
                .lookup_user_session(session_id)
                .expect("lookup user session")
                .as_deref(),
            Some(email)
        );
        assert!(storage
            .admin_session_valid("admin-session")
            .expect("lookup admin session"));
    }

    #[test]
    fn expired_auth_sessions_are_not_valid() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        storage
            .save_auth_session("expired", "user", Some("user@test.local"), 1, 2)
            .expect("save expired session");

        assert!(storage
            .lookup_user_session("expired")
            .expect("lookup")
            .is_none());
    }

    #[test]
    fn duplicate_email_returns_email_taken() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let email = "dup@test.local";

        storage
            .save_user(&test_user(email, "pass1"))
            .expect("first save");
        let err = storage
            .save_user(&test_user(email, "pass2"))
            .expect_err("duplicate email");
        assert!(matches!(err, StorageError::EmailTaken));
    }

    #[test]
    fn path_in_project_anchors_to_project_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let nested = manifest_dir.join("target").join("storage-path-test");
        fs::create_dir_all(&nested).expect("create nested dir");

        std::env::set_current_dir(&nested).expect("chdir nested");

        let template = path_in_project("templates/marketing-home.html");
        assert_eq!(template, manifest_dir.join("templates/marketing-home.html"));
        assert!(template.is_file(), "marketing template should exist");

        let styles = static_dir().join("styles.css");
        assert_eq!(styles, manifest_dir.join("static/styles.css"));
        assert!(styles.is_file(), "styles.css should exist");

        let _ = fs::remove_dir(nested);
        std::env::set_current_dir(&manifest_dir).expect("restore cwd");
    }

    #[test]
    fn relative_data_dir_anchors_to_project_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let nested = manifest_dir.join("target").join("storage-data-dir-test");
        fs::create_dir_all(&nested).expect("create nested dir");

        std::env::set_var("DATA_DIR", "data");
        std::env::set_current_dir(&nested).expect("chdir nested");

        let data_dir = data_dir_from_env();
        assert_eq!(data_dir, manifest_dir.join("data"));

        std::env::remove_var("DATA_DIR");
        let _ = fs::remove_dir(nested);
        std::env::set_current_dir(&manifest_dir).expect("restore cwd");
    }

    #[test]
    fn default_data_dir_finds_project_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let nested = manifest_dir.join("target").join("storage-test-nested");
        fs::create_dir_all(&nested).expect("create nested dir");

        std::env::set_current_dir(&nested).expect("chdir nested");

        let data_dir = default_data_dir();
        assert_eq!(data_dir, manifest_dir.join("data"));

        let _ = fs::remove_dir(nested);
        std::env::set_current_dir(&manifest_dir).expect("restore cwd");
    }

    #[test]
    fn forum_posts_and_replies_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let now = 1_700_000_000_u64;

        let post_id = storage
            .create_forum_post(
                "user@test.local",
                "catmom",
                "How often should I brush?",
                "My longhair sheds a lot.",
                "persian",
                now,
            )
            .expect("create post");

        storage
            .create_forum_reply(
                post_id,
                "helper@test.local",
                "vetfan",
                "Daily brushing helps!",
                now + 60,
            )
            .expect("create reply");

        let posts = storage.list_forum_posts(None).expect("list posts");
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].title, "How often should I brush?");
        assert_eq!(posts[0].breed_slug, "persian");

        let persian_posts = storage
            .list_forum_posts(Some("persian"))
            .expect("list persian posts");
        assert_eq!(persian_posts.len(), 1);
        assert!(storage
            .list_forum_posts(Some("siamese"))
            .expect("list siamese")
            .is_empty());

        let replies = storage.list_forum_replies(post_id).expect("list replies");
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].body, "Daily brushing helps!");

        assert_eq!(storage.count_forum_replies(post_id).expect("count"), 1);
    }

    #[test]
    fn forum_delete_respects_ownership() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let now = 1_700_000_000_u64;

        let post_id = storage
            .create_forum_post(
                "owner@test.local",
                "owner",
                "Question?",
                "Details.",
                "",
                now,
            )
            .expect("create post");

        storage
            .create_forum_reply(
                post_id,
                "owner@test.local",
                "owner",
                "My own answer.",
                now + 30,
            )
            .expect("create reply");

        storage
            .create_forum_reply(
                post_id,
                "other@test.local",
                "other",
                "Someone else's answer.",
                now + 60,
            )
            .expect("create other reply");

        let replies = storage.list_forum_replies(post_id).expect("list replies");
        let own_reply_id = replies
            .iter()
            .find(|reply| reply.user_id == "owner@test.local")
            .expect("own reply")
            .id;
        let other_reply_id = replies
            .iter()
            .find(|reply| reply.user_id == "other@test.local")
            .expect("other reply")
            .id;

        assert_eq!(
            storage
                .delete_forum_reply_owned(other_reply_id, "owner@test.local")
                .expect("delete other reply"),
            ForumDeleteOutcome::NotAuthorized
        );
        assert_eq!(
            storage
                .delete_forum_reply_owned(own_reply_id, "owner@test.local")
                .expect("delete own reply"),
            ForumDeleteOutcome::Deleted
        );
        assert_eq!(
            storage
                .delete_forum_post_owned(post_id, "other@test.local")
                .expect("delete post as other"),
            ForumDeleteOutcome::NotAuthorized
        );
        assert_eq!(
            storage
                .delete_forum_post_owned(post_id, "owner@test.local")
                .expect("delete post as owner"),
            ForumDeleteOutcome::Deleted
        );
        assert!(storage
            .list_forum_posts(None)
            .expect("list posts")
            .is_empty());
    }

    #[test]
    fn feedback_survives_storage_reopen() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_dir = temp.path().to_path_buf();
        {
            let storage = Storage::open_at(data_dir.clone()).expect("open storage");
            storage
                .save_feedback(&FeedbackSubmission {
                    id: 0,
                    name: "Tester".to_string(),
                    email: "tester@example.com".to_string(),
                    category: "idea".to_string(),
                    message: "Keep feedback after restart".to_string(),
                    submitted_at: 1_700_000_100,
                    user_id: Some("tester@example.com".to_string()),
                    author_username: "Tester".to_string(),
                })
                .expect("save feedback");
        }

        let storage = Storage::open_at(data_dir).expect("reopen storage");
        let loaded = storage.load_feedback().expect("load feedback");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].message, "Keep feedback after restart");
    }

    #[test]
    fn feedback_save_and_load_round_trips_with_user_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let submission = FeedbackSubmission {
            id: 0,
            name: "Tester".to_string(),
            email: "tester@example.com".to_string(),
            category: "bug".to_string(),
            message: "Button does not click".to_string(),
            submitted_at: 1_700_000_100,
            user_id: Some("tester@example.com".to_string()),
            author_username: "Tester".to_string(),
        };

        storage.save_feedback(&submission).expect("save feedback");
        let loaded = storage.load_feedback().expect("load feedback");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].message, "Button does not click");
        assert_eq!(loaded[0].user_id.as_deref(), Some("tester@example.com"));
    }

    #[test]
    fn feedback_votes_toggle_and_count() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let post_id = storage
            .save_feedback(&FeedbackSubmission {
                id: 0,
                name: "Idea Cat".to_string(),
                email: "idea@example.com".to_string(),
                category: "idea".to_string(),
                message: "Treat counter".to_string(),
                submitted_at: 1_700_000_100,
                user_id: Some("idea@example.com".to_string()),
                author_username: "Idea Cat".to_string(),
            })
            .expect("save feedback");

        let first = storage
            .cast_feedback_vote(post_id, "voter@example.com", 1)
            .expect("upvote");
        assert_eq!(first.upvotes, 1);
        assert_eq!(first.downvotes, 0);
        assert_eq!(first.user_vote, Some(1));

        let removed = storage
            .cast_feedback_vote(post_id, "voter@example.com", 1)
            .expect("repeat upvote removes vote");
        assert_eq!(removed.upvotes, 0);
        assert_eq!(removed.downvotes, 0);
        assert_eq!(removed.user_vote, None);

        let upvoted_again = storage
            .cast_feedback_vote(post_id, "voter@example.com", 1)
            .expect("upvote again");
        assert_eq!(upvoted_again.upvotes, 1);
        assert_eq!(upvoted_again.user_vote, Some(1));

        let switched = storage
            .cast_feedback_vote(post_id, "voter@example.com", -1)
            .expect("downvote");
        assert_eq!(switched.upvotes, 0);
        assert_eq!(switched.downvotes, 1);
        assert_eq!(switched.user_vote, Some(-1));

        let forum = storage
            .load_feedback_forum(Some("voter@example.com"))
            .expect("load forum");
        assert_eq!(forum.len(), 1);
        assert_eq!(forum[0].downvotes, 1);
        assert_eq!(forum[0].user_vote, Some(-1));
    }

    #[test]
    fn feedback_author_can_vote_and_switch_on_own_post() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let author = "idea@example.com";
        let post_id = storage
            .save_feedback(&FeedbackSubmission {
                id: 0,
                name: "Idea Cat".to_string(),
                email: author.to_string(),
                category: "idea".to_string(),
                message: "Treat counter".to_string(),
                submitted_at: 1_700_000_100,
                user_id: Some(author.to_string()),
                author_username: "Idea Cat".to_string(),
            })
            .expect("save feedback");

        let up = storage
            .cast_feedback_vote(post_id, author, 1)
            .expect("author upvote");
        assert_eq!(up.upvotes, 1);
        assert_eq!(up.user_vote, Some(1));

        let down = storage
            .cast_feedback_vote(post_id, author, -1)
            .expect("author switch to downvote");
        assert_eq!(down.upvotes, 0);
        assert_eq!(down.downvotes, 1);
        assert_eq!(down.user_vote, Some(-1));
    }

    #[test]
    fn friend_requests_and_pet_shares_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let owner = User {
            username: "owner".to_string(),
            first_name: "Cat".to_string(),
            last_name: "Owner".to_string(),
            email: "owner@example.com".to_string(),
            password: "secret1!".to_string(),
            created_at: 1_700_000_000,
        };
        let friend = User {
            username: "friend".to_string(),
            first_name: "Cat".to_string(),
            last_name: "Friend".to_string(),
            email: "friend@example.com".to_string(),
            password: "secret2!".to_string(),
            created_at: 1_700_000_001,
        };
        storage.save_user(&owner).expect("save owner");
        storage.save_user(&friend).expect("save friend");

        storage
            .create_friend_request("owner@example.com", "friend@example.com", 1)
            .expect("create friend request");
        let outgoing = storage
            .list_outgoing_friend_requests("owner@example.com")
            .expect("outgoing");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to_email, "friend@example.com");
        let incoming = storage
            .list_incoming_friend_requests("friend@example.com")
            .expect("incoming");
        assert_eq!(incoming.len(), 1);
        assert_eq!(
            storage
                .email_for_username("friend")
                .expect("lookup username")
                .as_deref(),
            Some("friend@example.com")
        );
        storage
            .respond_friend_request(&incoming[0].id, "friend@example.com", true)
            .expect("accept friend");
        assert!(storage
            .are_friends("owner@example.com", "friend@example.com")
            .expect("friends"));
        let friends = storage.list_friends("owner@example.com").expect("friends");
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].friend_email, "friend@example.com");

        let whisker_hits = storage
            .search_users_by_username("fri", 10)
            .expect("search whisker");
        assert_eq!(whisker_hits.len(), 1);
        assert_eq!(whisker_hits[0].username, "friend");
        assert_eq!(whisker_hits[0].email, "friend@example.com");

        storage
            .create_pet_share("owner@example.com", "friend@example.com", "primary", 2)
            .expect("share pet");
        let share_invites = storage
            .list_incoming_pet_shares("friend@example.com")
            .expect("share invites");
        assert_eq!(share_invites.len(), 1);
        storage
            .respond_pet_share(&share_invites[0].id, "friend@example.com", true)
            .expect("accept share");
        assert!(storage
            .has_accepted_pet_share("owner@example.com", "friend@example.com", "primary")
            .expect("accepted share"));

        let outgoing = storage
            .list_outgoing_pet_shares("owner@example.com")
            .expect("outgoing shares");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].status, "accepted");

        storage
            .revoke_pet_share(&outgoing[0].id, "owner@example.com")
            .expect("revoke share");
        assert!(!storage
            .has_accepted_pet_share("owner@example.com", "friend@example.com", "primary")
            .expect("share removed"));
    }

    #[test]
    fn feedback_delete_respects_ownership() {
        let storage = Storage::open_at(std::env::temp_dir().join(format!(
            "ww-feedback-delete-{}",
            uuid::Uuid::new_v4()
        )))
        .expect("storage");

        let submission = FeedbackSubmission {
            id: 0,
            name: "Owner".to_string(),
            email: "owner@test.local".to_string(),
            category: "idea".to_string(),
            message: "More treats".to_string(),
            submitted_at: 1,
            user_id: Some("owner@test.local".to_string()),
            author_username: "owner".to_string(),
        };
        let feedback_id = storage.save_feedback(&submission).expect("save feedback");

        assert_eq!(
            storage
                .delete_feedback_owned(feedback_id, "other@test.local")
                .expect("delete as other"),
            ForumDeleteOutcome::NotAuthorized
        );
        assert_eq!(
            storage
                .delete_feedback_owned(feedback_id, "owner@test.local")
                .expect("delete as owner"),
            ForumDeleteOutcome::Deleted
        );
        assert!(storage
            .get_feedback_submission(feedback_id)
            .expect("lookup")
            .is_none());
    }

    #[test]
    fn feedback_comments_support_nested_replies_and_delete() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let feedback_id = storage
            .save_feedback(&FeedbackSubmission {
                id: 0,
                name: "Owner".to_string(),
                email: "owner@test.local".to_string(),
                category: "idea".to_string(),
                message: "More treats".to_string(),
                submitted_at: 1,
                user_id: Some("owner@test.local".to_string()),
                author_username: "owner".to_string(),
            })
            .expect("save feedback");

        let top_id = storage
            .create_feedback_comment(
                feedback_id,
                None,
                "owner@test.local",
                "owner",
                "Top level",
                2,
            )
            .expect("top comment");
        let reply_id = storage
            .create_feedback_comment(
                feedback_id,
                Some(top_id),
                "friend@test.local",
                "friend",
                "Nested reply",
                3,
            )
            .expect("reply");

        let comments = storage
            .list_feedback_comments(feedback_id)
            .expect("list comments");
        assert_eq!(comments.len(), 2);

        assert_eq!(
            storage
                .delete_feedback_comment_owned(reply_id, "owner@test.local")
                .expect("delete reply as non-owner"),
            ForumDeleteOutcome::NotAuthorized
        );
        assert_eq!(
            storage
                .delete_feedback_comment_owned(top_id, "owner@test.local")
                .expect("delete top comment"),
            ForumDeleteOutcome::Deleted
        );
        assert!(storage
            .list_feedback_comments(feedback_id)
            .expect("list after delete")
            .is_empty());
    }

    #[test]
    fn social_post_comment_delete_requires_owner() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let post = storage
            .create_social_post(
                "owner@test.local",
                "owner",
                "Hello",
                &[],
                false,
                100,
            )
            .expect("create post");
        let comment = storage
            .create_social_post_comment(
                &post.id,
                "owner@test.local",
                "owner",
                "Nice post",
                200,
            )
            .expect("create comment");
        storage
            .toggle_social_comment_upvote(&comment.id, "friend@test.local", 201)
            .expect("vote");

        assert_eq!(
            storage
                .delete_social_post_comment_owned(&comment.id, "friend@test.local")
                .expect("non-owner delete"),
            ForumDeleteOutcome::NotAuthorized
        );
        assert_eq!(
            storage
                .delete_social_post_comment_owned(&comment.id, "owner@test.local")
                .expect("owner delete"),
            ForumDeleteOutcome::Deleted
        );
        let refreshed = storage
            .get_social_post_by_id(&post.id, Some("owner@test.local"))
            .expect("load post")
            .expect("post exists");
        assert!(refreshed.comments.is_empty());
    }
}
