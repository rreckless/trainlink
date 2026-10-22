# Desktop and mobile plan for TrainLink

Plan for shipping TrainLink as standalone apps on macOS, Windows, iOS, and Android, with the apps kept in sync and the web version retired once the desktop apps reach parity. Planning only; no code has changed, and work has not started.

## Decisions

- **Standalone apps.** Each app runs its own control code and talks to the Z21 itself. No machine acts as a host.
- **Sync between devices.** Layouts, the locomotive roster, pictures, and function labels sync across all of a person's apps. Live state (speed, direction, turnout positions) does not sync; it comes from the Z21.
- **Tauri 2** wraps the web UI for desktop and mobile.
- **All features on every platform,** including the layout editor on phones and tablets.
- **Web version retired** once the Mac and Windows apps match it. Phones ship after that.
- **No Apple Developer account yet.** iOS work waits until one is enrolled. Mac, Windows, and Android come first.

## Where we start

- **Frontend:** React and Vite, with react-konva for the layout canvas. Browser-only parts are the AnyRail file picker (it reads the file in the browser, which is where the OneDrive error came from) and the API address.
- **Backend:** FastAPI with SQLAlchemy on Postgres in Docker. Tests already run on SQLite.
- **Z21:** UDP on the LAN at 10.0.0.111. Protocol frames are in `backend/app/drivers/z21_*.py`.
- **Local files:** locomotive pictures are stored under `backend/data`, tied to the source tree.

## Architecture: one core, shared by every platform

A phone can't run the Python backend, so the control code moves into something every platform can run:

- **Rust core inside Tauri.** It owns the Z21 link (UDP), the local SQLite database, the pictures, and the sync client. The web UI calls it through Tauri commands, so the UI is the same on every platform.
- **Z21 protocol** is rewritten in Rust from the Python frame code. The Python tests are the reference: the same input must produce the same bytes.
- **AnyRail importer** is ported from Python to TypeScript, next to the geometry code already in the frontend. The Python importer stays as the reference until the port matches it on your file.
- **Local database:** SQLite in each platform's app data folder. The apps keep working offline and sync when they can reach the sync service.

## Sync

Sync is the largest new piece, so it gets its own design.

**What syncs:** layouts (including track and pieces), locomotives (name, address, type, function labels, pictures), and the moving-time totals.

**What does not sync:** live Z21 state (speed, direction, function states, turnout positions). Each app reads these from the Z21 when it connects.

**How it works:**
- Each app writes to its local SQLite database first, then sends the change to a sync service. Changes made offline are queued and sent later.
- Every record carries a last-changed time and the device that changed it. Records are sent as whole rows, not field by field.
- **Conflicts:** when two devices change the same record, the later change wins. This is simple, but if two people edit the same layout at the same moment, one set of edits is lost. Acceptable for a single user; revisit if it causes problems.
- **Moving time** is stored as events (for example, "drove 42 seconds on this device") and added up, not overwritten. Otherwise two devices would undercount each other.
- **Pictures** are uploaded to the sync service and downloaded to other devices, at the 2 MB limit already in place.

**Sync service: Supabase.** Hosted Postgres with sign-in and realtime. Closest to the current database, and works on every platform. Firebase and CloudKit were rejected: Firebase has a different data model, and CloudKit is Apple only.

**Accounts: one per person.** Each person signs in with their own account. Each account owns its own layouts and roster, so a household member doesn't see another's data unless they share it later. Data is encrypted in transit.

**Backup:** Supabase is the backup of record, and the apps also offer a manual "Export backup" file. Supabase's free tier has no automatic backups, so the Pro plan (or an equivalent scheduled export) is needed before relying on it. Check current plan limits before committing.

## Phases

**Phase 0: Spike (about 2 days, when started).**
- Confirm the existing UI runs in a Tauri window on Mac, and that the canvas works in WebKit.
- Send a UDP packet to the Z21 from the Rust core, and see what macOS Local Network permission asks for.
- Check whether iOS needs a special entitlement for UDP broadcast. Z21 discovery depends on it.
- Try a Supabase project with one table, syncing from two test copies of the app.

**Phase 1: Mac app, local only.**
- Build the Rust core: Z21 link, SQLite store, pictures.
- Port the control commands and the locomotive, turnout, and roster routes to Tauri commands, checked against the existing tests.
- Port the AnyRail importer to TypeScript and check it on your file.
- Use the Tauri file dialog for the import, so errors name the file.
- Sign and notarize, package as DMG, add the Tauri updater.

**Phase 2: Sync.** Add the sync client to the Rust core, the sign-in, and the conflict rules above. Test two Mac copies against each other, including offline edits. Test moving time across devices.

**Phase 3: Windows.** Same core, WebView2, UDP firewall rule, and a code-signing certificate to avoid SmartScreen warnings.

**Phase 4: Web retired.** Once the Mac and Windows apps match the web version, the Python backend, Postgres, and Docker are removed. Your current data is moved to the sync service and to a local SQLite file first. The migration keeps Erinburgh (layout 3) and Class 08 Shunter (locomotive 3) intact, and your main database is not changed without asking.

**Phase 5: Android, all features.** Phone and tablet layouts for every feature, including the layout editor. The editor needs touch controls: larger handles, pinch to zoom, and a bigger hit area for track ends. Emergency Stop must always be reachable, including after a dropped connection. Android needs the Wi-Fi and local network permissions. Sideloading the APK needs no account; a Play Store listing needs a one-time developer fee.

**Phase 6: iOS, all features.** Starts when the Apple Developer account is enrolled. Same screens as Android, plus the iOS Local Network permission.

**Phase 7: Store releases.** Mac and Windows by download, Android through Play, iOS through TestFlight and the App Store. CI on GitHub Actions with macOS, Windows, and Linux runners.

## Risks

- **The layout editor on a phone** is the largest UI risk. Canvas editing with fingers is harder than with a mouse, so it needs its own testing pass on real devices.
- **Sync conflicts** can lose edits when the same layout is changed on two devices at once. The current rule is simple; a merge approach is a later option.
- **Two apps driving one Z21:** the Z21 accepts several LAN clients, but each app shows its own live state until the Z21 broadcast reaches it.
- **Local Network permission** on macOS and iOS, and firewall rules on Windows, can block UDP. Checked in Phase 0 and Phase 3.
- **Porting risk:** the Rust Z21 code and the TypeScript importer must match the Python versions on tests before the Python backend is retired.
- **Safety on a phone:** a dropped Wi-Fi connection must show clearly, and Emergency Stop must still work after reconnecting.

## Decisions made

1. **Sync service:** Supabase.
2. **Accounts:** one per person.
3. **Backup:** Supabase is the backup of record, with a manual export file in the apps. Recommended because every device holds a full local copy, so one lost device loses nothing, but a single agreed record is still needed for restores. The Supabase plan must include scheduled backups.

No open questions remain in this plan.

## Status: Mac app built

**Built (uncommitted, on branch `desktop-mac`):**
- `src-tauri/`: a Rust core inside Tauri. It holds the database (SQLite in the app data folder), the Z21 link, the full API the web UI uses, the track library, moving time, and the AnyRail importer.
- The UI calls the core at `trainlink://localhost` when it runs in the app. The browser build still calls the web backend on port 8000.
- Release bundles: `TrainLink.app` and `TrainLink_0.1.0_aarch64.dmg`. They are not signed or notarised, since that needs an Apple Developer account, so the first launch needs right-click, Open.

**Checked:**
- 46 Rust tests: API contracts, Z21 command bytes and replies, moving time, and the importer against the Python output for five AnyRail fixtures.
- Your AnyRail file: the Rust importer gives the same 97 pieces and the same positions as the Python importer.
- The web UI run against the Rust core through a development bridge: control 15 of 15, import 11 of 11, bend 5 of 5, drag 4 of 4, length 3 of 3, drop 2 of 2, pan 3 of 3, setup 8 of 8.
- The Z21 link was tested against a fake Z21 on this Mac. Nothing was sent to the real command station.

**Not checked yet:**
- The app window itself. Screen capture was blocked in the test session, so the first launch needs a visual check.
- The custom `trainlink://` address inside the app's web view.
- The free-placement UI script. It's out of date (it still expects the old bend points) and fails at the same step on the Python backend.

**Bugs found and fixed during the port:**
- The Python importer mapped slip and three-way ends to the wrong legs when their order was scrambled. A scrambled slip came out with gaps of more than 150 mm. Fixed in the Python importer too, and the importer tests still pass.
- Picture uploads from the browser are multipart forms, which the Rust core did not accept. Fixed.

**Left in place on purpose:** the Python backend, Postgres, and Docker still serve the web version. Retiring them is your call. The track library JSON is copied into `src-tauri/library`, and the backend keeps its own copy until then.

**Not started:** sync (needs a Supabase project and credentials), Windows, iOS, and Android.
