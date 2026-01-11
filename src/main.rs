use anyhow::Result;

mod app;
mod auth;
mod components;
mod stores;
mod tasks;
mod transitions;
mod views;

#[tokio::main]
async fn main() -> Result<()> {
    app::run()
}
