mod app;
mod debug;
mod term;
mod ui;

use anyhow::Result;
use std::sync::Mutex;

// Global log file — written to by the log!() macro below.
static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Write a line to /tmp/tui.log. Use this instead of eprintln! everywhere
/// so output doesn't corrupt the terminal UI.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        use std::io::Write;
        if let Ok(mut guard) = $crate::LOG.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(f, $($arg)*);
            }
        }
    }};
}

#[tokio::main]
async fn main() -> Result<()> {
    // Open log file; truncate on each run so it stays readable.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("/tmp/tui.log")
        .expect("could not open /tmp/tui.log");
    *LOG.lock().unwrap() = Some(file);
    log!("--- tui started ---");
    run().await?;
    Ok(())
}

async fn run() -> Result<()> {
    let mut app = app::App::new()?;
    app.run().await?;
    Ok(())
}
