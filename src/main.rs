use anyhow::Result;
use tokio::sync::watch;

mod app;
mod config;
mod speedtest;
mod ui;

use app::AppState;
use speedtest::run_download_test;
use ui::run_tui;

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = watch::channel(AppState::default());

    tokio::spawn(async move {
        if let Err(err) = run_download_test(tx.clone()).await {
            let mut state = AppState::default();
            state.running = false;
            state.done = true;
            state.error = Some(err.to_string());

            let _ = tx.send(state);
        }
    });

    run_tui(rx).await
}