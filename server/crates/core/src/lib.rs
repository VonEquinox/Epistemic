//! Epistemic core: domain types, DB access, repository functions.

pub mod db;
pub mod domain;
pub mod error;
pub mod repo;
pub mod util;

pub use db::{connect, connect_no_migrate};
pub use error::{AppError, AppResult};
