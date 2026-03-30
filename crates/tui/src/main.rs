mod app;
mod debug;
mod term;
mod ui;

use anyhow::{Result, anyhow};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    run().await?;
    Ok(())
}

async fn run() -> Result<()> {
    let mut app = app::App::new()?;
    app.run().await?;
    Ok(())
}
