mod app;
mod cli;
mod discovery;
mod error;
mod input;
mod output;

pub mod search;

pub use app::{SearchOutcome, run};
pub use error::AppError;
