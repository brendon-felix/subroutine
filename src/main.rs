use anyhow::Result;

mod app;
mod auth;
// mod command_palette;
// mod conveyor_list;
// mod list_rewrite;
mod components;
mod stores;
mod tasks;
mod views;

#[tokio::main]
async fn main() -> Result<()> {
    app::run()
}
