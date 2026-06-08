use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Clone, Debug)]
pub struct OutboundEmail {
    pub to: String,
    pub subject: String,
    pub html_body: String,
    pub text_body: String,
}

pub fn smtp_configured() -> bool {
    std::env::var("SMTP_HOST")
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn smtp_port() -> u16 {
    std::env::var("SMTP_PORT")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(587)
}

fn smtp_from_address() -> String {
    std::env::var("SMTP_FROM")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| crate::company_email())
}

fn smtp_from_name() -> String {
    std::env::var("SMTP_FROM_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "WhiskerWatch".to_string())
}

pub async fn send_email(email: &OutboundEmail) -> Result<(), String> {
    if !smtp_configured() {
        eprintln!("Email (dev log) to {} — {}", email.to, email.subject);
        eprintln!("{}", email.text_body);
        return Ok(());
    }

    let host = std::env::var("SMTP_HOST").map_err(|_| "SMTP_HOST missing")?;
    let from = format!("{} <{}>", smtp_from_name(), smtp_from_address());
    let mail = Message::builder()
        .from(
            from.parse()
                .map_err(|e| format!("invalid from address: {e}"))?,
        )
        .to(email
            .to
            .parse()
            .map_err(|e| format!("invalid recipient: {e}"))?)
        .subject(email.subject.clone())
        .header(ContentType::TEXT_HTML)
        .body(email.html_body.clone())
        .map_err(|e| format!("build email: {e}"))?;

    let mut mailer_builder = AsyncSmtpTransport::<Tokio1Executor>::relay(host.trim())
        .map_err(|e| format!("smtp relay: {e}"))?
        .port(smtp_port());

    if let (Ok(user), Ok(password)) = (std::env::var("SMTP_USER"), std::env::var("SMTP_PASSWORD")) {
        if !user.trim().is_empty() && !password.trim().is_empty() {
            mailer_builder =
                mailer_builder.credentials(Credentials::new(user.trim().to_string(), password));
        }
    }

    mailer_builder
        .build()
        .send(mail)
        .await
        .map_err(|e| format!("smtp send: {e}"))?;

    Ok(())
}
