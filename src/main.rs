#![allow(clippy::manual_async_fn)]
#![allow(dead_code)]

use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::Parser;

use crate::application::{Application, Context as AppContext, component::RichContext};
use crate::backend::bridge::BackendBridge;

mod application;
mod backend;

#[cfg(feature = "mock")]
use crate::backend::mock::MockBackend;

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
    let file = std::io::LineWriter::new(file);
    let file = Box::new(file);
    let env = env_logger::Env::new()
        .filter_or("PATINA", "info")
        .write_style_or("PATINA_STYLE", "never");

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

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("tokio-worker")
        .enable_all()
        .build()
        .context("Failed to build tokio runtime")?;
    let _guard = rt.enter();
    log::info!(
        "Main thread id={:?} name={:?}",
        std::thread::current().id(),
        std::thread::current().name()
    );

    let context = AppContext {
        log_file,
        fps: 144,
        connection_maxitem: 5,
    };

    #[cfg(feature = "mock")]
    {
        let backend = MockBackend::new();
        
        // Create message channel for UI
        let (message_tx, message_rx) = std::sync::mpsc::channel();
        let message_tx = std::sync::Arc::new(message_tx);
        
        // Create rich context
        let rich_context = Arc::new(RichContext {
            log_file: context.log_file,
            fps: context.fps,
            message: message_tx.clone(),
            connection_maxitem: context.connection_maxitem,
        });
        
        // Create backend bridge
        let bridge = Arc::new(BackendBridge::new(backend, message_tx.clone()));
        
        // Create and run application
        let application = Application::new(rich_context, bridge, message_tx, message_rx);
        application.run()?;
    }

    #[cfg(not(feature = "mock"))]
    compile_error!("Real backend not yet implemented. Build with --features mock.");

    Ok(())
}
