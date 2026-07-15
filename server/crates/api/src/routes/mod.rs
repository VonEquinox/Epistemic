pub mod auth;
pub mod works;
pub mod projects;
pub mod imports;
pub mod relations;
pub mod collab;
pub mod graph;
pub mod pdf;
pub mod evidence;
pub mod claims;
pub mod views;
pub mod jobs_admin;

use axum::routing::get;
use axum::Router;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/works", works::router())
        .nest("/projects", projects::router())
        .nest("/imports", imports::router())
        .nest("/relations", relations::router())
        .nest("/evidence", evidence::router())
        .nest("/claims", claims::router())
        .merge(collab::router())
        .nest("/graph", graph::router())
        .nest("/versions", pdf::router())
        .nest("/views", views::router())
        .nest("/jobs", jobs_admin::router())
        // convenience: GET /works/{id}/evidence and /works/{id}/claims-full
        .route("/works/{id}/evidence", get(evidence::list_for_work))
        .route("/works/{id}/claims-full", get(claims::list_for_work))
}
