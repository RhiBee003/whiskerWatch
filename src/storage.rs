use crate::{ContactSubmission, FeedbackSubmission, User, UserProfile};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
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

#[derive(Clone)]
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
    data_dir: PathBuf,
}

pub fn data_dir_from_env() -> PathBuf {
    std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"))
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
                 name TEXT NOT NULL,
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
             );",
        )?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        };
        storage.migrate_from_jsonl()?;
        Ok(storage)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn load_users(&self) -> Result<Vec<User>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT name, email, password, created_at FROM users ORDER BY created_at ASC",
        )?;
        let users = stmt
            .query_map([], |row| {
                Ok(User {
                    name: row.get(0)?,
                    email: row.get(1)?,
                    password: row.get(2)?,
                    created_at: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(users)
    }

    pub fn save_user(&self, user: &User) -> Result<(), StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        conn.execute(
            "INSERT INTO users (email, name, password, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![user.email, user.name, user.password, user.created_at as i64],
        )?;
        Ok(())
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

    pub fn validate_login(&self, email: &str, password: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE email = ?1 COLLATE NOCASE AND password = ?2",
            params![email, password],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn find_user_by_email(&self, email: &str) -> Result<Option<User>, StorageError> {
        let conn = self.conn.lock().expect("storage lock");
        let mut stmt = conn.prepare(
            "SELECT name, email, password, created_at FROM users WHERE email = ?1 COLLATE NOCASE",
        )?;
        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                name: row.get(0)?,
                email: row.get(1)?,
                password: row.get(2)?,
                created_at: row.get::<_, i64>(3)? as u64,
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

    fn migrate_from_jsonl(&self) -> Result<(), StorageError> {
        let users_path = self.data_dir.join("users.jsonl");
        if self.load_users()?.is_empty() && users_path.exists() {
            let contents = std::fs::read_to_string(&users_path)?;
            for line in contents.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(user) = serde_json::from_str::<User>(line) {
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
