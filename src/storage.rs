use crate::{ContactSubmission, FeedbackSubmission, User, UserProfile};
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
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "database error: {error}"),
            Self::Json(error) => write!(f, "json error: {error}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::PasswordHash(error) => write!(f, "password hash error: {error}"),
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

fn default_data_dir() -> PathBuf {
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
            return root.join("data");
        }
    }

    PathBuf::from("data")
}

fn resolve_data_dir(dir: PathBuf) -> PathBuf {
    if dir.is_absolute() {
        dir
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(dir)
    }
}

pub fn data_dir_from_env() -> PathBuf {
    let dir = std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_data_dir());
    resolve_data_dir(dir)
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
             CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);
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
             );",
        )?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        };
        storage.migrate_user_columns()?;
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
             CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);",
        )?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        };
        storage.migrate_user_columns()?;
        Ok(storage)
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
        }

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
        let stored_password = if is_bcrypt_hash(&user.password) {
            user.password.clone()
        } else {
            hash_password(&user.password)?
        };
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
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
        )?;
        Ok(())
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

    fn update_password_hash(&self, email: &str, password_hash: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "UPDATE users SET password = ?1 WHERE email = ?2 COLLATE NOCASE",
            params![password_hash, email],
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
            "INSERT INTO feedback (name, email, category, message, submitted_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                submission.name,
                submission.email,
                submission.category,
                submission.message,
                submission.submitted_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn load_feedback(&self) -> Result<Vec<FeedbackSubmission>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT name, email, category, message, submitted_at
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
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(feedback)
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
    fn default_data_dir_finds_project_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let nested = manifest_dir.join("target").join("storage-test-nested");
        fs::create_dir_all(&nested).expect("create nested dir");

        let original = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&nested).expect("chdir nested");

        let data_dir = default_data_dir();
        assert_eq!(data_dir, manifest_dir.join("data"));

        std::env::set_current_dir(original).expect("restore cwd");
        let _ = fs::remove_dir(nested);
    }
}
