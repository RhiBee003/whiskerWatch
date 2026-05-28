use axum::{
    Form, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{fs, io::AsyncWriteExt, net::TcpListener};
use tower_http::services::ServeDir;
use uuid::Uuid;

const ADMIN_SESSION_COOKIE: &str = "ww_admin_session";

#[derive(Clone)]
struct AppState {
    admin_sessions: Arc<Mutex<HashSet<String>>>,
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

#[derive(Deserialize, Default)]
struct LoginQuery {
    error: Option<String>,
    signup: Option<String>,
}

#[derive(Deserialize, Default)]
struct SignupQuery {
    error: Option<String>,
    reason: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize, Default)]
struct FeedbackQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct SignupForm {
    name: String,
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    name: String,
    email: String,
    password: String,
    created_at: u64,
}

#[derive(Deserialize)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ContactSubmission {
    name: String,
    email: String,
    subject: String,
    message: String,
    submitted_at: u64,
}

#[derive(Deserialize)]
struct FeedbackForm {
    name: String,
    email: String,
    category: String,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct FeedbackSubmission {
    name: String,
    email: String,
    category: String,
    message: String,
    submitted_at: u64,
}

#[derive(Deserialize, Default)]
struct ContactQuery {
    status: Option<String>,
}

fn admin_email() -> String {
    env::var("ADMIN_EMAIL").unwrap_or_else(|_| "rhibee003@gmail.com".to_string())
}

fn admin_password() -> String {
    env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "WhiskerAdmin2026!".to_string())
}

fn listen_address() -> String {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    format!("0.0.0.0:{port}")
}

fn encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html(value)
}

fn timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn format_timestamp(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Unknown".to_string();
    }

    let seconds_per_day = 86_400;
    let days = timestamp / seconds_per_day;
    let hours = (timestamp % seconds_per_day) / 3_600;
    let minutes = (timestamp % 3_600) / 60;
    format!("day {days} {hours:02}:{minutes:02} UTC")
}

fn is_admin_credentials(email: &str, password: &str) -> bool {
    email.eq_ignore_ascii_case(&admin_email()) && password == admin_password()
}

fn admin_session_valid(state: &AppState, jar: &CookieJar) -> bool {
    let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) else {
        return false;
    };

    state
        .admin_sessions
        .lock()
        .expect("admin session lock")
        .contains(cookie.value())
}

fn create_admin_session(state: &AppState, jar: CookieJar) -> CookieJar {
    let session_id = Uuid::new_v4().to_string();
    state
        .admin_sessions
        .lock()
        .expect("admin session lock")
        .insert(session_id.clone());

    let mut cookie = Cookie::new(ADMIN_SESSION_COOKIE, session_id);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    jar.add(cookie)
}

fn clear_admin_session(state: &AppState, jar: CookieJar) -> CookieJar {
    if let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) {
        state
            .admin_sessions
            .lock()
            .expect("admin session lock")
            .remove(cookie.value());
    }

    jar.remove(Cookie::from(ADMIN_SESSION_COOKIE))
}

async fn index_page() -> impl IntoResponse {
    match fs::read_to_string("static/index.html").await {
        Ok(contents) => Html(contents).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load homepage".to_string(),
        )
            .into_response(),
    }
}

async fn login_page(Query(query): Query<LoginQuery>) -> impl IntoResponse {
    match fs::read_to_string("templates/login.html").await {
        Ok(contents) => {
            let login_error_block = match query.error.as_deref() {
                Some("invalid") => {
                    r#"<p class="auth-error" role="alert">Incorrect password. Please try again.</p>"#
                }
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please enter both email and password.</p>"#
                }
                _ => "",
            };
            let signup_success_block = match query.signup.as_deref() {
                Some("created") => r#"<p class="auth-success" role="status">Account created! You can log in with your new email and password.</p>"#,
                _ => "",
            };
            let body = contents
                .replace("{{LOGIN_ERROR_BLOCK}}", login_error_block)
                .replace("{{SIGNUP_SUCCESS_BLOCK}}", signup_success_block);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load login page".to_string(),
        )
            .into_response(),
    }
}

async fn signup_page(Query(query): Query<SignupQuery>) -> impl IntoResponse {
    match fs::read_to_string("templates/signup.html").await {
        Ok(contents) => {
            let signup_error_block = match query.error.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please fill out all sign up fields.</p>"#
                }
                Some("exists") => {
                    r#"<p class="auth-error" role="alert">An account with that email already exists.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not create your account. Please try again.</p>"#
                }
                _ => "",
            };
            let signup_info_block = match query.reason.as_deref() {
                Some("notfound") => {
                    r#"<p class="auth-success" role="status">No account found with that email. Create one below.</p>"#
                }
                _ => "",
            };
            let signup_email = escape_html_attr(query.email.as_deref().unwrap_or(""));
            let body = contents
                .replace("{{SIGNUP_INFO_BLOCK}}", signup_info_block)
                .replace("{{SIGNUP_ERROR_BLOCK}}", signup_error_block)
                .replace("{{SIGNUP_EMAIL}}", &signup_email);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load sign up page".to_string(),
        )
            .into_response(),
    }
}

async fn contact_page(Query(query): Query<ContactQuery>) -> impl IntoResponse {
    match fs::read_to_string("templates/contact.html").await {
        Ok(contents) => {
            let contact_success_block = match query.status.as_deref() {
                Some("sent") => {
                    r#"<p class="auth-success" role="status">Thanks! Your message was received. We will get back to you soon.</p>"#
                }
                _ => "",
            };
            let contact_error_block = match query.status.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please fill out all fields before sending your message.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not save your message. Please try again in a moment.</p>"#
                }
                _ => "",
            };
            let body = contents
                .replace("{{CONTACT_SUCCESS_BLOCK}}", contact_success_block)
                .replace("{{CONTACT_ERROR_BLOCK}}", contact_error_block);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load contact page".to_string(),
        )
            .into_response(),
    }
}

async fn feedback_page(Query(query): Query<FeedbackQuery>) -> impl IntoResponse {
    match fs::read_to_string("templates/feedback.html").await {
        Ok(contents) => {
            let feedback_success_block = match query.status.as_deref() {
                Some("sent") => {
                    r#"<p class="auth-success" role="status">Thanks! Your feedback was sent to the WhiskerWatch team.</p>"#
                }
                _ => "",
            };
            let feedback_error_block = match query.status.as_deref() {
                Some("missing") => {
                    r#"<p class="auth-error" role="alert">Please fill out all feedback fields.</p>"#
                }
                Some("failed") => {
                    r#"<p class="auth-error" role="alert">We could not save your feedback. Please try again.</p>"#
                }
                _ => "",
            };
            let body = contents
                .replace("{{FEEDBACK_SUCCESS_BLOCK}}", feedback_success_block)
                .replace("{{FEEDBACK_ERROR_BLOCK}}", feedback_error_block);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load feedback page".to_string(),
        )
            .into_response(),
    }
}

async fn login_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    let email = form.email.trim();
    let password = form.password.trim();

    if email.is_empty() || password.is_empty() {
        return Redirect::to("/login?error=missing").into_response();
    }

    if is_admin_credentials(email, password) {
        let jar = create_admin_session(&state, jar);
        return (jar, Redirect::to("/admin")).into_response();
    }

    if email.eq_ignore_ascii_case("demo@whiskerwatch.app") && password == "meow123" {
        return Redirect::to("/?login=success").into_response();
    }

    if user_login_valid(email, password).await {
        return Redirect::to("/?login=success").into_response();
    }

    if !email_exists(email).await {
        let encoded_email = encode_component(email);
        return Redirect::to(&format!("/signup?reason=notfound&email={encoded_email}")).into_response();
    }

    Redirect::to("/login?error=invalid").into_response()
}

async fn load_users() -> Vec<User> {
    let contents = match fs::read_to_string("data/users.jsonl").await {
        Ok(contents) => contents,
        Err(_) => return Vec::new(),
    };

    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<User>(line).ok())
        .collect()
}

async fn user_login_valid(email: &str, password: &str) -> bool {
    load_users()
        .await
        .into_iter()
        .any(|user| user.email.eq_ignore_ascii_case(email) && user.password == password)
}

async fn email_exists(email: &str) -> bool {
    if email.eq_ignore_ascii_case("demo@whiskerwatch.app")
        || email.eq_ignore_ascii_case(&admin_email())
    {
        return true;
    }

    load_users()
        .await
        .into_iter()
        .any(|user| user.email.eq_ignore_ascii_case(email))
}

async fn save_user(form: &SignupForm) -> Result<(), std::io::Error> {
    fs::create_dir_all("data").await?;

    let user = User {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        password: form.password.trim().to_string(),
        created_at: timestamp_now(),
    };

    append_json_line("data/users.jsonl", &user).await
}

async fn signup_submit(Form(form): Form<SignupForm>) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let password = form.password.trim();

    if name.is_empty() || email.is_empty() || password.is_empty() {
        return Redirect::to("/signup?error=missing");
    }

    if email_exists(email).await {
        return Redirect::to("/signup?error=exists");
    }

    match save_user(&form).await {
        Ok(()) => Redirect::to("/login?signup=created"),
        Err(_) => Redirect::to("/signup?error=failed"),
    }
}

async fn append_json_line<T: Serialize>(path: &str, value: &T) -> Result<(), std::io::Error> {
    fs::create_dir_all("data").await?;
    let line = serde_json::to_string(value).expect("value should serialize");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(format!("{line}\n").as_bytes()).await?;
    Ok(())
}

async fn save_contact_submission(form: &ContactForm) -> Result<(), std::io::Error> {
    let submission = ContactSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        subject: form.subject.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
    };

    append_json_line("data/contact_messages.jsonl", &submission).await
}

async fn save_feedback_submission(form: &FeedbackForm) -> Result<(), std::io::Error> {
    let submission = FeedbackSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        category: form.category.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at: timestamp_now(),
    };

    append_json_line("data/feedback.jsonl", &submission).await
}

async fn contact_submit(Form(form): Form<ContactForm>) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let subject = form.subject.trim();
    let message = form.message.trim();

    if name.is_empty() || email.is_empty() || subject.is_empty() || message.is_empty() {
        return Redirect::to("/contact?status=missing");
    }

    match save_contact_submission(&form).await {
        Ok(()) => Redirect::to("/contact?status=sent"),
        Err(_) => Redirect::to("/contact?status=failed"),
    }
}

async fn feedback_submit(Form(form): Form<FeedbackForm>) -> impl IntoResponse {
    let name = form.name.trim();
    let email = form.email.trim();
    let category = form.category.trim();
    let message = form.message.trim();

    if name.is_empty() || email.is_empty() || category.is_empty() || message.is_empty() {
        return Redirect::to("/feedback?status=missing");
    }

    if !matches!(category, "fix" | "idea" | "bug") {
        return Redirect::to("/feedback?status=missing");
    }

    match save_feedback_submission(&form).await {
        Ok(()) => Redirect::to("/feedback?status=sent"),
        Err(_) => Redirect::to("/feedback?status=failed"),
    }
}

async fn load_json_lines<T: for<'de> Deserialize<'de>>(path: &str) -> Vec<T> {
    let contents = match fs::read_to_string(path).await {
        Ok(contents) => contents,
        Err(_) => return Vec::new(),
    };

    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<T>(line).ok())
        .collect()
}

fn render_submission_rows(
    rows: &[(&str, &str, &str, &str, u64)],
    empty_message: &str,
) -> String {
    if rows.is_empty() {
        return format!(r#"<tr><td colspan="5">{empty_message}</td></tr>"#);
    }

    rows.iter()
        .rev()
        .map(|(kind, name, email, message, submitted_at)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(kind),
                escape_html(name),
                escape_html(email),
                escape_html(message),
                escape_html(&format_timestamp(*submitted_at)),
            )
        })
        .collect()
}

async fn admin_page(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if !admin_session_valid(&state, &jar) {
        return Redirect::to("/login").into_response();
    }

    let feedback = load_json_lines::<FeedbackSubmission>("data/feedback.jsonl").await;
    let contacts = load_json_lines::<ContactSubmission>("data/contact_messages.jsonl").await;

    let feedback_rows: Vec<(&str, &str, &str, &str, u64)> = feedback
        .iter()
        .map(|item| {
            (
                item.category.as_str(),
                item.name.as_str(),
                item.email.as_str(),
                item.message.as_str(),
                item.submitted_at,
            )
        })
        .collect();

    let contact_rows: Vec<(&str, &str, &str, &str, u64)> = contacts
        .iter()
        .map(|item| {
            (
                item.subject.as_str(),
                item.name.as_str(),
                item.email.as_str(),
                item.message.as_str(),
                item.submitted_at,
            )
        })
        .collect();

    let body = format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>WhiskerWatch Admin</title>
    <link rel="stylesheet" href="/styles.css" />
  </head>
  <body>
    <header class="topbar">
      <div class="brand" aria-label="WhiskerWatch">
        <img class="brand-logo" src="/images/logo.png" alt="WhiskerWatch" />
      </div>
      <nav>
        <a href="/">HOME</a>
        <a href="/feedback">FEEDBACK</a>
        <form class="admin-logout-form" action="/admin/logout" method="post">
          <button type="submit" class="admin-logout-btn">LOG OUT</button>
        </form>
      </nav>
    </header>
    <main class="section admin-page">
      <h1>Admin Dashboard</h1>
      <p>Review feedback, bug reports, and contact messages from testers.</p>

      <section class="admin-panel">
        <h2>Feedback and Ideas ({feedback_count})</h2>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Type</th>
              <th>Name</th>
              <th>Email</th>
              <th>Message</th>
              <th>Submitted</th>
            </tr>
          </thead>
          <tbody>
            {feedback_rows}
          </tbody>
        </table>
      </section>

      <section class="admin-panel">
        <h2>Contact Messages ({contact_count})</h2>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Subject</th>
              <th>Name</th>
              <th>Email</th>
              <th>Message</th>
              <th>Submitted</th>
            </tr>
          </thead>
          <tbody>
            {contact_rows}
          </tbody>
        </table>
      </section>
    </main>
  </body>
</html>"#,
        feedback_count = feedback.len(),
        contact_count = contacts.len(),
        feedback_rows = render_submission_rows(
            &feedback_rows,
            "No feedback submissions yet."
        ),
        contact_rows = render_submission_rows(
            &contact_rows,
            "No contact messages yet."
        ),
    );

    Html(body).into_response()
}

async fn admin_logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = clear_admin_session(&state, jar);
    (jar, Redirect::to("/login")).into_response()
}

#[tokio::main]
async fn main() {
    let state = AppState {
        admin_sessions: Arc::new(Mutex::new(HashSet::new())),
    };

    let app = Router::new()
        .route("/", get(index_page))
        .route("/login", get(login_page).post(login_submit))
        .route("/signup", get(signup_page).post(signup_submit))
        .route("/contact", get(contact_page).post(contact_submit))
        .route("/feedback", get(feedback_page).post(feedback_submit))
        .route("/admin", get(admin_page))
        .route("/admin/logout", post(admin_logout))
        .route("/login.html", get(|| async { Redirect::permanent("/login") }))
        .route("/signup.html", get(|| async { Redirect::permanent("/signup") }))
        .route("/contact.html", get(|| async { Redirect::permanent("/contact") }))
        .route("/feedback.html", get(|| async { Redirect::permanent("/feedback") }))
        .nest_service("/images", ServeDir::new("static/images"))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let address = listen_address();
    let listener = TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind to {address}: {error}"));

    println!("WhiskerWatch running at http://{address}");
    println!("Admin login: {} / (see ADMIN_PASSWORD env var)", admin_email());
    axum::serve(listener, app)
        .await
        .expect("server failed unexpectedly");
}
