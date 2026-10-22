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
cd src-tauri && cargo run --bin server -- --port 8000 --web ../dist
```

Open http://localhost:8000.

Options: `--port` (default 8000), `--host` (default 127.0.0.1), `--data <folder>` for the database and pictures, and `--web <folder>` for the built interface.

### Development, with live reloading

Start the server, then the development interface:

```
cd frontend/src-tauri && cargo run --bin server -- --port 8000
cd frontend && npm run dev
```

Open http://localhost:3000. The development settings point the page at port 8000. To use another port, change both: `VITE_API_BASE=http://localhost:8001 npm run dev`.

## Desktop app

Run it in development (macOS or Windows):

```
cd frontend && node_modules/.bin/tauri dev
```

### macOS

Build the app and a disk image:

```
cd frontend && node_modules/.bin/tauri build --bundles app,dmg
```

The build is in `frontend/src-tauri/target/release/bundle/`. It isn't signed or notarised, so the first time you open it, right-click it and choose Open.

### Windows

Build the installer:

```
cd frontend
node_modules\.bin\tauri build --bundles nsis
```

The installer is in `frontend\src-tauri\target\release\bundle\nsis\`. It isn't signed, so Windows SmartScreen warns about it: choose More info, then Run anyway. The installer adds a Windows Firewall rule so the Z21 can reply over UDP.

The **Windows build** workflow on GitHub Actions builds the same installer on every pull request, and it's available as the `trainlink-windows-installer` artifact on the run's page.

## Android app

Needs Android Studio, with the NDK (Side by side) installed from its SDK Manager, and the Rust Android targets (`rustup target add aarch64-linux-android x86_64-linux-android`).

```
cd frontend && node_modules/.bin/tauri android init
cd frontend && node_modules/.bin/tauri android build --debug --apk --target aarch64 --target x86_64
```

The APK is in `frontend/src-tauri/gen/android/app/build/outputs/apk/`. The generated Android project (`src-tauri/gen/`) is not committed. `src-tauri/tauri` is a small launcher the Android build needs: its Gradle step runs `node tauri`, and Node needs a file with that name in that folder. It hands over to the Tauri CLI that the frontend installs.

## iPad app

Needs Xcode (from the Mac App Store), with its iOS simulator platform (`xcodebuild -downloadPlatform iOS`), the Rust iOS targets (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`), and `xcodegen` (`brew install xcodegen`).

The first time, accept the Xcode licence (`sudo xcodebuild -license accept`, then `sudo xcodebuild -runFirstLaunch`).

```
cd frontend && node_modules/.bin/tauri ios init
cd frontend && scripts/ios-ipad.sh
cd frontend && node_modules/.bin/tauri ios build --debug --target aarch64-sim
```

`ios init` regenerates the Xcode project, so run `scripts/ios-ipad.sh` after it each time. The generated project lives in `frontend/src-tauri/gen/apple`. Like the Android build, the Xcode build step runs `node tauri`, so `frontend/src-tauri/gen/apple/tauri` is a launcher that is committed.

## First steps

1. Open **Settings** and check the Z21 address (default 10.0.0.111, port 21105).
2. Press **Connect** on the connection card at the bottom left.
3. Add locomotives under **Locomotives**, then drive them from **Control**.
4. Draw or import a layout under **Layout Editor**.

## Troubleshooting

- **Port already in use:** another program, such as an old backend, is on that port. Use `--port` to pick another one.
- **Pictures or layouts missing:** check the data folder in the README. The web server and the desktop app keep separate data.
- **Z21 not connected:** check the address in Settings, and that your computer is on the same network as the Z21.
