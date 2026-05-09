# Tauri command authoring guide

This directory holds every `#[tauri::command]` handler in MeedyaDL. The notes
below codify the conventions so new commands stay consistent with existing
ones — particularly when M8 (BBC iPlayer), M9 (Spotify), and M10 (YouTube)
land their per-service command modules.

> **Audit v2 finding #8** — pure documentation, no code changes. Tauri's
> dependency-injection macro requires explicit parameter signatures, so
> `State<'_, T>` boilerplate is unavoidable per-command. This guide makes
> sure that boilerplate is uniform across modules.

## Anatomy of a command

```rust
/// Brief description of what the command does (one line).
///
/// Longer description if needed, including any side effects, error
/// modes, or rate-limiting behaviour the frontend should know about.
///
/// # Arguments
/// * `app` - Tauri's AppHandle for emitting events / resolving paths.
/// * `queue` - Managed state, injected via `State<'_, QueueHandle>`.
/// * `request` - Deserialized JSON payload from the frontend.
///
/// # Returns
/// * `Ok(StartDownloadResult)` - Newly-queued download ID + duplicate flag.
/// * `Err(String)` - Validation failure or backend I/O error.
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    request: DownloadRequest,
) -> Result<StartDownloadResult, String> {
    // ... implementation
}
```

The shape is fixed by Tauri:

1. `#[tauri::command]` attribute on a free function.
2. Doc comment summarising the command's purpose, arguments, and return.
3. Parameters in canonical order: `AppHandle`, `State<'_, …>`, then
   request payload fields (deserialized from the frontend).
4. Return type is **always** `Result<T, String>`. Tauri serialises
   `Err(String)` to a rejected JS Promise.

## Parameter conventions

### `AppHandle` first when present

If the command needs to emit events (`app.emit(…)`), resolve paths
(`app.path()…`), or open dialogs, take `app: AppHandle` as the first
parameter. Omit when the command is pure (no Tauri runtime touch).

### `State<'_, T>` after `AppHandle`

Every managed state handle is declared explicitly. **Each handle is
its own `State<'_, …>` parameter** — there's no way to bundle them.

```rust
pub async fn some_command(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    config: State<'_, ConfigHandle>,
    // ...
) -> Result<…, String> { … }
```

When adding a new managed state type:

1. Define the wrapper type alias next to the underlying state struct
   (`pub type FooHandle = Arc<Mutex<Foo>>;`).
2. Register it in `lib.rs::run()` via `.manage(handle)` BEFORE the
   first command that needs it can run.
3. Reference the type alias in every command signature; never spell
   out `State<'_, Arc<Mutex<Foo>>>` inline.

### Request payload last

Frontend-provided arguments come after `app` and the `State<…>` block,
in the order the frontend sends them. Use a single struct
(`DownloadRequest`, `SettingsImport`, etc.) when the payload has more
than ~3 fields.

## Async vs sync

**Mark every command `async`** unless the body is genuinely synchronous
(no `.await`, no I/O, no blocking work). Tauri runs sync commands on
the IPC thread, blocking the event loop; async commands run on the
Tokio runtime where they belong.

If a command needs to call sync blocking work (CPU-bound, blocking I/O),
wrap it in `tokio::task::spawn_blocking(…)` rather than running the
sync function directly inside an `async fn`.

## Error handling

Always use `Result<T, String>`. Map errors with a context-bearing
prefix:

```rust
let entry = keyring::Entry::new(SERVICE_NAME, &key)
    .map_err(|e| format!("Failed to create keyring entry: {e}"))?;
```

The message reaches the frontend verbatim, so it should be:

- **Actionable** — tell the user what went wrong, not just that something did.
- **Free of internal type names** — wrap library errors in user-facing prose.
- **Single-line** — toast/UI rendering doesn't paginate.

For commands that want to suppress *expected* failures (file picker
cancellation, "no entry" lookups), return `Ok(None)` / `Ok(())`
rather than an error.

## Registration

Every `#[tauri::command]` function MUST be listed in
[`src-tauri/src/lib.rs`](../lib.rs)'s `tauri::generate_handler!`
macro call. Forgetting to register a command produces a runtime
"command not found" error when the frontend invokes it.

```rust
.invoke_handler(tauri::generate_handler![
    commands::system::get_platform_info,
    commands::gamdl::start_download,
    // ... add new commands here
])
```

The frontend wrapper in [`src/lib/tauri-commands.ts`](../../../src/lib/tauri-commands.ts)
should also gain a typed wrapper (`export function startDownload(…)`)
so callers don't have to remember the snake_case command string.

## Naming

- **Module names**: noun, pluralised when the module groups related
  commands (`credentials.rs`, `dependencies.rs`, `updates.rs`).
- **Function names**: `verb_object` in snake_case
  (`store_credential`, `start_download`, `check_all_updates`).
- **Frontend wrappers**: matching `verbObject` in camelCase
  (`storeCredential`, `startDownload`, `checkAllUpdates`).
- **Avoid prefixes** like `cmd_` or `handle_` — the
  `#[tauri::command]` attribute already disambiguates.

## Per-service modules (M8 / M9 / M10)

When a new service lands, prefer one command module per service:

```
commands/
├── apple_music.rs   ← future home for AM-specific commands
├── bbc_iplayer.rs   ← M8 (BBC iPlayer session, auth)
├── spotify.rs       ← M9 (Votify credentials, OAuth)
├── youtube.rs       ← M10 (yt-dlp config, API key)
└── …
```

Service-agnostic commands (queue management, settings, system info)
stay in their existing modules. Cross-service infrastructure
(engine routing, status checks) lives in
[`commands/gamdl.rs`](gamdl.rs) for now and will be renamed once a
second service ships.

Each service module registers its commands in `lib.rs::run()` next
to the existing block. Per-service `State<'_, …>` handles
(`SpotifyAuthHandle`, `IplayerSessionHandle`) live in the matching
service module under [`src-tauri/src/services/`](../services/) and
get `.manage()`-d in `lib.rs` alongside `QueueHandle`.

## Reference

- [Tauri commands](https://v2.tauri.app/develop/calling-rust/) — official guide
- [State management](https://v2.tauri.app/develop/state-management/) — `State<'_, T>` injection
- [`commands/credentials.rs`](credentials.rs) — clean small-command example
- [`commands/gamdl.rs::start_download`](gamdl.rs) — full-featured command with State, AppHandle, request payload, rate-limiting
