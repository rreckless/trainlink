# TrainLink

Model railway control for DCC layouts, built around the Roco Z21. TrainLink runs as a Mac desktop app, and the same core serves the web interface.

## What it does

- **Control:** drive locomotives (speed, direction, 20 function buttons with labels), several at once, and emergency stop
- **Layout editor:** draw track from PECO N gauge pieces (including curved flex track), place turnouts with DCC addresses, and operate them from the layout
- **AnyRail import:** convert a JMRI layout-config XML exported from AnyRail into track pieces
- **Roster:** locomotives with pictures, function labels, and moving time
- **Z21:** connect, track power, turnout and locomotive commands, and live state (speed, direction, functions, turnouts, feedback)

## Repository layout

| Folder | What it is |
|--------|------------|
| `src-tauri/` | The Rust core: database, Z21 link, API, AnyRail importer, the web server (`trainlink-server`), and the Tauri desktop shell |
| `frontend/` | The React interface, used by the desktop app and the web version |
| `docs/` | The desktop and mobile plan |
| `electron/` | An unused scaffold, to be removed |
| `docker-compose.yml` | PostgreSQL and Redis. Not used by the app. Kept for the data move from the old backend |

## Requirements

- macOS (the desktop app is built for macOS for now)
- Rust (stable), installed with [rustup](https://rustup.rs)
- Node.js 20 or later

## Quick start

See [QUICKSTART.md](QUICKSTART.md) for the step-by-step guide.

## Tests

```
cd src-tauri && cargo test          # Rust core: API, Z21 bytes, importer, web server
cd frontend && npm test             # layout geometry and model
```

The Rust tests use an in-memory database and a fake Z21 on this machine. They never send to your command station.

## Your Z21

The default is 10.0.0.111, port 21105. Set it in Settings. Track power is only resumed on your explicit request, and the emergency stop is on every page.

## Where the data is kept

| Mode | Data folder |
|------|-------------|
| Desktop app (Mac) | `~/Library/Application Support/com.trainlink.app` |
| Desktop app (Windows) | `%APPDATA%\com.trainlink.app` |
| Web server (Mac) | `~/Library/Application Support/com.trainlink.web` (default; change with `--data`) |
| Web server (Windows) | `%APPDATA%\com.trainlink.web` (default; change with `--data`) |

Each folder holds `trainlink.db` (SQLite) and `locomotive_images/`.

## Security

The web server listens on `127.0.0.1` only, and it has no login. Do not expose it to a network. Until the security work is finished, anything that can reach the server can drive your trains. See `docs/desktop-plan.md` for the planned fixes.

## Documentation

- [QUICKSTART.md](QUICKSTART.md): run the web version and the desktop app
- [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md): the current architecture and status
- [iTrain_Implementation_Plan.md](iTrain_Implementation_Plan.md): the feature plan and status
- [docs/desktop-plan.md](docs/desktop-plan.md): the desktop, sync, and mobile plan
