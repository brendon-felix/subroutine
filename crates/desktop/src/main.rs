use anyhow::Result;

mod app;
mod assets;
// mod auth;
mod components;
mod stores;
// mod tasks;
mod icons;
mod themes;
mod transitions;
mod utils;
mod views;

fn main() -> Result<()> {
    app::run()
}
