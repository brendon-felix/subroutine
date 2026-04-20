# SQLite + PostgreSQL Sync & Ratatui TUI — Quick Reference

---

## Part 1: SQLite ↔ PostgreSQL Sync

---

### Architecture

- App reads/writes **SQLite only**. A background loop syncs to/from Postgres.
- Sync loop reconnects every tick — transient failures self-heal automatically.
- Conflict resolution: **last `updated_at` wins** (last-write-wins). Sufficient for single-user, multi-device. For multi-user concurrent edits, look at CRDTs (e.g. `automerge`, `yrs`).

---

### Primary Keys: UUID v4 as TEXT

`INTEGER AUTOINCREMENT` breaks multi-client sync — two offline clients generating `id=42` collide. UUID v4 has 122 bits of entropy; any client generates IDs independently.

Use `TEXT` in both databases (not Postgres's native `UUID` type) to keep schemas identical and avoid type-mapping friction.

```rust
let id = Uuid::new_v4().to_string(); // "550e8400-e29b-41d4-a716-446655440000"
```

---

### Type Mapping

| Rust | SQLite | PostgreSQL | Notes |
|---|---|---|---|
| `String` | `TEXT` | `TEXT` | |
| `i64` / `i32` | `INTEGER` | `INTEGER` | SQLite stores as i64; PG `INTEGER` is i32 — cast on read |
| `bool` | `INTEGER` (0/1) | `INTEGER` (0/1) | PG has `BOOLEAN` but INTEGER keeps schemas identical |
| `DateTime<Utc>` | `TEXT` (RFC3339) | `TEXT` (RFC3339) | See below |
| `Option<T>` | nullable | nullable | SQL `NULL` ↔ Rust `None` |

**Why RFC3339 TEXT for datetimes:**
- Identical schema in both databases — no conversion layer
- No timezone confusion (`TIMESTAMPTZ` applies session TZ on read)
- SQLite `DATETIME` is just `TEXT` internally anyway
- RFC3339 strings sort lexicographically, so `WHERE updated_at > ?` works correctly as a plain string comparison

---

### Schema

Both schemas are intentionally nearly identical. The only differences are the `DEFAULT` expression syntax for `updated_at` and the upsert/ignore seed syntax.

**SQLite** (`migrations/001_initial.sql`):
```sql
CREATE TABLE IF NOT EXISTS items (
    id          TEXT PRIMARY KEY,
    -- ... your columns ...
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    synced_at   TEXT,          -- NULL = dirty (needs push); NOT NULL = synced
    deleted     INTEGER NOT NULL DEFAULT 0  -- soft-delete flag
);

INSERT OR IGNORE INTO seed_table (id, name) VALUES ('default', 'Default');
```

**PostgreSQL** (`migrations/001_initial.pg.sql`):
```sql
CREATE TABLE IF NOT EXISTS items (
    id          TEXT PRIMARY KEY,
    -- ... same columns ...
    updated_at  TEXT NOT NULL DEFAULT TO_CHAR(NOW() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    synced_at   TEXT,
    deleted     INTEGER NOT NULL DEFAULT 0
);

INSERT INTO seed_table (id, name) VALUES ('default', 'Default') ON CONFLICT DO NOTHING;
```

**Key sync columns:**

| Column | Role |
|---|---|
| `id` (UUID TEXT) | Client-generated, no coordination needed |
| `updated_at` (RFC3339) | Last-write timestamp; used as pull watermark |
| `synced_at` (RFC3339, nullable) | `NULL` = dirty/unsynced. Set to `NOW()` after successful push |
| `deleted` (0/1) | Soft-delete flag — see two-phase delete below |

---

### Setting Up Postgres (Homelab)

**Install:**
```sh
# Debian/Ubuntu
sudo apt install -y postgresql postgresql-contrib
sudo systemctl enable --now postgresql

# Fedora
sudo dnf install -y postgresql-server postgresql-contrib
sudo postgresql-setup --initdb
sudo systemctl enable --now postgresql
```

**Create user and database:**
```sh
sudo -u postgres psql
```
```sql
CREATE USER myapp WITH PASSWORD 'strong_password_here';
CREATE DATABASE myapp OWNER myapp;
GRANT ALL PRIVILEGES ON DATABASE myapp TO myapp;
\q
```

**Allow LAN connections — `postgresql.conf`:**
```sh
sudo -u postgres psql -c "SHOW config_file;"   -- find the file
# Edit it:
listen_addresses = '*'    -- or your specific LAN IP
```

**Allow LAN connections — `pg_hba.conf`** (same directory):
```
# TYPE  DATABASE  USER   ADDRESS           METHOD
host    myapp     myapp  192.168.1.0/24    scram-sha-256
```

**Restart and verify:**
```sh
sudo systemctl restart postgresql
pg_isready -h 192.168.1.100
```

**Firewall (ufw):**
```sh
sudo ufw allow from 192.168.1.0/24 to any port 5432
sudo ufw reload
```

**Firewall (firewalld):**
```sh
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="192.168.1.0/24" port protocol="tcp" port="5432" accept'
sudo firewall-cmd --reload
```

> Never expose port 5432 to the public internet. Use a VPN (WireGuard, Tailscale) for remote access. Tailscale is trivial to set up and gives every device a stable `100.x.x.x` address.

**Run migration and verify:**
```sh
psql -h 192.168.1.100 -U myapp -d myapp -f migrations/001_initial.pg.sql
psql -h 192.168.1.100 -U myapp -d myapp -c "\dt"
psql -h 192.168.1.100 -U myapp -d myapp -c "SELECT * FROM items LIMIT 5;"
```

---

### Local SQLite Setup

**Platform-aware DB path** using the `dirs` crate:
```rust
let db_path = {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("myapp");
    std::fs::create_dir_all(&p)?;
    p.push("data.db");
    p.to_string_lossy().to_string()
};
// Linux:   ~/.local/share/myapp/data.db
// macOS:   ~/Library/Application Support/myapp/data.db
// Windows: %LOCALAPPDATA%\myapp\data.db
```

**Open and migrate:**
```rust
let conn = Connection::open(&db_path)?;
conn.execute_batch("PRAGMA journal_mode=WAL;")?;  // always enable WAL (see below)
conn.execute_batch(include_str!("../migrations/001_initial.sql"))?;  // idempotent
```

**WAL mode:** The default journal mode takes an exclusive write lock, blocking concurrent reads. WAL allows readers and writers to proceed concurrently — essential when a background sync task reads while the UI writes.

**Incremental migrations** — for adding columns to existing databases:
```rust
let migrations = &[
    "ALTER TABLE items ADD COLUMN new_col INTEGER NOT NULL DEFAULT 0",
];
for sql in migrations {
    match conn.execute_batch(sql) {
        Ok(_) => {}
        Err(e) if e.to_string().contains("duplicate column name") => {}  // already applied
        Err(e) => return Err(e.into()),
    }
}
```

> `ALTER TABLE DROP COLUMN` is only supported in SQLite ≥ 3.35.0. `ADD COLUMN` is always safe.

**Verify locally:**
```sh
sqlite3 ~/.local/share/myapp/data.db ".tables"
sqlite3 ~/.local/share/myapp/data.db "SELECT id, synced_at FROM items LIMIT 5;"
# Or use DB Browser for SQLite (sqlitebrowser) for a GUI
```

---

### Connection Wrapping (`Arc<Mutex<Connection>>`)

`rusqlite::Connection` is not `Send + Sync`. Wrap it so it can be shared across Tokio tasks:

```rust
pub struct Db {
    pub conn: Arc<Mutex<Connection>>,
}
```

All DB calls must use `tokio::task::spawn_blocking` to avoid blocking the async runtime:

```rust
pub async fn fetch_items(conn: Arc<Mutex<Connection>>) -> Result<Vec<Item>> {
    task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| anyhow!("mutex poisoned: {e}"))?;
        let mut stmt = guard.prepare("SELECT ... FROM items WHERE deleted = 0")?;
        let items = stmt.query_map([], row_to_item)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    })
    .await?  // double-? : first resolves JoinHandle, second unwraps Result
}
```

---

### CRUD Pattern

Every write sets `updated_at = now()` and `synced_at = NULL` (marks dirty):

```rust
// Create — generate UUID client-side; synced_at=NULL marks it dirty
guard.execute(
    "INSERT INTO items (id, name, updated_at, synced_at)
     VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), NULL)",
    params![Uuid::new_v4().to_string(), name],
)?;

// Update — always reset synced_at to NULL
guard.execute(
    "UPDATE items SET name = ?1,
                      updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                      synced_at = NULL
     WHERE id = ?2",
    params![name, id],
)?;

// Soft-delete — mark dirty so sync can propagate before hard-delete
guard.execute(
    "UPDATE items SET deleted = 1,
                      updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                      synced_at = NULL
     WHERE id = ?1",
    params![id],
)?;

// Mark synced — called after successful push to Postgres
guard.execute(
    "UPDATE items SET synced_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
    params![id],
)?;

// Upsert from remote (pull sync)
guard.execute(
    "INSERT INTO items (id, name, updated_at, synced_at, deleted)
     VALUES (?1, ?2, ?3, ?4, ?5)
     ON CONFLICT(id) DO UPDATE SET
         name       = excluded.name,
         updated_at = excluded.updated_at,
         synced_at  = excluded.synced_at,
         deleted    = excluded.deleted",
    params![item.id, item.name, item.updated_at, item.synced_at, item.deleted],
)?;
```

**Row mapping:** use positional `row.get(n)` matching the `SELECT` column order. Parse RFC3339 strings back to `DateTime<Utc>`:
```rust
let updated_at: String = row.get(5)?;
let updated_at = DateTime::parse_from_rfc3339(&updated_at)
    .unwrap_or_default()
    .with_timezone(&Utc);

// For booleans stored as INTEGER:
let deleted: bool = row.get::<_, i64>(6).unwrap_or(0) != 0;
```

---

### Sync Logic

**Invariant:** `synced_at IS NULL` = row has local changes not in Postgres.

**`sync_once` — push then pull:**

```rust
pub async fn sync_once(conn: Arc<Mutex<Connection>>, pg: &PgClient) -> Result<bool> {
    let mut changed = false;

    // --- Push: dirty local rows → Postgres ---
    let dirty = fetch_dirty_items(Arc::clone(&conn)).await?;  // WHERE synced_at IS NULL
    for item in &dirty {
        if item.deleted {
            pg.execute("DELETE FROM items WHERE id = $1", &[&item.id.as_str()]).await?;
            hard_delete(Arc::clone(&conn), item.id.clone()).await?;
        } else {
            pg.execute(
                "INSERT INTO items (id, name, updated_at) VALUES ($1, $2, $3)
                 ON CONFLICT (id) DO UPDATE SET
                     name = EXCLUDED.name, updated_at = EXCLUDED.updated_at",
                &[&item.id.as_str(), &item.name.as_str(), &item.updated_at.to_rfc3339().as_str()],
            ).await?;
            mark_synced(Arc::clone(&conn), item.id.clone()).await?;
        }
        changed = true;
    }

    // --- Pull: Postgres rows newer than watermark → local ---
    let watermark = {
        let guard = conn.lock()?;
        guard.query_row("SELECT MAX(synced_at) FROM items", [], |r| r.get::<_, Option<String>>(0))
            .unwrap_or(None)
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
    };

    let rows = pg.query(
        "SELECT id, name, updated_at, deleted FROM items WHERE updated_at > $1",
        &[&watermark],
    ).await?;

    for row in &rows {
        let deleted: i32 = row.try_get(3).unwrap_or(0);
        if deleted != 0 {
            hard_delete(Arc::clone(&conn), row.get(0)).await?;
        } else {
            upsert_item(Arc::clone(&conn), item_from_row(row)).await?;
        }
        changed = true;
    }

    Ok(changed)
}
```

**Background sync loop:**

```rust
pub async fn run_sync_loop(
    conn: Arc<Mutex<Connection>>,
    pg_url: String,
    tx: UnboundedSender<AppAction>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;

        // Reconnect every tick — no pool to manage, transient failures auto-recover
        match tokio_postgres::connect(&pg_url, tokio_postgres::NoTls).await {
            Err(e) => { /* log e, loop continues */ }
            Ok((pg_client, pg_conn)) => {
                // pg_conn must be driven on its own task
                tokio::spawn(async move { let _ = pg_conn.await; });

                match sync_once(Arc::clone(&conn), &pg_client).await {
                    Ok(true) => { let _ = tx.send(AppAction::RefreshData); }
                    Ok(false) => {}
                    Err(e) => { /* log e */ }
                }
            }
        }
    }
}
```

`tokio_postgres` separates `Client` (send queries) from `Connection` (socket driver). The `Connection` must be `.await`ed on its own task — it exits when the socket drops, which is fine since we reconnect next tick anyway.

---

### Two-Phase Soft Delete

**Problem:** Hard-deleting immediately leaves nothing to push to Postgres — the row reappears on next pull from another device.

**Solution:**

1. **Soft-delete:** `UPDATE SET deleted=1, synced_at=NULL` — row vanishes from UI (filter `WHERE deleted=0`) but stays in SQLite
2. **Sync pushes:** `fetch_dirty` picks it up → `DELETE FROM items` on Postgres → `hard_delete` locally

**Without Postgres configured:** soft-deleted rows would accumulate. Clean up on each data refresh:
```rust
if std::env::var("DATABASE_URL").is_err() {
    for item in fetch_dirty().await?.iter().filter(|i| i.deleted) {
        hard_delete(conn.clone(), item.id.clone()).await?;
    }
}
```

---

### Configuration & Security

**Opt-in sync via env var** — if absent, app runs local-only:
```rust
if let Ok(pg_url) = std::env::var("DATABASE_URL") {
    tokio::spawn(run_sync_loop(conn, pg_url, tx));
}
```

**Connection string format** (libpq keyword/value):
```
host=192.168.1.100 dbname=myapp user=myapp password=secret
host=192.168.1.100 port=5433 dbname=myapp user=myapp password=secret
host=my-server.tailnet.ts.net dbname=myapp user=myapp password=secret
```

**Setting the env var:**
```sh
# Shell profile (~/.zshrc, ~/.bashrc)
export DATABASE_URL="host=192.168.1.100 dbname=myapp user=myapp password=secret"

# One-shot
DATABASE_URL="host=..." cargo run

# systemd user service
Environment=DATABASE_URL=host=192.168.1.100 dbname=myapp user=myapp password=secret
```

- Never commit credentials — add `.env` to `.gitignore`
- `NoTls` is fine if traffic goes through a VPN tunnel (Tailscale, WireGuard)
- For TLS: use `tokio-postgres-rustls` or `tokio_postgres::native_tls`

---

### Testing Sync End-to-End

```sh
# 1. Create a row in the app, wait ≤30s for sync tick
# 2. Verify it arrived in Postgres:
psql -h 192.168.1.100 -U myapp -d myapp -c "SELECT id, synced_at FROM items;"

# 3. Simulate offline → online:
sudo systemctl stop postgresql   # make changes in app
sudo systemctl start postgresql  # sync fires within 30s

# 4. Verify soft-delete propagation:
# Delete a row in the app. Check SQLite — row should exist with deleted=1:
sqlite3 ~/.local/share/myapp/data.db "SELECT id, deleted, synced_at FROM items WHERE deleted=1;"
# After sync, check Postgres — row should be gone:
psql -h ... -c "SELECT count(*) FROM items WHERE id='<uuid>';"
# Check SQLite again — hard-delete should have run:
sqlite3 ... "SELECT id FROM items WHERE id='<uuid>';"  -- (no rows)
```

---

---

## Part 2: Ratatui + Tokio TUI

---

### Dependencies

```toml
ratatui        = { version = "0.29", features = ["unstable-widget-ref"] }
crossterm      = { version = "0.28", features = ["event-stream"] }
tokio          = { version = "1", features = ["full"] }
tokio-util     = "0.7"   # CancellationToken
tokio-postgres = "0.7"
rusqlite       = { version = "0.32", features = ["bundled"] }
anyhow         = "1.0"
chrono         = "0.4"
uuid           = { version = "1", features = ["v4"] }
dirs           = "6"
once_cell      = "1"
futures        = "0.3"   # StreamExt/FutureExt for crossterm EventStream
tachyonfx      = "0.20"  # optional: terminal animations
```

`bundled` statically compiles SQLite — no system SQLite dependency.
`unstable-widget-ref` enables `WidgetRef` for stateful widgets passed by reference.

---

### Project Structure

```
src/
├── main.rs        # resolve DB path, Db::open, App::new, app.run()
├── app.rs         # App struct, run loop, AppAction enum, action dispatch
├── term.rs        # AppTerminal: raw mode, crossterm events, tick/render loop
├── tasks.rs       # domain types: Item, ItemData (DTO), ItemAction
├── debug.rs       # global debug sender, debug! macro
├── db/
│   ├── mod.rs     # Db struct, open + migrate
│   ├── local.rs   # all SQLite CRUD (spawn_blocking)
│   └── sync.rs    # sync_once, run_sync_loop
└── ui/
    ├── mod.rs         # AppUI, UIAction, mode switching, popup dispatch
    ├── normalmode.rs  # NormalModeUI, pane layout, ActivePane state machine
    ├── viewselector.rs
    ├── itemlist.rs
    ├── itemeditor.rs
    ├── focusmode/
    └── popup/
        ├── mod.rs     # Popup trait
        ├── confirm.rs
        ├── newitem.rs
        └── debug.rs   # auto-expiring debug overlay
```

---

### Terminal Abstraction

```rust
pub struct AppTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    task: JoinHandle<()>,
    cancellation_token: CancellationToken,
    event_rx: UnboundedReceiver<Event>,
    event_tx: UnboundedSender<Event>,
}
```

**Setup/teardown sequence:**
```rust
pub fn enter(&mut self) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout(), EnterAlternateScreen, cursor::Hide)?;
    crossterm::execute!(stdout(), EnableMouseCapture)?;
    // macOS: disambiguate escape codes for reliable modifier key detection
    #[cfg(target_os = "macos")]
    crossterm::execute!(stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES))?;
    self.start();  // spawn event polling task
    Ok(())
}

pub fn exit(&mut self) -> Result<()> {
    self.stop()?;
    if crossterm::terminal::is_raw_mode_enabled()? {
        self.flush()?;
        crossterm::execute!(stdout(), LeaveAlternateScreen, cursor::Show)?;
        crossterm::execute!(stdout(), DisableMouseCapture)?;
        crossterm::terminal::disable_raw_mode()?;
    }
    Ok(())
}

// Implement Drop — restores terminal even on panic
impl Drop for AppTerminal {
    fn drop(&mut self) { let _ = self.exit(); }
}
```

**Event polling task** (spawned in `start()`):
```rust
tokio::spawn(async move {
    let mut reader = crossterm::event::EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_secs_f64(1.0 / 10.0));   // 10 Hz
    let mut render_interval = tokio::time::interval(Duration::from_secs_f64(1.0 / 60.0)); // 60 Hz
    loop {
        tokio::select! {
            _ = cancellation_token.cancelled() => break,
            maybe_event = reader.next().fuse() => { /* forward Key/Mouse/Resize events */ }
            _ = tick_interval.tick() => { tx.send(Event::Tick).unwrap(); }
            _ = render_interval.tick() => { tx.send(Event::Render(last_frame)).unwrap(); }
        }
    }
});
```

Tick (10 Hz) drives animation state machines and time-based logic. Render (60 Hz) triggers frame draws. Keep them separate so animation state isn't coupled to rendering frequency.

---

### Action Bus Pattern

All state changes flow through a single enum dispatched over an mpsc channel:

```rust
#[derive(Debug, Clone)]
pub enum AppAction {
    Tick,
    Render(Instant),
    Resize(u16, u16),
    Quit,
    RefreshData,
    UpdateCache,
    ItemAction(ItemAction, ItemData),
    UIAction(UIAction),
    MultiAction(Vec<AppAction>),     // execute multiple actions atomically
    AfterNTicks(u32, Box<AppAction>), // delay an action by N tick events
}
```

```rust
let (tx, rx) = mpsc::unbounded_channel::<AppAction>();
```

`tx` is cloned into every component (UI widgets, sync loop, debug system). `rx` lives in `App::run()` only. UI components never touch the database directly — they send an `AppAction` and the `App` dispatches it.

**`AfterNTicks`** schedules an action after N ticks — used to let animations play before applying state changes (e.g. play completion animation, then actually remove the item after 3 ticks).

---

### App Run Loop

```rust
pub async fn run(&mut self) -> Result<()> {
    self.ti.enter()?;
    self.tx.send(AppAction::RefreshData)?;

    if let Ok(pg_url) = std::env::var("DATABASE_URL") {
        tokio::spawn(run_sync_loop(Arc::clone(&self.db.conn), pg_url, self.tx.clone()));
    }

    loop {
        // 1. Block until next terminal event (key, tick, render, ...)
        if let Some(event) = self.ti.next().await {
            self.handle_event(event)?;
        }

        // 2. Drain all pending actions (non-blocking)
        while let Ok(action) = self.rx.try_recv() {
            self.execute_action(action)?;
        }

        if self.quitting { break; }
    }

    self.ti.exit()?;
    Ok(())
}
```

Draining `rx` after every event ensures `MultiAction` batches are fully processed before the next render.

**Two-phase data refresh** (avoids blocking the render thread):

```rust
// Phase 1: fetch on background task, store in pending buffer
fn refresh_data(&mut self) {
    let conn = Arc::clone(&self.db.conn);
    let pending = Arc::clone(&self.pending_items);  // Arc<Mutex<Option<Vec<Item>>>>
    let tx = self.tx.clone();
    tokio::spawn(async move {
        match fetch_all_items(conn).await {
            Ok(items) => {
                *pending.lock().unwrap() = Some(items);
                let _ = tx.send(AppAction::UpdateCache);
            }
            Err(e) => { /* log */ }
        }
    });
}

// Phase 2: swap into cached_items on main thread (called on UpdateCache action)
fn update_cache(&mut self) {
    if let Some(items) = self.pending_items.lock().unwrap().take() {
        self.cached_items = Arc::new(items.into_iter().map(Arc::new).collect());
        self.ui.update_items(Arc::clone(&self.cached_items));
    }
}
```

---

### UI Architecture

**Mode switching:**
```rust
pub struct AppUI {
    mode: AppUIMode,          // Normal | Focus
    normal_ui: NormalModeUI,
    focus_ui: FocusModeUI,
    popup: Option<Box<dyn Popup>>,
    debug_popup: Option<DebugPopup>,
    tx: UnboundedSender<AppAction>,
}
```

The active popup intercepts all input before the mode UI sees it.

**Popup trait:**
```rust
pub trait Popup {
    fn handle_key_event(&mut self, key_event: KeyEvent);
    fn handle_mouse_event(&mut self, mouse_event: MouseEvent);
    fn allow_key_cmd(&self) -> bool;  // false while user is typing → blocks 'q' etc.
    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant);
}
```

**`allow_key_cmd()` guard** — prevents global keys from firing while in insert mode:
```rust
// AppUI
pub fn allow_key_cmd(&self) -> bool {
    match &self.popup {
        Some(p) => p.allow_key_cmd(),
        None => self.active_ui().allow_key_cmd(),
    }
}

// NormalModeUI
pub fn allow_key_cmd(&self) -> bool {
    match self.active_pane {
        ActivePane::Editor => !self.editor.is_in_insert_mode(),
        _ => true,
    }
}
```

**Three-pane Normal mode layout:**
```
[ViewSelector 15cols] | [ItemList / ItemEditor — fills rest]
```
ActivePane state machine: `ViewSelector ↔ ItemList ↔ ItemEditor`
- `l` / `→` / `Enter` → move right
- `h` / `←` → move left
- `Esc` (from editor, normal mode) → back to list

**View filtering:**
```rust
pub enum View { Today, Tomorrow, Week, Inbox, All, Completed }

impl View {
    pub fn contains_item(&self, now: DateTime<Utc>, item: &Item) -> bool {
        match self {
            View::Today     => is_due_today(now, item) || is_overdue(now, item),
            View::Tomorrow  => is_due_tomorrow(now, item),
            View::Completed => item.status == 2,
            View::All       => item.status != 2,
            // ...
        }
    }
}
```

On view selection change → send action → `ItemList::filter_by_view(&view)`.

**ItemEditor field layout example (4 fields):**
```rust
fn splits_fn(area: Rect) -> Vec<Rect> {
    let rows = Layout::vertical([Length(3), Length(3), Min(3)]).split(area);
    let mid  = Layout::horizontal([Percentage(70), Percentage(30)]).split(rows[1]);
    vec![rows[0], mid[0], mid[1], rows[2]]  // Title | DueDate | Priority | Description
}
```

Field validators return `Option<bool>` (None = not yet validated, Some(false) = invalid → colored border):
```rust
fn validate_due_date(input: &str) -> Option<bool> {
    if input.trim().is_empty() { None }
    else { Some(parse_datetime(input).is_ok()) }
}
```

---

### Key Bindings Reference

| Context | Key | Action |
|---|---|---|
| Global | `q` | Confirm-quit dialog |
| Global | `Ctrl+C` | Force quit |
| Global | `Ctrl+R` | Force data refresh |
| Global | `n` | New item popup |
| Global | `b` | Batch create popup |
| Global | `r` | Reschedule popup |
| ViewSelector | `j`/`k` | Navigate views |
| ViewSelector | `l`/`→`/`Enter` | Focus ItemList |
| ViewSelector | `f` | Enter Focus Mode |
| ItemList | `j`/`k` | Navigate items |
| ItemList | `h`/`←` | Focus ViewSelector |
| ItemList | `Enter` | Open in ItemEditor |
| ItemList | `d` | Delete (soft) |
| ItemList | `c` | Complete |
| ItemList | `x` | Toggle multi-select |
| ItemList | `f` | Enter Focus Mode |
| ItemEditor | `Ctrl+Shift+Enter` | Submit |
| ItemEditor | `Esc` (normal mode) | Back to ItemList |
| FocusMode | `j`/`k` | Navigate + animate |
| FocusMode | `Enter`/`Space`/`e` | Complete + animate |
| FocusMode | `Esc` | Back to Normal Mode |

---

### Animations (tachyonfx)

Each animated widget owns an `EffectManager<()>`. Call `process_effects` on every render:
```rust
let elapsed = last_frame.elapsed();
self.effects.process_effects(elapsed.into(), f.buffer_mut(), area);
```

Effects remove themselves from the manager when they expire.

```rust
// Sweep-in on load (reveal top-to-bottom over 500ms)
let fx = fx::sweep_in(Motion::UpToDown, 10, 0, Color::Rgb(25, 25, 25),
                      EffectTimer::from_ms(500, Interpolation::Linear));
effects.add_effect(fx);

// Coalesce on item open (cells resolve from scrambled to final over 200ms)
// Constrain to the field's inner area so effects don't bleed across fields
let fx = fx::coalesce(EffectTimer::from_ms(200, Interpolation::Linear))
    .with_area(inner_area);
effects.add_effect(fx);
```

---

### Debug Overlay

Can't `println!` in raw mode — it corrupts the UI. Instead, route debug messages through the action bus to a self-expiring overlay:

```rust
// debug.rs — initialized once at startup
static DEBUG_SENDER: OnceCell<UnboundedSender<AppAction>> = OnceCell::new();

pub fn init_debug_sender(sender: UnboundedSender<AppAction>) {
    DEBUG_SENDER.set(sender).expect("already initialized");
}

// Callable from anywhere without holding a reference to the sender:
#[macro_export]
macro_rules! debug {
    ($fmt:expr, $($arg:expr),+; $ticks:expr) => {
        $crate::debug::debug_f(format_args!($fmt, $($arg)*), $ticks)
    };
    ($($arg:tt)*) => {
        $crate::debug::debug(format_args!($($arg)*).to_string())
    };
}
```

```rust
debug!("sync error: {:#}", e);
debug!("value: {:?}", val; 40);  // show for 40 ticks (~4s at 10Hz)
```

`DebugPopup` renders as an overlay, decrements a tick counter on each `AppAction::Tick`, and removes itself when expired. Fits in ~50 lines with no external dependencies.

---

### Lifecycle: Key Press → Database → UI (example: delete item)

```
KeyCode::Char('d') pressed
  → crossterm EventStream → AppTerminal event task → App::handle_event
  → AppUI::handle_key_event → NormalModeUI (ActivePane::ItemList)
  → ItemList::handle_key_event → tx.send(AppAction::ItemAction(ItemAction::Delete, data))
  → App::execute_action → execute_item_action → tokio::spawn:
      local::soft_delete_item(conn, id).await   // deleted=1, synced_at=NULL
      tx.send(AppAction::RefreshData)
  → refresh_data() spawns background fetch → tx.send(AppAction::UpdateCache)
  → update_cache() swaps cached_items → ui.update_items(...)
  → next Render tick: item gone from list (filtered by WHERE deleted=0)
  → [background] sync loop picks up dirty row → DELETE on Postgres → hard_delete locally
```
