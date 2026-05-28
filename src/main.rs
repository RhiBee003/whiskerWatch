use axum::{Router, response::Html, routing::get};
use tokio::net::TcpListener;

async fn home() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>Whisker Watch Web</title>
            </head>
            <body style="font-family: sans-serif; padding: 2rem;">
                <h1>Whisker Watch Web</h1>
                <p>Your Rust website is running.</p>
            </body>
        </html>
        "#,
    )
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(home));
    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");

    println!("Whisker Watch Web running at http://127.0.0.1:3000");
    axum::serve(listener, app)
        .await
        .expect("server failed unexpectedly");
}
