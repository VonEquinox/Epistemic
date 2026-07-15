pub mod auth;
pub mod works;
pub mod projects;
pub mod imports;
pub mod relations;
pub mod collab;
pub mod graph;
pub mod pdf;

use axum::Router;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/works", works::router())
        .nest("/projects", projects::router())
        .nest("/imports", imports::router())
        .nest("/relations", relations::router())
        .merge(collab::router())
        .nest("/graph", graph::router())
        .nest("/versions", pdf::router())
}
