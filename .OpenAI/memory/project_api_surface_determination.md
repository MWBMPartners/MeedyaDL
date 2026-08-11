---
name: MeedyaDL has no HTTP API — OpenAPI/Swagger is N/A for this repo
description: MeedyaDL is a Tauri desktop app; its only programmatic surface is in-process Tauri IPC. Do not generate OpenAPI here — the native-app-facing API is a separate MeedyaSuite backend repo.
type: project
---
**Decision (2026-08-03):** When asked to "update OpenAPI/Swagger documentation" or "add Swagger UI", the correct answer for the **MeedyaDL repo** is: **there is nothing to generate here.**

Why:
- MeedyaDL is a **Tauri 2.0 desktop application**. Its only programmatic interface is the **in-process Tauri IPC command set** (`#[tauri::command]` fns registered in `src-tauri/src/lib.rs`'s `generate_handler![]`, invoked from the bundled React WebView via `invoke()`). IPC has no URLs / HTTP verbs / status codes and is not a network endpoint — OpenAPI (an HTTP-API description format) cannot model it, and there is **no server to host Swagger UI on**.
- Confirmed there is **no HTTP-server dependency** anywhere: no `express` / `fastify` / `koa` in `package.json`, no `axum` / `actix` / `warp` / `rocket` / `hyper`-server / `utoipa` in `src-tauri/Cargo.toml`.
- The IPC contract is already enforced by `tools/audit-checks/check_ipc_commands.py` (registration + `invoke()` target validation) and mirrored in `src/lib/tauri-commands.ts` — that is MeedyaDL's "interface doc".

**Where the native-app API actually lives:** the Apple/Android/other native clients will consume a **separate first-party backend — the MeedyaSuite / MWBM-IntAppsAPI service** (same backend family as the remote feature-availability flags `INTAPPS_*` and the future server-issued MusicKit token architecture in `DEV_NOTES.md` → "Recommended Production Architecture"). That backend is in its **own repository**. Any OpenAPI/Swagger spec + shared-hosting (no-Docker) Swagger UI belongs **there**, not in MeedyaDL.

**How to apply:** If the user wants the backend API documented, they must point the session at (or `add_repo`) the MeedyaSuite backend repo — do not fabricate an OpenAPI document for the Tauri IPC surface (it would misrepresent an in-process interface as a web API). Public note recorded in `DEV_NOTES.md` → "Programmatic Interface / API Surface".
