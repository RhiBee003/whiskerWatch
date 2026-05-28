use axum::{
    Form, Router,
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::{fs, io::AsyncWriteExt, net::TcpListener};
use tower_http::services::ServeDir;

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

#[derive(Serialize)]
struct ContactSubmission {
    name: String,
    email: String,
    subject: String,
    message: String,
    submitted_at: u64,
}

#[derive(Deserialize, Default)]
struct ContactQuery {
    status: Option<String>,
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

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
            let (success, error) = match query.status.as_deref() {
                Some("sent") => (
                    "Thanks! Your message was received. We will get back to you soon.",
                    "",
                ),
                Some("missing") => (
                    "",
                    "Please fill out all fields before sending your message.",
                ),
                Some("failed") => (
                    "",
                    "We could not save your message. Please try again in a moment.",
                ),
                _ => ("", ""),
            };
            let body = contents
                .replace("{{CONTACT_SUCCESS}}", success)
                .replace("{{CONTACT_ERROR}}", error);
            Html(body).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load contact page".to_string(),
        )
            .into_response(),
    }
}

async fn login_submit(Form(form): Form<LoginForm>) -> impl IntoResponse {
    let email = form.email.trim();
    let password = form.password.trim();

    if email.is_empty() || password.is_empty() {
        return Redirect::to("/login?error=missing");
    }

    if email.eq_ignore_ascii_case("demo@whiskerwatch.app") && password == "meow123" {
        return Redirect::to("/?login=success");
    }

    if user_login_valid(email, password).await {
        return Redirect::to("/?login=success");
    }

    if !email_exists(email).await {
        let encoded_email = encode_component(email);
        return Redirect::to(&format!("/signup?reason=notfound&email={encoded_email}"));
    }

    Redirect::to("/login?error=invalid")
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
    if email.eq_ignore_ascii_case("demo@whiskerwatch.app") {
        return true;
    }

    load_users()
        .await
        .into_iter()
        .any(|user| user.email.eq_ignore_ascii_case(email))
}

async fn save_user(form: &SignupForm) -> Result<(), std::io::Error> {
    fs::create_dir_all("data").await?;

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let user = User {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        password: form.password.trim().to_string(),
        created_at,
    };

    let line = serde_json::to_string(&user).expect("user should serialize");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/users.jsonl")
        .await?;
    file.write_all(format!("{line}\n").as_bytes()).await?;

    Ok(())
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

async fn save_contact_submission(form: &ContactForm) -> Result<(), std::io::Error> {
    fs::create_dir_all("data").await?;

    let submitted_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let submission = ContactSubmission {
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        subject: form.subject.trim().to_string(),
        message: form.message.trim().to_string(),
        submitted_at,
    };

    let line = serde_json::to_string(&submission).expect("contact submission should serialize");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/contact_messages.jsonl")
        .await?;
    file.write_all(format!("{line}\n").as_bytes()).await?;

    Ok(())
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

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index_page))
        .route("/login", get(login_page).post(login_submit))
        .route("/signup", get(signup_page).post(signup_submit))
        .route("/contact", get(contact_page).post(contact_submit))
        .route("/login.html", get(|| async { Redirect::permanent("/login") }))
        .route("/signup.html", get(|| async { Redirect::permanent("/signup") }))
        .route("/contact.html", get(|| async { Redirect::permanent("/contact") }))
        .nest_service("/images", ServeDir::new("static/images"))
        .fallback_service(ServeDir::new("static"));

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");

    println!("Whisker Watch Web running at http://127.0.0.1:3000");
    axum::serve(listener, app)
        .await
        .expect("server failed unexpectedly");
}
