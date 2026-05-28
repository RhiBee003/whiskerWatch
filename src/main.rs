use axum::Router;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let app = Router::new().fallback_service(ServeDir::new("static"));
    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");

    println!("Whisker Watch Web running at http://127.0.0.1:3000");
    axum::serve(listener, app)
        .await
        .expect("server failed unexpectedly");
}
