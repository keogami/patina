use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use crate::application::{Application, Context as AppContext};

mod application;

#[cfg(test)]
mod tests;

#[derive(clap::Parser)]
struct Args {
    #[arg(short = 'l', long = "log-file")]
    log_file: Option<PathBuf>,
}

fn setup_logging(log_file: &PathBuf) -> anyhow::Result<()> {
    if let Some(parent) = log_file.parent() {
        std::fs::create_dir_all(parent).context("Couldn't create parent dirs for the log file")?;
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .context("Can't create log file")?;
    let file = Box::new(file);
    let env = env_logger::Env::new()
        .filter("PATINA")
        .write_style("PATINA_STYLE");

    env_logger::Builder::from_env(env)
        .target(env_logger::Target::Pipe(file))
        .init();

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // setup logging as early as possible
    let log_file = args
        .log_file
        .unwrap_or_else(|| match dirs::config_local_dir() {
            Some(path) => path.join("patina").join("patina.log"),
            None => "patina.log".into(),
        });

    setup_logging(&log_file)?;

    let context = AppContext {
        log_file,
        fps: 144,
        connection_maxitem: 5,
    };

    let application = Application::new(context);

    application.run()?;

    Ok(())
}
