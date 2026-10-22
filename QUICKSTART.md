# Quick start

Two ways to run TrainLink, from the repository root.

## Install the frontend packages (once)

```
cd frontend && npm install
```

## Web version

Build the interface, then start the server. The server serves both the API and the interface, on one address.

```
cd frontend && npm run build
cd ../src-tauri && cargo run --bin server -- --port 8000 --web ../frontend/dist
```

Open http://localhost:8000.

Options: `--port` (default 8000), `--host` (default 127.0.0.1), `--data <folder>` for the database and pictures, and `--web <folder>` for the built interface.

### Development, with live reloading

Start the server, then the development interface:

```
cd src-tauri && cargo run --bin server -- --port 8000
cd frontend && npm run dev
```

Open http://localhost:3000. The development settings point the page at port 8000. To use another port, change both: `VITE_API_BASE=http://localhost:8001 npm run dev`.

## Desktop app

Run it in development (macOS or Windows):

```
frontend/node_modules/.bin/tauri dev
```

### macOS

Build the app and a disk image:

```
frontend/node_modules/.bin/tauri build --bundles app,dmg
```

The build is in `src-tauri/target/release/bundle/`. It isn't signed or notarised, so the first time you open it, right-click it and choose Open.

### Windows

Build the installer:

```
frontend\node_modules\.bin\tauri build --bundles nsis
```

The installer is in `src-tauri\target\release\bundle\nsis\`. It isn't signed, so Windows SmartScreen warns about it: choose More info, then Run anyway. The installer adds a Windows Firewall rule so the Z21 can reply over UDP.

The **Windows build** workflow on GitHub Actions builds the same installer on every pull request, and it's available as the `trainlink-windows-installer` artifact on the run's page.

## First steps

1. Open **Settings** and check the Z21 address (default 10.0.0.111, port 21105).
2. Press **Connect** on the connection card at the bottom left.
3. Add locomotives under **Locomotives**, then drive them from **Control**.
4. Draw or import a layout under **Layout Editor**.

## Troubleshooting

- **Port already in use:** another program, such as an old backend, is on that port. Use `--port` to pick another one.
- **Pictures or layouts missing:** check the data folder in the README. The web server and the desktop app keep separate data.
- **Z21 not connected:** check the address in Settings, and that your computer is on the same network as the Z21.
