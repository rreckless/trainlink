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

2. **Backend Foundation** 🟡
   - ✅ Rust core with the API, and a health check
   - ⬜ WebSocket endpoint (removed; nothing used it)
   - 🟡 Database models: Locomotive, Block, Signal, Feedback, Route, Layout, CommandStation. REST routes exist for Locomotive, Block, Signal, and Route only
   - 🟡 REST API: Z21, locomotives, blocks, signals, routes, feedbacks, layouts, track library. Command stations still need routes
   - ⬜ State management (not needed: the core holds state in memory and SQLite)

3. **Frontend Foundation** 🟡
   - ✅ Layout editor (drawing, placement, import, zoom, save and load)
   - 🧪 Control page: locomotive roster and operate layout
   - ✅ Connection status display (Z21 heartbeat)
   - ✅ Settings UI (Z21 host and port), shared with the Control page

4. **Testing Infrastructure** 🟡
   - ✅ Core tests (`src-tauri/`, 52 passing with `cargo test`)
   - ✅ Frontend unit tests (`npm test`, 42 passing)
   - 🟡 Integration tests (driver tests run against the mock; browser tests run from a scratch Playwright script, not yet in the repo)
   - ✅ Fake Z21 on localhost for testing (`src-tauri/src/tests/z21_link.rs`)
   - ✅ Z21 frame encoding checked against the Z21 LAN Protocol Specification v1.13. Real hardware test still pending

### Phase 2: Core Features (3-4 weeks)
**Goal**: Fundamental train control on real hardware

1. **Locomotive Control** 🧪
   - ✅ Speed control (0–126 steps). Verified on hardware
   - ✅ Several locomotives at once: tick the roster to send one speed or direction to all of them
   - ✅ Direction (Forward / Reverse / Stop). Verified on hardware
   - 🧪 Function buttons (lights, sounds). 20 buttons with labels and momentary buttons. Not yet verified on hardware
   - 🟡 Travelled time is recorded and shown on the roster. Distance is not tracked yet
   - 🧪 Real-time speed feedback from Z21. Replies are parsed and shown on the roster. Not yet verified on hardware
   - ✅ Emergency stop. Verified on hardware (not yet confirmed separately)
   - 🧪 Track state from Z21 (short circuit, track power off), with locomotive commands blocked until ready and a Resume Track Power action. Reconnect in Settings

2. **Block System** ⬜
   - 🟡 Block model and REST routes exist
   - ⬜ Block definition and visualization
   - ⬜ Block occupancy detection via Z21 feedback
   - ⬜ Block linking to signals
   - ⬜ Automatic signal management
   - ⬜ Block properties UI
   - ⬜ Live occupancy on the Switchboard and in the layout editor. The Z21 feedback is read, but no screen shows it yet

3. **Signal Control** ⬜
   - 🟡 Signal model and REST routes exist
   - ⬜ Main signal aspects (Stop, Caution, Go)
   - ⬜ Warning signals
   - ⬜ Automatic signal assignment from block state
   - ⬜ Signal visualization on layout

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
   - ⬜ Signals and feedbacks are placed but have no link to automation or hardware yet

### Phase 3: Automation & Routes (3-4 weeks)
**Goal**: Route-based automation

1. **Route Management** ⬜
   - 🟡 Route model and REST routes exist, Route Manager UI is a placeholder
   - ⬜ Route definition (sequence of blocks)
   - ⬜ Route creation UI with wizard
   - ⬜ Advanced route options (speed limits, dwell times)
   - ⬜ Route visualization
   - ⬜ Run and Stop on the Route Manager. The Run button does nothing yet, and the backend refuses to run a route until automatic driving is built

2. **Automatic Driving** ⬜
   - ⬜ Simple automatic mode (follow route)
   - ⬜ Speed management
   - ⬜ Stop at destinations
   - ⬜ Route completion handling

3. **Collision Avoidance** ⬜
   - ⬜ Block occupancy tracking
   - ⬜ Train detection via Z21 feedback
   - ⬜ Automatic signal management to prevent collisions
   - ⬜ Deadlock prevention

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
   - 🟡 Incoming parsing: status, system state, R-BUS feedback, turnout info. Locomotive info replies not parsed
   - ⬜ Error handling and recovery

2. **Z21 Feature Support** ⬜
   - 🟡 Query Z21 system state (version not queried)
   - 🧪 Query locomotive info (speed, direction, functions). Replies are parsed. Not yet verified on hardware
   - 🟡 Track power on, with resume after a short circuit. Track power off not built
   - ⬜ Feature detection

3. **Locomotive Decoder Management** ⬜
   - ⬜ Import locomotive definitions
   - ⬜ Store locomotive profiles
   - ⬜ DCC address assignment
   - ⬜ Function mapping

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
**Needs Phase 5 decoder profiles first.** Writing a wrong CV can stop a decoder working, so every write is checked.

1. **Programming Track** ⬜
   - ⬜ Read and write CVs with the Z21 programming commands. Check the command codes in the spec v1.13 before implementing
   - ⬜ Direct mode on the programming track, and POM mode on the main track if the spec and decoder allow it

2. **CV Browser** ⬜
   - ⬜ List CVs with names and ranges from the decoder profile
   - ⬜ Show the current value, and edit it

3. **Safety** ⬜
   - ⬜ Confirmation before every write
   - ⬜ Refuse values outside the CV's range
   - ⬜ Read the value back after each write and report any mismatch

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

**Not yet reviewed in LocoLink** (add when seen): Macros, Shuttles, Drivers, Freight & switching, the Consists tab on Wagons & trains, Command Truth, Projects' Save as and Export, and the Experience, Systems, Data and Demo settings.

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
