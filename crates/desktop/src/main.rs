use anyhow::Result;

mod app;
mod assets;
// mod auth;
mod components;
mod stores;
// mod tasks;
mod themes;
mod transitions;
mod views;

fn main() -> Result<()> {
    app::run()
}
