use crate::{ContactSubmission, FeedbackSubmission, ForumPost, ForumReply, User, UserProfile};
use rusqlite::{params, Connection};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
    PasswordHash(bcrypt::BcryptError),
    EmailTaken,
    UsernameTaken,
    InvalidResetToken,
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
    password.starts_with("$2a$")
        || password.starts_with("$2b$")
        || password.starts_with("$2y$")
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
                 submitted_at INTEGER NOT NULL
             );
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
        storage.migrate_forum_tables()?;
        storage.migrate_submission_tables()?;
        storage.migrate_from_jsonl()?;
        Ok(storage)
    }

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
        storage.migrate_forum_tables()?;
        storage.migrate_submission_tables()?;
        Ok(storage)
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
                 user_id TEXT
             );",
        )?;
        if !Self::table_has_column(&conn, "feedback", "user_id")? {
            conn.execute("ALTER TABLE feedback ADD COLUMN user_id TEXT", [])?;
        }
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

    fn ensure_username_index(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);",
        )?;
        Ok(())
    }

    fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, StorageError> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(columns.iter().any(|name| name == column))
    }

    fn email_local_part(email: &str) -> String {
        email
            .split('@')
            .next()
            .unwrap_or(email)
            .trim()
            .to_string()
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
                let mut stmt = conn.prepare(
                    "SELECT email, name, username, first_name, last_name FROM users",
                )?;
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
                    let username =
                        Self::unique_username_from_base(&conn, &username_base)?;
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
        let mut stmt = conn.prepare(
            "SELECT profile_json FROM user_profiles WHERE email = ?1 COLLATE NOCASE",
        )?;
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

    pub fn save_feedback(&self, submission: &FeedbackSubmission) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO feedback (name, email, category, message, submitted_at, user_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                submission.name,
                submission.email,
                submission.category,
                submission.message,
                submission.submitted_at as i64,
                submission.user_id,
            ],
        )?;
        Ok(())
    }

    pub fn load_feedback(&self) -> Result<Vec<FeedbackSubmission>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT name, email, category, message, submitted_at, user_id
             FROM feedback ORDER BY submitted_at ASC",
        )?;
        let feedback = stmt
            .query_map([], |row| {
                Ok(FeedbackSubmission {
                    name: row.get(0)?,
                    email: row.get(1)?,
                    category: row.get(2)?,
                    message: row.get(3)?,
                    submitted_at: row.get::<_, i64>(4)? as u64,
                    user_id: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(feedback)
    }

    pub fn create_forum_post(
        &self,
        user_id: &str,
        author_username: &str,
        title: &str,
        body: &str,
        created_at: u64,
    ) -> Result<i64, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO forum_posts (user_id, author_username, title, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                user_id,
                author_username,
                title,
                body,
                created_at as i64,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_forum_posts(&self) -> Result<Vec<ForumPost>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, user_id, author_username, title, body, created_at
             FROM forum_posts ORDER BY created_at DESC",
        )?;
        let posts = stmt
            .query_map([], |row| {
                Ok(ForumPost {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    author_username: row.get(2)?,
                    title: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(posts)
    }

    pub fn get_forum_post(&self, post_id: i64) -> Result<Option<ForumPost>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT id, user_id, author_username, title, body, created_at
             FROM forum_posts WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![post_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ForumPost {
                id: row.get(0)?,
                user_id: row.get(1)?,
                author_username: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get::<_, i64>(5)? as u64,
            }))
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
            params![
                post_id,
                user_id,
                author_username,
                body,
                created_at as i64,
            ],
        )?;
        Ok(())
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
            eprintln!(
                "Migrated users from {} into SQLite",
                users_path.display()
            );
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
            for line in contents.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(submission) = serde_json::from_str::<FeedbackSubmission>(line) {
                    let _ = self.save_feedback(&submission);
                }
            }
        }

        Ok(())
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
        assert!(storage.validate_login(email, "SecretPass1!").expect("validate"));
        assert!(!storage.validate_login(email, "wrong").expect("validate wrong"));

        drop(storage);

        let storage = Storage::open_at(temp.path().to_path_buf()).expect("reopen storage");
        assert!(storage.user_exists(email).expect("exists"));
        assert!(storage.validate_login(email, "SecretPass1!").expect("validate after reopen"));
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
        assert!(storage.user_exists("new@test.local").expect("new user exists"));
        assert!(storage.validate_login("legacy@test.local", "plainpass").expect("legacy login"));
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

        assert!(storage.validate_login(email, "NewSecure1!").expect("new login"));
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

        let posts = storage.list_forum_posts().expect("list posts");
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].title, "How often should I brush?");

        let replies = storage.list_forum_replies(post_id).expect("list replies");
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].body, "Daily brushing helps!");

        assert_eq!(storage.count_forum_replies(post_id).expect("count"), 1);
    }

    #[test]
    fn feedback_save_and_load_round_trips_with_user_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(temp.path().to_path_buf()).expect("open storage");
        let submission = FeedbackSubmission {
            name: "Tester".to_string(),
            email: "tester@example.com".to_string(),
            category: "bug".to_string(),
            message: "Button does not click".to_string(),
            submitted_at: 1_700_000_100,
            user_id: Some("tester@example.com".to_string()),
        };

        storage.save_feedback(&submission).expect("save feedback");
        let loaded = storage.load_feedback().expect("load feedback");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].message, "Button does not click");
        assert_eq!(loaded[0].user_id.as_deref(), Some("tester@example.com"));
    }
}

