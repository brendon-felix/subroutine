# Subroutine Server

## Overview

The project was refactored from a desktop app with a direct Postgres connection into a
client-server architecture:

```
simple-core     — domain types and scheduling logic only (no DB)
simple-server   — axum HTTP API server; owns Postgres and all business logic
simple-desktop  — GPUI desktop app; talks to the server over HTTP
```

The server runs as a systemd service on **fermi** (`100.112.215.8`, Tailscale).

---

## Crate responsibilities

### `simple-core`

Pure domain logic. No database dependency.

- **Types**: `Action`, `ActionState`, `ActionTarget`, `Event`, `Routine`, `RoutineStep`,
  `RecurrenceRule`, `RecurrenceUnit`, `AnyItem`
- **Scheduling**: `requeue_actions`, `expedite_actions`, `quantize`, `find_free_slot`, etc.
- `DEFAULT_ACTION_DURATION` (5 min) and `EXPEDITE_HORIZON` (6 h) are `pub` constants.
- `crate-type = ["cdylib", "rlib"]` — rlib for server/desktop, cdylib for the Android bridge.

### `simple-server`

Axum 0.8 HTTP server. Owns all persistence.

- Connects to Postgres via `sqlx` (rustls, no OpenSSL).
- Runs `sqlx::migrate!()` on startup — migrations are embedded at compile time from
  `crates/simple-server/migrations/`.
- `sqlx::FromRow` for `Action`, `Event`, `Routine` lives here (not in simple-core) because
  of the orphan rule — implemented via `ActionRow(Action)` / `EventRow(Event)` newtypes.
- Business logic (pipeline refresh, action queueing, recurrence) delegates to `simple-core`.

### `simple-desktop`

GPUI desktop app. No direct database access.

- `AppDatabaseStore` — GPUI entity wrapping an in-memory cache
  (`Vec<Action>`, `Vec<Event>`, `Vec<Routine>`) and a flume command channel.
- A dedicated background thread runs a tokio runtime (GPUI uses smol, not tokio, so
  reqwest needs its own runtime) with a `reqwest::Client`.
- GPUI `cx.spawn` tasks await flume reply channels and apply results to entity state,
  emitting typed events (`DataChanged`, `ActionDataChanged`, etc.) as signals.
- Server URL is read from `SUBROUTINE_SERVER_URL` or assembled from
  `SUBROUTINE_HOST` / `SUBROUTINE_PORT` (default `http://localhost:3000`).

---

## HTTP API

Base URL: `http://100.112.215.8:3000`

All request and response bodies are JSON. The `Action.state` field uses serde's default
enum representation:

| State | JSON |
|---|---|
| Backlogged | `{"Backlogged": null}` or `{"Backlogged": "2024-01-15"}` (with date hint) |
| Queued | `{"Queued": {"time": "2024-01-15T14:00:00Z", "is_static": false}}` |
| Completed | `{"Completed": "2024-01-15T14:05:00Z"}` |
| Skipped | `"Skipped"` |

### Endpoints

| Method | Path | Description | Response |
|---|---|---|---|
| `GET` | `/api/data` | Fetch all collections | `{actions, events, routines}` |
| `PUT` | `/api/actions/:id` | Create or update an action | `Action` |
| `DELETE` | `/api/actions/:id` | Soft-delete an action | 204 |
| `POST` | `/api/actions/:id/queue` | Move backlogged → queued; refreshes pipeline | `Vec<Action>` (all changed) |
| `POST` | `/api/actions/:id/backlog` | Move queued → backlogged | `Action` |
| `POST` | `/api/actions/:id/complete` | Mark complete; creates next recurrence if set | `{completed, next?}` |
| `PUT` | `/api/events/:id` | Create or update an event | `Event` |
| `DELETE` | `/api/events/:id` | Soft-delete an event | 204 |
| `PUT` | `/api/routines/:id` | Create or update a routine | `Routine` |
| `DELETE` | `/api/routines/:id` | Soft-delete a routine | 204 |
| `POST` | `/api/routines/:id/instantiate` | Create one action per routine step | `Vec<Action>` |
| `POST` | `/api/pipeline/refresh` | Requeue missed/floating actions | `Vec<Action>` (changed) |
| `POST` | `/api/pipeline/expedite` | Pull floating actions toward now | `Vec<Action>` (changed) |

### Nushell examples

```nushell
let server = "http://100.112.215.8:3000"

# Read
http get $"($server)/api/data" | get actions | select title state

# Create a backlogged action
http put $"($server)/api/actions/(random uuid)" {
    id: <uuid>  lineage_id: <uuid>  origin_routine_id: null
    title: "Buy milk"  content: null  duration: null
    recurrence: null  saved: false  state: {Backlogged: null}
}

# Queue it, complete it
http post $"($server)/api/actions/<uuid>/queue" {}
http post $"($server)/api/actions/<uuid>/complete" {}
```

---

## Desktop store — important design details

### Background thread + tokio runtime

GPUI uses **smol** as its async executor, not tokio. `reqwest` requires a tokio runtime.
The store spawns a dedicated `std::thread` with `tokio::runtime::Builder::new_current_thread()`
and processes HTTP commands from a `flume::unbounded` channel one at a time.

```
GPUI entity (smol)
  │  cmd_tx.send(Cmd::...)        ← synchronous, non-blocking
  │  cx.spawn → await reply_rx    ← smol task awaiting flume channel
  │
  └─► worker thread (tokio)
        while let Ok(cmd) = cmd_rx.recv_async().await
          run(&client, &base, cmd).await   ← sequential, one at a time
          reply_tx.send(result)
```

Because commands are processed one at a time, the order in which `cmd_tx.send` is called
is the order they execute on the server — no concurrency surprises.

### Optimistic vs. server-response updates

| Category | Pattern | Examples |
|---|---|---|
| Simple CRUD | Update local cache immediately, fire-and-forget to server | `upsert_action`, `delete_action`, `upsert_event`, etc. |
| Server-response ops | Wait for server response, then update cache | `complete_action`, `auto_queue_action`, `refresh_pipeline`, `expedite_actions`, `instantiate_routine` |
| Combined create + follow-up | Single command, two sequential requests | `upsert_and_queue_action`, `upsert_and_schedule_event`, `upsert_and_instantiate_routine` |

### The `UpsertAndQueueAction` race condition fix

The action creator previously called `upsert_action` then `auto_queue_action` as two
separate commands. Because commands are sequential, this *usually* worked — but failed when
the parser produced an action that was already `Queued` (e.g. the user typed a time), because
`/queue` requires the action to be in `Backlogged` state.

The fix: `Cmd::UpsertAndQueueAction` does both steps atomically inside the worker:
1. `PUT /api/actions/:id` — persist the action
2. If the action is already `Queued`, stop (parser set an explicit time — no re-queuing needed)
3. If the action is `Backlogged`, `POST /api/actions/:id/queue`

The action creator now calls `store.upsert_and_queue_action(action, cx)` instead of two
separate calls.

---

## Server setup on fermi

### Postgres

- Database: `subroutine`, owner: `subroutine` role
- Auth: `scram-sha-256` via TCP. Two rules in `/var/lib/pgsql/data/pg_hba.conf`:
  - `127.0.0.1/32` — for the server process running locally
  - `100.64.0.0/10` — for Tailscale clients (e.g. direct `psql` access from your Mac)
- Password stored in `~/.config/simple-server/env` (chmod 600)

### systemd service

System service (visible in Cockpit) at `/etc/systemd/system/simple-server.service`:

```ini
[Unit]
Description=Subroutine HTTP API server
After=network.target postgresql.service

[Service]
User=felixb
EnvironmentFile=/home/felixb/.config/simple-server/env
ExecStart=/usr/local/bin/simple-server
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Environment file at `~/.config/simple-server/env`:

```
DATABASE_URL=postgresql://subroutine:<password>@127.0.0.1/subroutine
BIND_ADDR=0.0.0.0:3000
RUST_LOG=simple_server=info,tower_http=info
```

### SELinux

Fedora enforces SELinux. The binary **must** live in `/usr/local/bin/` and have its context
restored after every deploy — `mv` from `/tmp` strips the context and causes a
`203/EXEC Permission denied` failure.

---

## Cross-compilation

The server is built on macOS and copied to fermi. OpenSSL doesn't cross-compile cleanly,
so both `sqlx` and `reqwest` use **rustls** (`runtime-tokio-rustls` / `rustls-tls` features)
— no system library dependency.

**Tooling**: `zig` (via Homebrew) + `cargo-zigbuild` + `rustup target add x86_64-unknown-linux-gnu`

### Deploy script

```bash
cargo zigbuild --release -p simple-server --target x86_64-unknown-linux-gnu

ssh felixb@fermi "sudo systemctl stop simple-server"
scp target/x86_64-unknown-linux-gnu/release/simple-server felixb@fermi:/tmp/simple-server
ssh felixb@fermi "sudo mv /tmp/simple-server /usr/local/bin/simple-server \
  && sudo restorecon /usr/local/bin/simple-server \
  && sudo systemctl start simple-server"
```

`restorecon` is required on every deploy to restore the SELinux `bin_t` context.
