use anyhow::Result;

mod app;
mod auth;
mod stores;
mod tasks;
mod views;

#[tokio::main]
async fn main() -> Result<()> {
    app::Subroutine::run()
}
