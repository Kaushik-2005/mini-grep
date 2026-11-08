mod args;
mod config;

use clap::Parser;

use crate::AppError;

pub(crate) fn parse() -> Result<config::Config, AppError> {
    let args = args::Args::try_parse().map_err(|error| AppError::Cli(Box::new(error)))?;

    config::Config::try_from(args)
}
