# TrainLink - Implementation Plan

## Project Overview
Build a web-based replica of iTrain, a digital model railroad control system for managing locomotives, blocks, signals, and routes, with manual and automatic control. TrainLink targets the **Roco Z21** command station first. Other command station drivers are added only after Z21 is fully functional (Phase 7).

**Status key:** ✅ Done · 🟡 Partial · 🧪 Built but untested · ⬜ Not started

**Last reviewed:** 2026-10-20

---

## Current Status Snapshot

| Area | Status | Notes |
|------|--------|-------|
| Project setup: Rust core, Tauri desktop shell, React frontend | ✅ | Renamed to TrainLink. The old `docker-compose.yml` is removed. The PostgreSQL data move is still outstanding |
| Z21 UDP driver and API routes | ✅ | Connect, speed, direction, emergency stop, status, track power resume. Speed, direction, and emergency stop verified on hardware. Turnout commands and R-BUS feedback not yet tried on hardware |
| Locomotive control and roster | 🧪 | Multi-select speed and direction, emergency stop, 20 function buttons with labels and momentary buttons, pictures, moving time. Browser-tested with the Z21 stubbed. Functions and moving time not yet verified on hardware |
| Consists | 🧪 | Link locomotives, drive the group with a direction per member, and switch functions for the whole consist. Saved on each device, not synced. Not yet run on a Z21 |
| Z21 state (locomotive replies, track state) | 🧪 | Replies parsed for speed, direction, and functions, and shown on the roster. Short circuit and track power off block commands. Tested against a fake Z21 in the Rust tests |
| Control page (operate layout, Z21 connection) | 🧪 | Layout in operate mode, turnout throws, Z21 connection at the top. Browser-tested with the Z21 stubbed |
| Web app shell | ✅ | Top bar with page search, track power state, and emergency stop. Grouped navigation with a collapse toggle. Connection card |
| System settings UI (Z21 host and port) | ✅ | Host and port are shared with the Control page |
| Desktop app (Mac) | 🧪 | Tauri app with a Rust core: the same API, database, Z21 link, and importer. Release DMG built. Window and Control page checked. Pull request #12. Not yet run against the real Z21 |
| Database models and schemas | 🟡 | Locomotive, Block, Signal, Feedback, Route, Layout, CommandStation defined. REST routes exist for all except CommandStation |
| WebSocket | ⬜ | Removed. Nothing uses it; the core answers requests directly |
| Redis | ⬜ | Not used. The `docker-compose.yml` that started it is removed |
| Frontend state (Zustand) | 🟡 | Store exists. Only some panels read from it |
| Layout Editor | 🧪 | Drawing, placement, curved flex track with handles, joins, rotation, rename and delete, open-end markers, AnyRail import (matches the Python importer on a real file). Browser-tested with Playwright |
| Switchboard | 🟡 | Lists the first five blocks. No turnout or occupancy view yet |
| Route Manager | 🟡 | Routes can be created and edited. Run does nothing yet, because route execution is not built |
| Electron wrapper | ⬜ | Superseded by the Tauri desktop app. The `electron/` folder is an unused scaffold to remove |
| Rust tests | ✅ | 99 pass, 2 ignored (`cargo test` in `src-tauri/`), including consists, the web server, and the fake Z21 |
| Frontend tests | ✅ | 60 tests (`npm test`) |
| Mock Z21 for testing | ✅ | A fake Z21 on localhost in `src-tauri/src/tests/z21_link.rs` |

---

## Technology Stack

### Frontend
- **Framework**: React with Vite
  - Electron wrapper for desktop (planned)
  - Web dashboard accessible from other devices on the network
- **State Management**: Zustand (in use)
- **Styling**: Tailwind CSS (in use)
- **Canvas/Drawing**: Konva.js or Fabric.js for the layout editor (not yet used)

### Backend
- **Core**: Rust. It serves the desktop app in-process and the web UI over local HTTP (`trainlink-server`)
- **Database**: SQLite, one file per app
- **Real-time**: none. Clients ask for state; the core does not push
- **Cache**: none

### Hardware Communication
- **Command station (current)**: Roco Z21
- **Transport**: UDP, default port 21105, configurable host and port
- **Protocol**: Z21 LAN protocol (binary, length-prefixed messages). Locomotive control uses DCC addressing (1–9999), speed steps 0–126, with direction in bit 7
- **Future**: Z21 feedback messages (occupancy) and turnout commands. Both are planned, not built
- **Later, once Z21 is fully functional**: Märklin 6051/P50x, Lenz Intellibox, Digitrax LocoNet (see Phase 7). Roco/Fleischmann ECoS is also a candidate

### Architecture Pattern
- **Client-Server**: The core owns layout state. The desktop app calls it in-process, and browsers call it over local HTTP. The Electron scaffold is to be removed
- **Publish-Subscribe**: WebSocket events for live updates
- **Driver layer**: The Z21 driver is isolated in `z21/` so the rest of the core does not speak UDP directly. Additional command stations plug in through the same base driver interface (Phase 7)

---

## Implementation Phases

### Phase 1: Foundation (2-3 weeks)
**Goal**: Core architecture and basic UI

1. **Project Setup** ✅
   - Monorepo with `src-tauri/` (Rust core), `frontend/`, `electron/` (to be removed)
   - Docker Compose for PostgreSQL and Redis (now unused by the app; kept for the old data)
   - Database models defined in SQLite

2. **Backend Foundation** ✅
   - ✅ Rust core with the API, and a health check
   - ✅ WebSocket endpoint: dropped. It was removed because nothing used it
   - ✅ Database models: Locomotive, Block, Signal, Feedback, Route, Layout, CommandStation, and Consist
   - ✅ REST API: Z21, locomotives, blocks, signals, routes, feedbacks, layouts, track library, consists, command stations, and project overviews
   - ✅ State management: dropped. The core holds state in memory and SQLite, which is enough

3. **Frontend Foundation** ✅
   - ✅ Layout editor (drawing, placement, import, zoom, save and load)
   - ✅ Control page: locomotive roster and operate layout. Driven on hardware from the desktop app (see the desktop plan's checks)
   - ✅ Connection status display (Z21 heartbeat)
   - ✅ Settings UI (Z21 host and port), shared with the Control page

4. **Testing Infrastructure** ✅
   - ✅ Core tests (`src-tauri/`, 112 passing with `cargo test`, 2 ignored)
   - ✅ Frontend unit tests (`npm test`, 60 passing)
   - ✅ Integration tests: driver tests run against the mock. Browser tests (Playwright, `npm run test:e2e` in `frontend/`) run against the real core on a fresh data folder. A test session is written by `e2e/prepare-session.mjs`, so the tests do not contact the sign-in service
   - ✅ Fake Z21 on localhost for testing (`src-tauri/src/tests/z21_link.rs`)
   - ✅ Z21 frame encoding checked against the Z21 LAN Protocol Specification v1.13, and checked on hardware from the desktop app

### Phase 2: Core Features (3-4 weeks)
**Goal**: Fundamental train control on real hardware

1. **Locomotive Control** 🧪
   - ✅ Speed control (0–126 steps). Verified on hardware
   - ✅ Several locomotives at once: tick the roster to send one speed or direction to all of them
   - ✅ Direction (Forward / Reverse / Stop). Verified on hardware
   - 🧪 Function buttons (lights, sounds). 20 buttons with labels and momentary buttons. Not yet verified on hardware
   - 🟡 Travelled time is recorded and shown on the roster. Distance is not tracked yet. **Open:** distance needs each locomotive's speed steps converted to real speed, which needs a calibration step and a design decision
   - 🧪 Real-time speed feedback from Z21. Replies are parsed and shown on the roster. Not yet verified on hardware
   - ✅ Emergency stop. Verified on hardware (not yet confirmed separately)
   - 🧪 Track state from Z21 (short circuit, track power off), with locomotive commands blocked until ready and a Resume Track Power action. Reconnect in Settings

2. **Block System** 🟡
   - ✅ Block model and REST routes, with links to feedbacks, signals, and the next block (set with `feedback_ids`, `signal_ids` and `next_block_id` on a block)
   - ✅ Block definition and visualization: blocks are drawn on the layout canvas, and listed on the Switchboard
   - 🧪 Block occupancy detection via Z21 feedback. A block is occupied when any linked feedback reports it, and falls back to its own state when nothing has reported. Tested against the fake Z21. **Open:** check on a real Z21 with a detector on the track
   - ✅ Block linking to signals and feedbacks: a Blocks page under Layout sets the links, the next block, the name, the maximum speed and the state
   - ✅ Automatic signal management: a linked signal shows stop when a block it protects is occupied, caution when the block after is, and go otherwise
   - ✅ Block properties UI: the Blocks page
   - ✅ Live occupancy on the Switchboard, in the layout editor, and in the operate panel on the Control page. Refreshed every two seconds
   - 🧪 Block links sync between devices: the next block, feedbacks and signals travel with each block. Tested with the in-memory cloud. The migration `supabase/migrations/20261026100000_block_links.sql` is applied to the project. **Open:** check on two real devices

3. **Signal Control** ✅
   - ✅ Signal model and REST routes exist
   - ✅ Main signal aspects: stop, caution and go, from the blocks. Caution uses the next block set on the block, not the route order, so it does not wait for Phase 3
   - ✅ Warning signals: a warning signal shows caution when a block it protects is occupied, and go otherwise. It never shows stop
   - ✅ Automatic signal assignment from block state (signals that protect blocks follow them; manual aspects are refused while linked)
   - ✅ Signal visualization on layout: signals are drawn on the layout canvas, coloured by their live aspect
   - 🟡 Aspects are not yet sent to hardware. **Open:** the accessory commands for signal decoders, checked on a real signal

4. **Layout Drawing** 🧪
   - ✅ Layout REST routes (track segments stored in layout elements)
   - 🧪 Draw straight track segments (free drawing, kept alongside track pieces)
   - 🧪 Track library: 53 PECO N gauge entries, taken from PECO product pages, plus a custom curve whose radius is set per placement. Each piece records its source page and whether its figures are confirmed there
   - 🧪 Place Code 80 straights, curves, and turnouts (Setrack and Streamline Code 80 share one system). Ends snap to open ends and a layout uses one track system
   - 🧪 Flex track (Code 80 and Code 55) is placed as a straight of its catalogue length.
   - 🧪 SL-U395F/U396F: measured from the vector plan sheet (16.4mm toe, about 540mm diverging radius, 137mm long). The catalogue 457mm radius and 10 degree frog angle don't match the drawing.
   - 🧪 Other Streamline Code 55 medium, large, and small-radius turnouts still use the frog-angle model. They need the same check against their plan sheets before they're trusted
   - 🧪 Single slips SL-380F and SL-E380F: geometry measured from the PECO plan sheet (tracks 154.3mm and 154.2mm, crossing angle 9.9°, matching the listed 154mm and 10°)
   - 🧪 Curved double radius turnouts SL-386/387 (Code 80): traced from the PECO plan sheet. Arcs of 457mm and 914mm after an 8.5mm toe, both ending at 158.04mm
   - 🟡 SL-E399F 3-way turnout: branch radii and sweeps measured from the AnyRail layout's anchor positions. The branch departure points (47mm and 73mm) are still from the plan sheet, because the anchors fix where each branch ends but not where it leaves the straight. **TODO: check the departure points on a higher-resolution plan sheet or a physical measurement.**
   - 🧪 Import from AnyRail (JMRI layout-config XML): turnouts, straights, curves, slips and the three-way are placed from the file. Tested on one layout, which closes with no open joints
   - 🧪 Custom-length straights and curves: a placed piece carries its own length, radius, or angle
   - 🧪 Zoom in and out (25% to 400%), with Ctrl or Cmd and the wheel around the pointer
   - ⬜ Y turnouts (SL-397, SL-E397), ST-44/45 curved turnouts, and SL-E397F and ST-7 (no plan sheet): still unplaceable
   - 🧪 Crossings, curved turnouts and Code 55 curved turnouts: placeable from the crossing and curve models. Not all checked against drawings
   - 🧪 Place blocks, signals, feedbacks
   - 🧪 Save and load layouts (layout width and height are saved with the track)
   - 🧪 Drag-and-drop editing
   - 🧪 Snap-to-grid (20 unit grid)
   - ✅ Signals and feedbacks are linked to blocks, which drive stop, caution and go
   - 🟡 Signal aspects are not yet sent to hardware (see Signal Control)

### Phase 3: Automation & Routes (3-4 weeks)
**Goal**: Route-based automation

1. **Route Management** ✅
   - ✅ Route model and REST routes exist, with a sequence of blocks that cannot repeat a block and must stay on one layout
   - ✅ Route definition (sequence of blocks): a wizard on the Route Manager, with blocks in order (move up, move down, remove), then options, then review
   - ✅ Steps that do not follow a block's next-block link are flagged on the Route Manager and in the route's `unlinked_steps`. Not required before a route is saved
   - ✅ Route creation wizard: blocks, options, and review. The review draws the route on the layout with its blocks outlined
   - ✅ Advanced route options: a speed for each block (blank for the block's maximum), a wait in seconds at each block, and auto-return. Checked on the server: one entry per block, speed 0 to 126, wait 0 to 3600 seconds
   - ✅ Route visualization: the steps in order with each block's live state, and the route outlined on the layout
   - 🧪 Run and Stop on the Route Manager, through automatic driving (below). Tested with the fake Z21. **Open:** check on a real layout

2. **Automatic Driving** 🧪
   - 🧪 Simple automatic mode (follow route): a run starts a route with a locomotive in its first block, and moves on as the train enters each block. Tested with the fake Z21
   - 🧪 Speed management: each block's speed from the route's speed profile, or the block's maximum speed
   - 🧪 Stop at destinations: a run stops at the last block of its route, or turns round with auto-return
   - 🧪 Route completion handling: a route that completes is no longer running. A run that was not started in this session is stopped at start-up
   - 🧪 Waiting for a busy block: a run holds at speed 0 until the block ahead is free, and the Route Manager shows what it waits for. No configurable time yet
   - **Open:** check on a real layout. Collision avoidance (item 3) must come before runs on a shared layout

3. **Collision Avoidance** 🧪
   - ✅ Block occupancy tracking: a block is occupied when a linked feedback reports it, or by its own state where nothing has reported
   - 🧪 Train detection via Z21 feedback: the feedback sets a block's occupancy, which runs and signals read. Tested with the fake Z21. **Open:** check on a real detector
   - 🧪 Automatic signal management to prevent collisions: a run never enters an occupied block, and signals show stop for an occupied block, so both agree. Caution comes from the next block
   - 🧪 Deadlock prevention: a run holds the block it is in and claims the block ahead before moving. A cycle of runs waiting on each other is broken by stopping the newer run, with a notice. Tested with the fake Z21
   - 🧪 A block that is occupied but not claimed by a run (a train no run expects) holds the runs that want it, and raises an alarm on the Route Manager until it is free. An alarm can be dismissed, but the hold stays. Tested with the fake Z21. **Open:** check on a real layout
   - **Open:** check on a real layout with two runs sharing a block

4. **Turnouts** 🧪
   - 🧪 Z21 turnout commands (LAN_X_SET_TURNOUT, queued, activate then deactivate), frames checked against the spec. Not yet tried on a turnout
   - 🧪 Turnout state from Z21 replies (LAN_X_TURNOUT_INFO), polled while operating
   - 🟡 DCC address and straight output per placed turnout. Turnout control is on the Control page (Operate panel), not the Switchboard. Only plain turnouts take an address

5. **Feedback / Sensor Integration** 🟡
   - 🟡 Z21 R-BUS feedback parsed and stored, read through `GET /api/z21/feedback`. Not yet verified on hardware
   - 🟡 Feedback model and REST routes exist
   - ⬜ Multiple feedbacks per block
   - ⬜ Occupancy detection logic and debouncing
   - ⬜ Sensor feedback UI in layout editor

### Phase 4: Multi-Client Support (2-3 weeks)
**Goal**: Network-enabled control

1. **Server Architecture** 🟡
   - 🟡 Connection manager exists
   - ⬜ Centralized state management
   - ⬜ Real-time synchronization protocol
   - ⬜ Session management
   - ⬜ User permissions and roles

2. **Client Types** ⬜
   - ⬜ Main control client (full features)
   - ⬜ Remote overview clients (read-only)
   - ⬜ Dedicated controllers (single block or signal)
   - ⬜ Mobile web interface

3. **Network Reliability** ⬜
   - ⬜ Reconnection handling (frontend hook exists, not verified)
   - ⬜ Offline queue
   - ⬜ Conflict resolution
   - ⬜ Z21 reconnection and failover

4. **Synchronization** ⬜
   - ⬜ Real-time state sync via WebSocket
   - ⬜ Command acknowledgment
   - ⬜ Undo and redo across the network

### Phase 5: Z21 Integration Hardening (2-3 weeks)
**Goal**: Reliable, verified Z21 support

1. **Serial/UDP Communication** 🟡
   - ✅ UDP transport with async I/O
   - ✅ Connection status and heartbeat
   - 🟡 Incoming parsing: status, system state, R-BUS feedback, turnout info, and locomotive info. Tested against the fake Z21; hardware not yet checked
   - 🧪 Error handling and recovery: the app sends a status request every 10 seconds, because the spec drops a client that is silent for a minute. A link with no frame for 30 seconds is lost: commands are refused, and a watchdog reconnects to the same Z21 once a second while it is lost. A socket error no longer stops the reader. **Open:** check on hardware. The app cannot stop trains while the link is down, so a drop during a run leaves them running until it reconnects

2. **Z21 Feature Support** ⬜
   - 🟡 Query Z21 system state (version not queried)
   - 🧪 Query locomotive info (speed, direction, functions). Replies are parsed. Not yet verified on hardware
   - 🟡 Track power on, with resume after a short circuit. Track power off not built
   - ⬜ Feature detection

3. **Locomotive Decoder Management** 🟡
   - ⬜ Import locomotive definitions
   - 🧪 Store decoder profiles: the CVs (number, name, range, read-only) and function names. Imported from JSON, and listed, read and deleted through the API. The NMRA standard profile is built in and cannot be deleted. A locomotive can choose a profile (`decoder_profile_id`)
   - 🧪 DCC address assignment: addresses are 1-9999 and unique per command station (locomotives with no station share one set). "Next free" on the locomotive form gives the lowest unused one for that station. Reading the address from a decoder is not built. Sync carries each locomotive's command station by name. Each device resolves the name to its own station of that name, and a device with no such station keeps the name on the locomotive, sends it back unchanged, and attaches the locomotive when it gets a station of that name. Station records are not synced, because a station's port is specific to its network. The Supabase column `locomotives.command_station` needs `supabase/migrations/20261027100000_locomotive_command_station.sql` applied first. A locomotive from the cloud that clashes with one on this device by address on its station, or by name, is skipped and listed in the sync result (`skipped`). The rest of the sync goes through. The sync control in the top bar shows the count, and the reasons on click. **Open:** address lookups for moving time and the Z21 commands still match on address alone
   - 🧪 Function mapping: linking a decoder profile names the function buttons it defines. Labels a user has already changed are kept

### Phase 6: Polish & Export (2 weeks)
**Goal**: Production-ready application for Z21

1. **Performance Optimization** ⬜
   - ⬜ Frontend rendering optimization
   - ⬜ Backend query optimization
   - ⬜ Network message batching
   - ⬜ Memory profiling

2. **User Experience** ⬜
   - ⬜ Keyboard shortcuts
   - ⬜ Undo and redo
   - ⬜ User preferences
   - ⬜ Help and tutorial

3. **Reliability** ⬜
   - ⬜ Comprehensive error handling
   - ⬜ Logging system (basic logging configured)
   - ⬜ Crash recovery
   - ⬜ Data backup and restore

4. **Packaging** ⬜
   - 🟡 Electron scaffold exists
   - ⬜ Electron builds for Windows, macOS, Linux
   - ⬜ Auto-update
   - ⬜ Installation wizard
   - ⬜ Documentation

### Phase 7: Additional Command Station Drivers (2-3 weeks per driver)
**Goal**: Extend TrainLink beyond Z21. **Starts only after Phase 6 is complete and Z21 is verified on real hardware.**

Entry criteria:
- [ ] Z21 locomotive, turnout, and feedback features verified on real hardware
- [ ] Base driver interface extracted from the Z21 driver and covered by tests
- [ ] Mock command station pattern in place, reusable for each new protocol

1. **Base Driver Interface** ⬜
   - ⬜ Extract `base_driver.py` from the Z21 driver (connect, disconnect, speed, direction, functions, feedback events)
   - ⬜ Feature negotiation: each driver declares what it supports
   - ⬜ Driver registry and selection from CommandStation config

2. **Märklin 6051 / P50x** ⬜
   - ⬜ Serial communication (PySerial)
   - ⬜ Protocol parsing and error recovery
   - ⬜ Mock P50x for tests
   - ⬜ Verified on real hardware

3. **Lenz Intellibox** ⬜
   - ⬜ Serial communication (PySerial)
   - ⬜ XpressNet protocol parsing
   - ⬜ Mock Intellibox for tests
   - ⬜ Verified on real hardware

4. **Digitrax LocoNet** ⬜
   - ⬜ Serial communication (PySerial)
   - ⬜ LocoNet message parsing and slot handling
   - ⬜ Mock LocoNet for tests
   - ⬜ Verified on real hardware

5. **Roco / Fleischmann ECoS** ⬜ (candidate, scope to be confirmed)
   - ⬜ Protocol research and driver design
   - ⬜ Mock ECoS for tests
   - ⬜ Verified on real hardware

6. **Auto-Detection and Feature Negotiation** ⬜
   - ⬜ Auto-detect command station type
   - ⬜ Query available features
   - ⬜ Configure based on capabilities

### Phase 8: Driver Tools (2-3 weeks)
**Goal**: Drive more than one locomotive at a time, and use more than the keyboard and mouse

1. **Consists** 🟡
   - 🧪 Link locomotives into a consist with a lead locomotive, which drives all of them. Built in PR #45. Not yet run on a Z21
   - ⬜ Check the Z21 spec v1.13 for consist commands. The build sends speed and direction to each member, since the spec has not been checked
   - 🧪 Consists saved on each device, with a direction per member. Not synced between devices yet
   - 🧪 Functions sent to the whole consist. Functions chosen per member: ⬜

2. **Multi-Throttle** ⬜
   - ⬜ Several locomotives on screen at once, each with its own speed, direction, and functions
   - ⬜ Builds on the roster's group control

3. **Controller Support** ⬜
   - ⬜ USB and Bluetooth game controllers and throttles
   - ⬜ Map speed, direction, functions, and emergency stop to buttons and axes
   - ⬜ Emergency stop cannot be mapped to anything that could be pressed by accident

4. **Cab View and Cab Ride** ⬜ (deferred)
   - ⬜ Needs a camera or video feed source chosen first. Not planned in detail until then

### Phase 9: Decoder Programming (3-4 weeks)
**Goal**: Read and write decoder settings (CVs) on the programming track
**Decoder profiles are stored (Phase 5, item 3).** The CV browser shows the CVs of the chosen profile, which starts as the NMRA standard, or the locomotive's own profile. Writing a wrong CV can stop a decoder working, so every write is confirmed, limited to the profile's CVs and ranges, and checked where the Z21 allows it.

1. **Programming Track** 🧪
   - 🧪 Direct-mode CV read and write on the programming track (spec v1.13, sections 6.1 to 6.5). The frames are checked against the spec's bytes in tests. **Open:** check on a real decoder
   - 🧪 POM write of a CV and of one bit on the main track, for a locomotive on the roster (spec 6.6 and 6.7). The Z21 gives no answer, so these cannot be checked
   - 🧪 POM read on the main track (spec 6.8), for a locomotive on the roster. It needs RailCom in the Z21 and the decoder, so the answer may never come. **Open:** check on a real decoder
   - 🧪 Only one operation runs at a time, and an operation with no answer from the Z21 in 30 seconds is reported as failed

2. **CV Browser** 🟡
   - 🧪 CVs with names and ranges from the chosen decoder profile. Profiles can be imported from JSON, and a locomotive can use one. Functions are listed too. **Open:** the JSON format for importing a profile is not yet documented for users
   - 🧪 Show the value read from the decoder, and write a new one

3. **Safety** ✅
   - ✅ Confirmation before every write: the write is refused unless `confirm` is set, and the page needs the box ticked
   - ✅ Values and CV numbers outside their range are refused (0 to 255, CV 1 to 1024, bit 0 to 7)
   - ✅ Direct-mode writes are read back and any mismatch is reported. POM writes cannot be read back, and the page says so
   - 🧪 With a profile, a write is refused for a CV that is not in it, a read-only CV, or a value outside the CV's range
   - 🧪 A write needs a ready track. Tested with the fake Z21

### Phase 10: LocoLink Parity (Backlog)
**Goal**: Cover the features seen in LocoLink 2.0.9, built from our own design. Nothing is copied from LocoLink's code, text, icons or images. The full comparison, with what was seen on screen, is in `~/Documents/TrainLink/locolink-vs-trainlink.md`. Not started. Items are grouped by the phase they most depend on, and the order is still to be decided.

**10a. Z21 basics (no new hardware)**
1. **Turnouts** ⬜
   - ⬜ Standalone turnout list with search, filters for straight and diverging, and a "no address" warning
   - ⬜ Turnout types: straight, left, right, three-way, Y, curved left and right, double-slip, single-slip, crossing
   - ⬜ Manual turnout (no motor) flag, and a per-turnout switching time (default 300 ms, replacing the fixed 150 ms)
   - ⬜ Per-turnout group, command station and notes
   - ⬜ Position feedback toggle per turnout
2. **Locomotives** ⬜
   - ⬜ Decoder type (DCC 128 and others) and command station per locomotive
   - ⬜ Locomotive types: steam, diesel, electric, multiple unit, tram, shunt, museum and other
   - ⬜ Group, maximum speed, reverse direction, and a colour
   - ⬜ 29 function buttons, replacing 20
   - ⬜ Photos up to 16 MB, replacing 2 MB
   - ⬜ Locomotive import from another app
3. **Import review** ⬜
   - ⬜ After an AnyRail import, a list of items to check later: turnouts without an address, turnout symbols without a link, and blocks without a sensor
4. **Setup checklist** ⬜
   - ⬜ Guided steps: command station, locomotives, first manual run, track plan, blocks and sensors, turnouts, and routes

**10b. Blocks and sensors (needs Phase 2)**
5. **Blocks** ⬜
   - ⬜ Blocks page with filters for all, occupied, reserved, free and problems
   - ⬜ Block properties: group, length, max speed, borders on, sensors, signals, brake and stop positions
   - ⬜ Link entry, middle and exit sensors, and signal addresses
6. **Feedback monitor** ⬜
   - ⬜ 128 sensor addresses in 8 modules, with known, occupied, free and not-linked counts
   - ⬜ Read now, and a list of the latest reports
7. **Diagnostics** ⬜
   - ⬜ Project and layout diagnostics: blocks without sensors, turnouts without addresses, with a "check again" action
   - ⬜ A health score and a category view (command stations, locomotives, blocks, routes, cameras, recovery)
   - ⬜ Connection log and a copyable support package
8. **Accessory keyboard** ⬜
   - ⬜ Grid of accessory addresses, with two outputs each, showing what uses each address
   - ⬜ Range, grid size, "only in use" filter, and a safe test mode that locks empty addresses

**10c. Automation (needs 10b)**
9. **Routes** ⬜
   - ⬜ Routes built from blocks and turnout steps, with an icon, colour and a route button
   - ⬜ Build from the track plan, and a validation overview
10. **Automatic driving** ⬜
    - ⬜ Autopilot with pause and stop all, routes and rides
    - ⬜ Choose blocks, assign a locomotive, and start the ride
    - ⬜ Safety checks: ghost trains, stuck rides and non-arrival (configurable times), and an emergency stop that can switch power off
    - ⬜ Wait for a busy block, with a configurable time
11. **Stations** ⬜
    - ⬜ A station as a group of blocks, with free and occupied counts
12. **Timetables** ⬜
    - ⬜ Operating session with a model clock and a clock factor from 0.5x to 120x
    - ⬜ Timetables with running days, notes and trip entries (model time, train and action)
13. **Dispatch and warrants** ⬜
    - ⬜ Warrants with priority, locomotive, from and to blocks, stop at the end, shunting allowed, and a dispatcher and notes
    - ⬜ Open and under-way counts. A warrant without a locomotive and end block is a note only, and is not checked
14. **Trip logs and maintenance** ⬜
    - ⬜ Trip log with rides, time and distance, and a day-by-day view
    - ⬜ Maintenance records with a service interval (default 50 hours) and alerts

**10d. Decoder programming (needs Phase 9)**
15. **Wizards** ⬜
    - ⬜ Locomotive wizard: place on the programming track, read the address, choose short or long, write, read back
    - ⬜ Turnout and signal wizards: learning-mode steps, with the send button enabled only after the learning mode is confirmed
16. **Decoder CVs** ⬜
    - ⬜ CV editor with known CVs, ranges, read-only CVs, and read and write on the programming track and the main track
    - ⬜ Decoder workshop: backups, speed tables and motor settings

**10e. Extras (may need hardware or library decisions)**
17. **3D workbench** ⬜ (needs a rendering choice)
    - ⬜ 3D view of the track plan, with saved views and layers
    - ⬜ Click a turnout in 3D to operate it, and highlight the active route
18. **Cameras** ⬜ (needs a camera source choice)
    - ⬜ Camera monitoring with camera areas, and block occupancy from camera images
19. **Controllers** ⬜ (needs a controller library choice)
    - ⬜ USB and wireless game controllers, with mapping of speed, direction and functions
    - ⬜ Emergency stop cannot be mapped to a button that could be pressed by accident
20. **Sound** ⬜
    - ⬜ Per-locomotive sounds, app sounds, and muting
21. **Wagons and freight** ⬜
    - ⬜ Wagon model with class, article number, era, livery, length, weight, decoder and functions
    - ⬜ Consists that mix locomotives and wagons, and a freight and switching area
22. **Other command stations** ⬜ (each needs a driver, per Phase 7)
    - ⬜ ECoS, CS2/CS3 and DCC-EX
23. **Web panel and webhooks** ⬜
    - ⬜ A shareable web panel and webhook addresses, with router settings
24. **Backups** ⬜
    - ⬜ Automatic backups before updates, a manual backup, and restore with a list of versions
25. **Settings sweep** ⬜
    - ⬜ Display mode, accent colour, picture style, interface size, language, and sidebar customisation
    - ⬜ Command station management, auto-connect at start-up, and a Wi-Fi setup assistant
    - ⬜ Experience, systems, data and demo settings: not yet reviewed in LocoLink
26. **Multi-language** ⬜
    - ⬜ Interface translation (LocoLink ships English, Dutch, German, French and others)
27. **Recovery** ⬜
    - ⬜ Recovery snapshot after an interruption, with a decision per item to keep occupied or release
28. **Analog mode (DC)** ⬜ (needs a DCC-EX driver, per item 22)
    - ⬜ Run DC locomotives on a separate track section, through a DCC-EX command station. The Z21 cannot do this
    - ⬜ Set a DCC-EX output to DC for the analog section, and link a DC profile to make it drivable
    - ⬜ Switch track power off before every change between DCC and analog mode
    - ⬜ TrackManager, profiles, replay and shuttle, and precision settings: not yet reviewed in detail
29. **Multi-throttle** ⬜ (builds on Phase 8)
    - ⬜ One throttle card per locomotive, each with its own speed slider, direction and functions
    - ⬜ Speed presets at 0%, 25%, 50%, 75% and 100%, and a "stop this locomotive" button
    - ⬜ Quick function buttons (light, sound, whistle, F3, pinnable), with a link to the full function list
30. **Signals** ⬜
    - ⬜ Signal types: German main (Hp), German distant (Vr), shunt (Sh), combination (Ks), and national types for the Netherlands, Austria, plus custom and general
    - ⬜ Feed by signal type, or one address for red and green, with a swap option
    - ⬜ Aspect table that maps each aspect to its addresses, for example Hp0 stop and Hp1 clear
    - ⬜ Fictitious signals that are not wired to hardware
31. **Multi-unit consists** 🟡 (replaces the software-only consist in Phase 8 once CV19 programming exists)
    - 🧪 A consist has its own consist address (1 to 127), stored and checked so no two consists share one. Built on branch `consist-address`, not yet pushed
    - ⬜ Each locomotive's CV19 is set to the consist address, so the decoders run together. Needs Phase 9 CV programming
    - 🧪 A minimum of two locomotives per consist, with a reversed flag for each. Existing one-locomotive consists keep working but cannot be edited down to one
    - 🧪 A Consists page in the side menu, under Drive, for making and driving consists, with one speed slider for the whole consist. Not yet in a separate window that the layout remembers
    - ⬜ A consist address that routes and automation can refer to (Phase 3)
32. **Trains** ⬜ (builds on item 21)
    - ⬜ A train is one locomotive plus an ordered list of wagons, with a name, group, weight class and train type
    - ⬜ Train length and weight are worked out from the wagons, for block and automation checks
    - ⬜ A "main train of this loco" flag, for locomotives with several trains

**Not yet reviewed in LocoLink** (add when seen): Macros, Shuttles, Drivers, Freight & switching, Command Truth, Projects' Save as and Export, and the Experience, Systems, Data and Demo settings.

### Phase 10f: Design Direction (from the LocoLink review)
**Goal**: Change how TrainLink is organised and presented, so that a new user sees what to do next and an experienced user can find things quickly. The pages stay in one codebase across platforms.

**Done**
1. ✅ **Project health check.** `GET /api/overview/health` lists blocks without a sensor and turnouts without a DCC address, with counts. The Health page under System shows them, with buttons to the Switchboard and the Layout Editor
2. ✅ **Turnout list.** `GET /api/overview/turnouts` lists every turnout on every layout. The Turnouts page under Layout has filters (all, with address, no address), search, and the layout each turnout is on
3. ✅ **Consists as their own page** under Drive (done earlier, branch `consist-address`)

**Not yet done**
4. ⬜ **Basic and Advanced sidebar.** Group the pages into Basics and Advanced, with the rest collapsed, as LocoLink does
5. ⬜ **Guided setup checklist.** A Setup page with numbered steps (command station, locomotives, track plan, blocks and sensors, turnouts, routes), each showing done or to do
6. ⬜ **Summary cards and filters** on the Locomotives and Blocks pages, as on the Turnouts page
7. ⬜ **Item editing in one form with a live preview**, for new items first (turnouts, blocks, locomotives), then converting the existing inline forms
8. ⬜ **Import review list** after an AnyRail import: turnouts without an address, turnouts without a link, and blocks without a sensor (see Phase 10a item 3)
9. ⬜ **Empty states with a next action** on every page, replacing plain text

---

## Immediate Priorities

1. ✅ **Test locomotive control against a real Z21.** Done: a train ran under TrainLink control (user test). Still to confirm on hardware: short-circuit detection, track power resume, and feedback. Connect, set speed and direction, emergency stop, and confirm the status heartbeat. Record results in `IMPLEMENTATION_SUMMARY.md`. Frame bytes have been checked against the Z21 LAN Protocol Specification v1.13.
2. ✅ Mock Z21 UDP responder (done)
3. ✅ Unit tests for the Z21 packet builder and the speed and direction logic (done; now in the Rust core)
4. ✅ REST routes for locomotives, blocks, signals, routes, feedbacks, layouts, and the track library. Command stations still need routes.
5. 🟡 Z21 R-BUS feedback parsing and `GET /api/z21/feedback` done. Still to do: link feedback inputs to blocks, and verify on hardware. **Next.**
6. ✅ DCC turnout control: turnouts take a DCC address, and the Operate panel switches them. Spec v1.13 commands. Not yet tried on hardware
7. ✅ Control page: locomotive roster with multi-select, the layout in operate mode, and the Z21 connection at the top. Browser-tested with the Z21 stubbed

---

## Core Data Models

```
Locomotive
├── id, address, name, type
├── speed, direction, functions
├── travelled_time, travelled_distance
└── decoder_profile_id

Block
├── id, name, position
├── signal_ids (main, warning)
├── feedback_ids (occupancy detection)
├── max_speed, properties
└── current_state (free, occupied, reserved)

Signal
├── id, block_id, type (main/warning)
├── aspects (Stop, Caution, Go)
└── current_aspect

Route
├── id, name, block_sequence
├── speed_profile, dwell_times
├── auto_return, enabled
└── execution_state

Layout
├── id, name, width, height
├── scale, track_layout
├── elements (blocks, signals, feedbacks, text, platforms)
└── saved_state

CommandStation
├── id, type (z21), host, port, settings
├── connected, firmware_version
└── supported_features

Feedback
├── id, block_id, z21 address
└── state (clear, occupied)
```

---

## Critical Components

### Core Modules
1. **Layout Manager**: Load, save, validate layouts. 🧪 (REST routes, track validation, AnyRail import, rename and delete)
2. **Block Controller**: Track occupancy, manage blocks. 🟡 (blocks stored and set by hand; no occupancy from the Z21 yet)
3. **Signal Controller**: Manage signal states from block state. 🟡 (signals stored and set by hand)
4. **Route Engine**: Execute routes, handle automation. ⬜
5. **Locomotive Manager**: Control trains, track state. 🧪 (Z21 speed and direction, functions, roster, multi-select)
6. **Z21 Driver**: Abstract the Z21 UDP link. 🧪
7. **Web Server**: Serves the API and the web UI on one address. 🧪
8. **State Synchronizer**: Keep state consistent across devices. ⬜ (planned: Supabase)

### Frontend Components
1. **Layout Editor**: Draw and configure the layout. 🧪 (drawing, placement, AnyRail import, zoom, save/load; browser-tested with Playwright)
2. **Switchboard View**: Visual representation of the layout. 🟡 (placeholder)
3. **Control page**: Locomotive roster, operate layout, and Z21 connection. 🧪
4. **Route Manager**: Create and manage routes. 🟡 (placeholder)
5. **Block Monitor**: View block states. ⬜
6. **System Settings**: Z21 connection settings, shared with the Control page. ✅
7. **Network Status**: Show connected clients. ⬜
8. **Event Log**: Display system events. ⬜

---

## File Structure

Current layout of the repository. Items marked ⬜ are planned.

```
trainlink/
├── src-tauri/                            # Rust core and desktop shell
│   ├── src/
│   │   ├── lib.rs                        # ✅ Desktop app, and the trainlink:// protocol
│   │   ├── bin/server.rs                 # ✅ trainlink-server: API and web UI on one address
│   │   ├── server.rs                     # ✅ HTTP server, request limits, static files
│   │   ├── api/                          # ✅ REST routes, with the validation rules
│   │   │   ├── layouts.rs, locomotives.rs, trackside.rs, library.rs, z21.rs, multipart.rs
│   │   ├── db/                           # ✅ SQLite tables and queries
│   │   ├── z21/                          # 🧪 Frames, commands, replies, connection
│   │   ├── anyrail/                      # 🧪 AnyRail (JMRI XML) import and geometry
│   │   ├── library.rs                    # ✅ Track library, loaded from library/*.json
│   │   ├── moving_time.rs                # 🧪 Time each locomotive moves
│   │   ├── state.rs, fields.rs, http.rs  # ✅ Shared state, request bodies, replies
│   │   └── tests/                        # ✅ 52 tests, with fixtures from the old Python importer
│   ├── library/                          # ✅ PECO N gauge and custom curve
│   ├── Cargo.toml
│   └── tauri.conf.json
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   │   ├── shell/                    # 🧪 Top bar, side navigation, page list
│   │   │   ├── control/                  # 🧪 Control page: roster, operate layout, function panel
│   │   │   ├── layout/                   # 🧪 Canvas, toolbar, properties, track picker, AnyRail import
│   │   │   ├── locomotives/              # 🧪 Roster setup and pictures
│   │   │   ├── LayoutEditor.jsx          # 🧪
│   │   │   ├── SystemSettings.jsx        # ✅
│   │   │   ├── Switchboard.jsx           # 🟡
│   │   │   └── RouteManager.jsx          # 🟡
│   │   ├── hooks/                        # 🧪 Z21 connection, status, turnouts, layout editor
│   │   ├── layout/                       # 🧪 Track geometry and layout model
│   │   ├── services/                     # 🧪 api.js (base address), layoutApi.js, locomotiveApi.js
│   │   ├── store/                        # 🟡 Zustand
│   │   └── App.jsx
│   └── package.json
├── electron/                             # ⬜ Unused scaffold, to be removed
├── docs/
│   └── desktop-plan.md                   # ✅ Desktop, sync, and mobile plan
├── docker-compose.yml                    # ✅ PostgreSQL and Redis for the old data; the app does not use them
├── README.md
├── QUICKSTART.md
├── IMPLEMENTATION_SUMMARY.md
└── iTrain_Implementation_Plan.md
```

---

## Key Challenges & Mitigation

| Challenge | Impact | Mitigation |
|-----------|--------|-----------|
| **Z21 Protocol Accuracy** | High | Test against real hardware early. Build a mock Z21 for CI |
| **Real-time Synchronization** | High | WebSocket plus Redis for state. Event-based updates |
| **Feedback Timing** | Medium | Debounce feedback. Handle false occupancy readings |
| **Layout Scalability** | Medium | Virtualize rendering for large layouts |
| **Multi-client Conflicts** | Medium | Conflict resolution and a command queue |
| **Cross-platform Compatibility** | Low | Electron handles most of it. Test on all platforms early |

---

## Development Timeline

| Phase | Duration | Status | Key Deliverable |
|-------|----------|--------|-----------------|
| Foundation | 2-3 weeks | 🟡 Mostly done | Working WebSocket server and basic UI |
| Core Features | 3-4 weeks | 🟡 Loco control, layout editor, turnout control | Full locomotive and block control |
| Automation | 3-4 weeks | ⬜ | Route-based automation working |
| Multi-Client | 2-3 weeks | ⬜ | Network support functional |
| Z21 Hardening | 2-3 weeks | ⬜ | Verified Z21 integration |
| Polish | 2 weeks | ⬜ | Production-ready executable |
| Additional Drivers | 2-3 weeks each | ⬜ (after Z21 complete) | Märklin, Intellibox, LocoNet, ECoS |
| **Total** | **~16-21 weeks for Z21, plus ~8-12 weeks for drivers** | | Full TrainLink |

---

## Getting Started Checklist

- [x] Choose backend framework (FastAPI; later replaced by the Rust core)
- [x] Set up project repository with monorepo structure
- [x] Design database schema (now SQLite)
- [ ] Create basic Electron app shell (scaffold only)
- [x] Implement WebSocket server (echo only; later removed)
- [x] Build layout editor component (placeholder)
- [x] Create locomotive control UI
- [x] Test locomotive control against real Z21
- [x] Build mock Z21 for testing
- [ ] Build comprehensive test suite
- [ ] Document API and component interfaces
- [ ] Phase 7: Extract base driver interface (after Z21 complete)

---

## Resources & References

### Roco Z21
- Z21 LAN Protocol documentation (Roco / Fleischmann)
- Z21 default address: UDP port 21105

### Libraries
- **Rust**: Tauri 2, rusqlite (SQLite), roxmltree (AnyRail XML)
- **Frontend**: React, Vite, Zustand, Tailwind CSS, react-konva (layout canvas)
- **Electron**: electron-builder or electron-forge (packaging)

---

## Success Criteria

✅ **MVP Complete when:**
- [x] Z21 connection verified on real hardware
- [x] Locomotive speed and direction control verified on real hardware
- [ ] Emergency stop verified on real hardware
- [x] Layout editor functional
- [ ] Block occupancy detection functional
- [ ] Basic signal automation in place
- [ ] Multiple clients can connect and sync

✅ **Feature Complete (Z21) when:**
- [ ] Z21 turnout and track power control working (turnouts 🧪, not yet tried on hardware)
- [ ] Routes and automation fully working
- [ ] Multi-client support stable
- [ ] Export and import functionality working
- [ ] Performance acceptable for large layouts (100+ blocks)

✅ **Multi-Command-Station Complete when:**
- [ ] Feature Complete (Z21) criteria met
- [ ] At least one additional driver (Phase 7) verified on real hardware
- [ ] All planned drivers (Märklin P50x, Intellibox, LocoNet) verified on real hardware
