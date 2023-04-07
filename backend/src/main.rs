use std::net::SocketAddr;

use axum::Router;
use tower_http::{trace::TraceLayer, services::{ServeDir, ServeFile}};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};


#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_static_file_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new().nest_service("/", ServeDir::new("routes").fallback(ServeFile::new("routes/not_found.html")));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.layer(TraceLayer::new_for_http()).into_make_service())
        .await
        .unwrap();
}
