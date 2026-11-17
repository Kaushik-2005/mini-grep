mod args;
mod config;

use clap::{CommandFactory, Parser};

use crate::AppError;

pub(crate) use config::{PathListSource, PatternSource};

pub(crate) fn parse() -> Result<config::Config, AppError> {
    let args = args::Args::try_parse().map_err(|error| AppError::Cli(Box::new(error)))?;

    config::Config::try_from(args).map_err(|error| match error {
        AppError::Usage(message) => AppError::Cli(Box::new(
            args::Args::command().error(clap::error::ErrorKind::ValueValidation, message),
        )),
        other => other,
    })
}
