# Implementation summary

The current state of TrainLink. For the feature list and detailed status, see `iTrain_Implementation_Plan.md`. For the desktop, sync, and mobile plan, see `docs/desktop-plan.md`.

## Architecture

One core, two ways in:

```
 Desktop app (Tauri)  ── in-process calls ──┐
                                            ├── Rust core (src-tauri/src)
 Web UI (browser)     ── HTTP on /api ──────┘     ├── SQLite database
                                                  ├── Z21 link (UDP, port 21105)
                                                  ├── API (same paths and JSON as the old backend)
                                                  ├── track library (from JSON built into the app)
                                                  ├── moving time
                                                  └── AnyRail importer
```

- **Frontend** (`frontend/`): React, Vite, Tailwind, react-konva for the layout canvas, Zustand for state. The same build is used by the desktop app and the web version.
- **Rust core** (`src-tauri/src/`): the only backend.
  - `api/`: the REST routes, with the validation rules of the old backend
  - `db/`: SQLite tables for layouts, locomotives, blocks, signals, feedbacks, routes
  - `z21/`: frames, commands, replies, and the connection (a reader thread applies the Z21's replies to the shared state)
  - `anyrail/`: the AnyRail importer, checked against the old Python importer's output
  - `server.rs` and `bin/server.rs`: the web server (`trainlink-server`)
  - `lib.rs`: the desktop app, which answers the UI on the `trainlink://localhost` protocol
- **Storage:** SQLite per app. Data folders are listed in the README.

## What is built

- **Control:** the roster with multi-select speed and direction, emergency stop, 20 function buttons with labels and momentary buttons, pictures, and moving time
- **Z21:** connect, status, track power resume after a short circuit, turnout throws, locomotive commands, and parsing of the replies for speed, direction, functions, turnouts, and feedback
- **Layout editor:** drawing, placement, curved flex track with handles, joins, rotation, rename and delete, open-end markers, AnyRail import
- **Operate:** turnouts with DCC addresses, switched from the layout in operate mode
- **Settings:** the Z21 address
- **Shell:** top bar with page search, track power state, and emergency stop; grouped navigation with a collapse toggle; connection card

## What is not built yet

- Live occupancy shown on the Switchboard and the layout
- Route running (the Run button does nothing; the core refuses to run a route)
- Automatic signals
- Consists, multi-throttle, controller support, decoder programming
- Sync between devices (designed, not built)
- The security work: origin and content-type checks, the importer's crash on malformed XML, the desktop CSP, the token for network use, the Z21 host policy

## Testing

- **Rust core:** `cargo test` in `src-tauri/`. Covers the API rules, the Z21 bytes (checked against the old backend's output), the replies, a fake Z21 on this machine, moving time, the importer against fixtures, and the web server.
- **Frontend:** `npm test` in `frontend/`, covering layout geometry and the model.
- **UI scripts:** browser scripts were run against the built web UI served by the core (Control, Setup, Import, Bend, Drag, Length, Drop, Pan). They are not in the repository.

## Known issues

- The Route Manager's Run button does nothing.
- The Switchboard lists only the first five blocks.
- The free-placement UI script is out of date.
- The Z21 has not been tested with the desktop app against hardware since the port, only through the fake Z21.
