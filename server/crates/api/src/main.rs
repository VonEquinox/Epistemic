mod auth;
mod error;
mod routes;
mod state;

use axum::http::{header, Method};
use axum::routing::get;
use axum::Router;
use epistemic_core::repo::users;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("epistemic=debug".parse()?))
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://epistemic:epistemic@localhost:5432/epistemic".into());
    let host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("API_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let pdf_dir = PathBuf::from(std::env::var("PDF_DIR").unwrap_or_else(|_| "./data/pdfs".into()));
    let tei_dir = PathBuf::from(std::env::var("TEI_DIR").unwrap_or_else(|_| "./data/tei".into()));
    tokio::fs::create_dir_all(&pdf_dir).await?;
    tokio::fs::create_dir_all(&tei_dir).await?;

    tracing::info!("connecting to database…");
    let pool = epistemic_core::connect(&database_url).await?;

    // Bootstrap first admin if empty
    if let Ok(email) = std::env::var("BOOTSTRAP_ADMIN_EMAIL") {
        let name = std::env::var("BOOTSTRAP_ADMIN_NAME").unwrap_or_else(|_| "Admin".into());
        let password =
            std::env::var("BOOTSTRAP_ADMIN_PASSWORD").unwrap_or_else(|_| "changeme123".into());
        match users::bootstrap_admin(&pool, &email, &name, &password).await {
            Ok(Some(u)) => tracing::info!(email = %u.email, "bootstrap admin created"),
            Ok(None) => tracing::debug!("users already exist, skip bootstrap"),
            Err(e) => tracing::warn!(error = %e, "bootstrap admin failed"),
        }
    }

    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(std::env::var("SESSION_SECURE").ok().as_deref() == Some("true"))
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(14)));

    let state = AppState {
        pool,
        pdf_dir,
        tei_dir,
    };

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            // Dev: allow localhost Vite
            let s = origin.as_bytes();
            s.starts_with(b"http://localhost:") || s.starts_with(b"http://127.0.0.1:")
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::COOKIE])
        .allow_credentials(true);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/api/v1", routes::router())
        .layer(session_layer)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    tracing::info!(%addr, "epistemic-api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
