# TrainLink

Model railway control for DCC layouts, built around the Roco Z21. TrainLink runs as a desktop app on macOS and Windows, and the same core serves the web interface.

## What it does

- **Control:** drive locomotives (speed, direction, 20 function buttons with labels), several at once, and emergency stop
- **Layout editor:** draw track from PECO N gauge pieces (including curved flex track), place turnouts with DCC addresses, and operate them from the layout
- **AnyRail import:** convert a JMRI layout-config XML exported from AnyRail into track pieces
- **Roster:** locomotives with pictures, function labels, and moving time
- **Z21:** connect, track power, turnout and locomotive commands, and live state (speed, direction, functions, turnouts, feedback)
- **Accounts and sync:** sign in with email and password, or with Google. Layouts, the roster, pictures, and moving time sync between the devices signed in to the same account

## Status

| Part | State |
|------|-------|
| macOS desktop app | Built as an unsigned DMG. Runs on this Mac |
| Windows desktop app | Built as an unsigned installer by GitHub Actions. Tested on a Windows PC: it runs, shows synced data, and connects to the Z21 |
| Web version | Runs on the same core as the desktop app |
| Email and Google sign-in | Working on the desktop app and the web version |
| Sync between devices | Working. Tested between devices, with unit and live tests passing |
| Code signing | None yet. Windows shows a SmartScreen warning, and macOS needs right-click, Open the first time |
| Android and iOS | Not started |

The plan is in [docs/desktop-plan.md](docs/desktop-plan.md).

## Repository layout

| Folder | What it is |
|--------|------------|
| `frontend/src-tauri/` | The Rust core: database, Z21 link, API, AnyRail importer, sign-in and sync, the web server (`trainlink-server`), and the Tauri desktop shell |
| `frontend/` | The React interface, used by the desktop apps and the web version |
| `supabase/` | The cloud database schema and its row-level security tests |
| `docs/` | The desktop, sync, and mobile plan, and the login plan |

## Requirements

- **macOS:** Rust (stable), installed with [rustup](https://rustup.rs), and Node.js 20 or later
- **Windows:** Git, Node.js 22, Rust (stable, MSVC toolchain), and the Visual Studio C++ build tools. Only needed to build from source; the installer does not need them

## Installing

- **Mac:** build the DMG (see [QUICKSTART.md](QUICKSTART.md)), open it, and drag TrainLink to Applications. Right-click it and choose Open the first time.
- **Windows:** run the `.exe` installer. It installs for all users and adds a Windows Firewall rule so the Z21 can reply over UDP. Windows SmartScreen will warn that the app is unknown; choose More info, then Run anyway. Installers are built by the **Windows build** workflow on GitHub Actions. Open a run and download the `trainlink-windows-installer` artifact.

## Quick start

See [QUICKSTART.md](QUICKSTART.md) for the step-by-step guide to the web version and the desktop app.

## Tests

```
cd frontend/src-tauri && cargo test    # Rust core: API, Z21 bytes, importer, web server, sign-in
cd frontend && npm test             # layout geometry and model
```

The Rust tests use an in-memory database, a fake Z21, and a fake sign-in service on this machine. They never send to your command station or to the real sign-in service. The Windows build workflow runs the same tests on Windows.

## Your Z21

The default is 10.0.0.111, port 21105. Set it in Settings. It must be on this computer's network, given as an IP address. Track power is only resumed on your explicit request, and the emergency stop is on every page.

## Where the data is kept

| Mode | Data folder |
|------|-------------|
| Desktop app (Mac) | `~/Library/Application Support/com.trainlink.app` |
| Desktop app (Windows) | `%APPDATA%\com.trainlink.app` |
| Web server (Mac) | `~/Library/Application Support/com.trainlink.web` (default; change with `--data`) |
| Web server (Windows) | `%APPDATA%\com.trainlink.web` (default; change with `--data`) |

Each folder holds `trainlink.db` (SQLite), `locomotive_images/`, and the sign-in in `session.json`. The web server on a network address also keeps `server-token` there.

## Security

- **The web server** listens on `127.0.0.1` by default. Only this computer can reach it, so it needs no token. Requests must name the server by an IP address or `localhost`, and pages from other sites are refused. Writes must be JSON or a form, so a plain form from another site cannot make one.
- **On a network** (`--host 0.0.0.0` or a LAN address), each device must sign in with the token. The server creates the token the first time it starts with a network address, and prints where it is kept (`server-token` in the data folder). The browser keeps the sign-in in a cookie that other sites cannot read or send.
- **The Z21 address** must be this computer or a private network address, given as an IP address. The app will not talk to a Z21 anywhere else.
- **The desktop apps** have no network port. Their own pages are the only ones that can call their API, and the page's security policy limits what it can load.
- **Sign-in with Google** uses PKCE, so only the app that started a sign-in can complete it.

## Documentation

- [QUICKSTART.md](QUICKSTART.md): run the web version and the desktop app
- [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md): the current architecture and status
- [iTrain_Implementation_Plan.md](iTrain_Implementation_Plan.md): the feature plan and status
- [docs/desktop-plan.md](docs/desktop-plan.md): the desktop, sync, and mobile plan
- [docs/login-plan.md](docs/login-plan.md): accounts, sign-in, and sync
