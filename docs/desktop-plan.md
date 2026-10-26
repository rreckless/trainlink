# Desktop and mobile plan for TrainLink

Plan for shipping TrainLink as standalone apps on macOS, Windows, iOS, and Android, with the apps kept in sync. The web version stays, and the same core serves it. Planning only; no code has changed, and work has not started.

## Decisions

- **Standalone apps.** Each app runs its own control code and talks to the Z21 itself. No machine acts as a host.
- **Sync between devices.** Layouts, the locomotive roster, pictures, and function labels sync across all of a person's apps. Live state (speed, direction, turnout positions) does not sync; it comes from the Z21.
- **Tauri 2** wraps the web UI for desktop and mobile.
- **All features on every platform,** including the layout editor on phones and tablets.
- **One backend.** The Rust core serves the desktop apps and the web version. The Python backend, Postgres, and Docker are retired once the web version runs on the core and passes the same tests.
- **No Apple Developer account yet.** iOS work waits until one is enrolled. Mac, Windows, and Android come first.

## Where we start

- **Frontend:** React and Vite, with react-konva for the layout canvas. Browser-only parts are the AnyRail file picker (it reads the file in the browser, which is where the OneDrive error came from) and the API address.
- **Backend:** the Rust core, on SQLite. The Python backend has been removed (see git history).
- **Z21:** UDP on the LAN at 10.0.0.111. Protocol frames are in `src-tauri/src/z21/`.
- **Local files:** locomotive pictures are stored in each app's data folder (see the README).

## Architecture: one core, shared by every platform

A phone can't run the Python backend, so the control code moves into something every platform can run:

- **Rust core inside Tauri.** It owns the Z21 link (UDP), the local SQLite database, the pictures, and the sync client. The web UI calls it through Tauri commands, so the UI is the same on every platform.
- **Z21 protocol:** rewritten in Rust (done). The same input produces the same bytes as the old Python code, checked by tests.
- **AnyRail importer:** ported to Rust (done), not TypeScript. It matches the old Python importer on the fixtures and on your AnyRail file.
- **Web version on the same core.** The core serves the web UI over local HTTP, with the same API the desktop app uses. It carries the origin and content-type checks from the security plan, and a token when reachable from the LAN. The Python backend has been retired: the web UI runs on the core (done).
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

**Web version:** the browser build signs in to the same Supabase accounts and keeps a local copy in IndexedDB, with a service worker so the app loads without a connection. This is extra work beyond the desktop sync.

**Accounts: one per person.** Each person signs in with their own account. Each account owns its own layouts and roster, so a household member doesn't see another's data unless they share it later. Data is encrypted in transit.

**Backup:** Supabase is the backup of record, and the apps also offer a manual "Export backup" file. Supabase's free tier has no automatic backups, so the Pro plan (or an equivalent scheduled export) is needed before relying on it. Check current plan limits before committing.

## Phases

Status is marked after each phase. Done means built and checked on the devices named; anything not yet checked is listed under Outstanding.

**Phase 0: Spike (about 2 days, when started).** Done. The Z21 connects from the Mac, Windows, Android and iPad. Not separately recorded: the iOS Local Network permission prompt.
- Confirm the existing UI runs in a Tauri window on Mac, and that the canvas works in WebKit.
- Send a UDP packet to the Z21 from the Rust core, and see what macOS Local Network permission asks for.
- Check whether iOS needs a special entitlement for UDP broadcast. Z21 discovery depends on it.
- Try a Supabase project with one table, syncing from two test copies of the app.

**Phase 1: Mac app, local only.** Done, except signing and notarising (needs the Apple Developer Program) and the Tauri updater, which isn't added. The importer is in Rust rather than TypeScript, and checked against the Python output.
- Build the Rust core: Z21 link, SQLite store, pictures.
- Port the control commands and the locomotive, turnout, and roster routes to Tauri commands, checked against the existing tests.
- Port the AnyRail importer to TypeScript and check it on your file.
- Use the Tauri file dialog for the import, so errors name the file.
- Sign and notarize, package as DMG, add the Tauri updater.

**Phase 2: Sync.** Done. The sync client, sign-in and conflict rules are in place, and settings sync too. Tested between devices, including a two-device live test of settings. Not yet tested: offline edits, and moving time across devices (unit tests only).

**Phase 3: Windows.** Done, apart from code signing. The NSIS installer, the firewall rule and WebView2 are in place, and the installer runs on a Windows PC, shows synced data and connects to the Z21. The code-signing certificate to avoid SmartScreen warnings isn't bought.

**Phase 4: Web on the core, Python retired.** Mostly done: the web UI runs on the core and the Python backend is removed. Outstanding: Postgres remains until the data move. Your current data is moved to the sync service and to a local SQLite file first. The migration keeps Erinburgh (layout 3) and Class 08 Shunter (locomotive 3) intact, and your main database is not changed without asking.

**Phase 5: Android, all features.** Partly done: the app runs on a Chromebook, with sign-in (email and Google), the Z21 connection, layouts and locomotives. Outstanding: phone and tablet layouts for every feature, including the layout editor. The editor needs touch controls: larger handles, pinch to zoom, and a bigger hit area for track ends. Emergency Stop must always be reachable, including after a dropped connection. Android needs the Wi-Fi and local network permissions. Sideloading the APK needs no account; a Play Store listing needs a one-time developer fee.

**Phase 6: iOS, all features.** Partly done: the iPad app builds, runs in development and as a standalone release, signed with a free Apple ID. Free signing expires after about 7 days, so the build has to be renewed. Outstanding: enrolment in the Apple Developer Program (for TestFlight and the App Store), the iOS Local Network permission check, and Sign in with Apple. Phones aren't built yet.

**Phase 7: Store releases.** Partly done: GitHub Actions workflows build the Windows installer, the Android APK and the macOS DMG, each run by hand. Outstanding: release signing for all three, Play Store, TestFlight and the App Store listings.

## Risks

- **The layout editor on a phone** is the largest UI risk. Canvas editing with fingers is harder than with a mouse, so it needs its own testing pass on real devices.
- **Sync conflicts** can lose edits when the same layout is changed on two devices at once. The current rule is simple; a merge approach is a later option.
- **Two apps driving one Z21:** the Z21 accepts several LAN clients, but each app shows its own live state until the Z21 broadcast reaches it.
- **Local Network permission** on macOS and iOS, and firewall rules on Windows, can block UDP. Checked in Phase 0 and Phase 3.
- **Porting risk:** resolved for the web UI. The Rust Z21 code and importer match the old Python versions on tests before the Python backend was removed.
- **Safety on a phone:** a dropped Wi-Fi connection must show clearly, and Emergency Stop must still work after reconnecting.

## Decisions made

1. **Sync service:** Supabase.
2. **Accounts:** one per person.
3. **Backup:** Supabase is the backup of record, with a manual export file in the apps. Recommended because every device holds a full local copy, so one lost device loses nothing, but a single agreed record is still needed for restores. The Supabase plan must include scheduled backups.

No open questions remain in this plan.

## Features to add on every platform

These come from the review against the other app (see the implementation plan, Phases 8 and 9). Each is built once in the Rust core and the UI, then shipped on every platform:

- **Live occupancy:** the core already reads the Z21 feedback. It needs a feedback event to the UI, and the Switchboard and layout editor need to show it.
- **Route running:** needs automatic driving in the core before the Run button can work.
- **Multi-throttle:** the core sends the commands; the UI shows each throttle. Consists are built (see Status).
- **Controller support:** the input layer differs per platform (gamepad input in the web view on desktop, separate handling on phones). Emergency stop stays on a fixed button.
- **Decoder programming:** new Z21 programming commands in the Rust core. Writes need the same confirmation and read-back as the web version.
- **Cab view:** deferred until a camera or video source is chosen.

Sync (above) also covers the new data: consists, route definitions, and decoder profiles sync like the roster.

## Status

**Built:** the Rust core inside Tauri, with the database, the Z21 link, the API, the track library, moving time, the AnyRail importer, sign-in, sync, settings sync, and consists (saved on each device, not yet synced). The apps: the Mac app and DMG, the Windows installer, the Android APK, and the iPad app. The web version runs on the same core. Each platform's build is run by hand from GitHub Actions for Windows, Android and the Mac, or locally for the Mac and iPad.

**Checked:**
- Rust tests: 99 pass (2 ignored). Frontend tests: 60 pass.
- The web UI through the browser: control, setup, import, bend, drag, length, drop and pan suites, with the pan test flaky on both the old and new builds.
- Z21 on hardware: the connection, track state, locomotive control and turnout colours follow the Z21, from the desktop app, the web version and Android. The iPad was reported working.
- Sign-in: email and Google, from the desktop, the web version and Android. Sign-in on the iPad was reported working, but the Google route on the iPad hasn't been recorded.
- Sync: between two devices, live, in both directions.

**Outstanding:**
- Signing and notarising the Mac app, and adding the Tauri updater.
- Code signing for the Windows installer, to avoid SmartScreen warnings.
- Android: the layout editor on phones and tablets, with touch controls, and Emergency Stop reachable after a dropped connection.
- iOS: Apple Developer Program enrolment, the Local Network permission check, Sign in with Apple, and phone layouts.
- Sync: offline edits and moving time across devices are not yet tested live.
- The data move from Postgres. The Docker Compose file is already removed.
- Store releases.
- Consists: sync between devices, and a run on a real Z21. Built, but not yet tested on hardware.
- Route running, live occupancy, multi-throttle, controller support, decoder programming, and cab view. None is started.

