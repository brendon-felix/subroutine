use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod app;
mod assets;
mod components;
mod icons;
mod stores;
mod themes;
mod utils;
mod views;

pub use icons::AppIcon;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    app::run()
}
