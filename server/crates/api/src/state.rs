use sqlx::PgPool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub pdf_dir: PathBuf,
    #[allow(dead_code)]
    pub tei_dir: PathBuf,
}
