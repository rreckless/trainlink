# TrainLink - Implementation Plan

## Project Overview
Build a web-based replica of iTrain, a digital model railroad control system for managing locomotives, blocks, signals, and routes, with manual and automatic control. TrainLink targets the **Roco Z21** command station first. Other command station drivers are added only after Z21 is fully functional (Phase 7).

**Status key:** ✅ Done · 🟡 Partial · 🧪 Built but untested · ⬜ Not started

**Last reviewed:** 2026-10-19

---

## Current Status Snapshot

| Area | Status | Notes |
|------|--------|-------|
| Project setup, Docker (PostgreSQL, Redis), Python 3.12 venv | ✅ | Renamed to TrainLink |
| Z21 UDP driver and API routes | ✅ | Connect, speed, direction, emergency stop, status. Train run on real Z21 (user test) |
| Locomotive control UI | ✅ | Speed slider, direction buttons, emergency stop. Train run on real Z21 (user test) |
| System settings UI (Z21 host and port) | 🧪 | Depends on the Z21 driver being tested |
| Database models and schemas | 🟡 | Locomotive, Block, Signal, Feedback, Route, Layout, CommandStation defined. No REST routes for them |
| WebSocket endpoint | 🟡 | Endpoint and connection manager exist. Messages are echoed to all clients. No auth, no typed events |
| Redis | 🟡 | Configured in Docker and config. Not used in code |
| Frontend state (Zustand) | 🟡 | Store exists. Only some panels read from it |
| Switchboard, Route Manager, Layout Editor | 🟡 | Placeholder UI only |
| Locomotive Panel | 🟡 | Placeholder component |
| Electron wrapper | 🟡 | `electron/` scaffold exists. Not verified to run |
| Backend tests | ⬜ | `backend/tests/` is empty |
| Mock Z21 for testing | ⬜ | Not started |

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
- **Language**: Python 3.12
- **Framework**: FastAPI
- **Real-time**: WebSocket for multi-client sync (infrastructure only)
- **Database**: PostgreSQL (relational data, via SQLAlchemy)
- **Cache**: Redis (real-time state, not yet used)

### Hardware Communication
- **Command station (current)**: Roco Z21
- **Transport**: UDP, default port 21105, configurable host and port
- **Protocol**: Z21 LAN protocol (binary, length-prefixed messages). Locomotive control uses DCC addressing (1–9999), speed steps 0–126, with direction in bit 7
- **Future**: Z21 feedback messages (occupancy) and turnout commands. Both are planned, not built
- **Later, once Z21 is fully functional**: Märklin 6051/P50x, Lenz Intellibox, Digitrax LocoNet (see Phase 7). Roco/Fleischmann ECoS is also a candidate

### Architecture Pattern
- **Client-Server**: Central FastAPI server owns layout state. Browsers and Electron connect as clients
- **Publish-Subscribe**: WebSocket events for live updates
- **Driver layer**: The Z21 driver is isolated behind a service so the rest of the backend does not speak UDP directly. Additional command stations plug in through the same base driver interface (Phase 7)

---

## Implementation Phases

### Phase 1: Foundation (2-3 weeks)
**Goal**: Core architecture and basic UI

1. **Project Setup** ✅
   - Monorepo with `backend/`, `frontend/`, `electron/`
   - Docker Compose for PostgreSQL and Redis
   - Database models defined

2. **Backend Foundation** 🟡
   - ✅ FastAPI app with CORS and health check
   - 🟡 WebSocket endpoint (echo only, no authentication)
   - 🟡 Database models: Locomotive, Block, Signal, Feedback, Route, Layout, CommandStation. REST routes exist for Locomotive, Block, Signal, and Route only
   - 🟡 Basic REST API: Z21, locomotives, blocks, signals, routes. Feedback, layouts, and command stations still need routes
   - ⬜ State management in Redis

3. **Frontend Foundation** 🟡
   - 🟡 Layout editor skeleton (placeholder)
   - 🧪 Locomotive control panel
   - ✅ Connection status display (Z21 heartbeat)
   - 🧪 Settings UI (Z21 host and port)

4. **Testing Infrastructure** ⬜
   - ✅ Backend unit test setup (`backend/tests/`, 30 passing)
   - 🟡 Integration test framework (driver tests run against the mock)
   - ✅ Mock Z21 UDP responder for testing (`backend/tests/conftest.py`)
   - ✅ Z21 frame encoding checked against the Z21 LAN Protocol Specification v1.13. Real hardware test still pending

### Phase 2: Core Features (3-4 weeks)
**Goal**: Fundamental train control on real hardware

1. **Locomotive Control** 🧪
   - ✅ Speed control (0–126 steps). Verified on hardware
   - ✅ Direction (Forward / Reverse / Stop). Verified on hardware
   - ⬜ Function buttons (lights, sounds). Z21 function commands not implemented
   - ⬜ Travelled time and distance tracking
   - ⬜ Real-time speed feedback from Z21
   - ✅ Emergency stop. Verified on hardware (not yet confirmed separately)
   - 🧪 Track state from Z21 (short circuit, track power off), with locomotive commands blocked until ready and a Resume Track Power action. Reconnect in Settings

2. **Block System** ⬜
   - 🟡 Block model exists
   - ⬜ Block definition and visualization
   - ⬜ Block occupancy detection via Z21 feedback
   - ⬜ Block linking to signals
   - ⬜ Automatic signal management
   - ⬜ Block properties UI

3. **Signal Control** ⬜
   - 🟡 Signal model exists
   - ⬜ Main signal aspects (Stop, Caution, Go)
   - ⬜ Warning signals
   - ⬜ Automatic signal assignment from block state
   - ⬜ Signal visualization on layout

4. **Layout Drawing** 🧪
   - ✅ Layout REST routes (track segments stored in layout elements)
   - 🧪 Draw straight track segments (free drawing, kept alongside track pieces)
   - 🧪 Track library: PECO N gauge catalogue entries (46, unverified until checked against the catalogue)
   - 🧪 Place Setrack (Code 80) straights, curves, and left- and right-hand turnouts. Ends snap to open ends and a layout uses one track system
   - ⬜ Streamline (Code 55) pieces: listed but not placeable. Turnout geometry in the catalogue doesn't reconcile yet
   - ⬜ Crossings, curved turnouts, Y turnouts, slips, and flex track: listed, placement not built
   - 🧪 Place blocks, signals, feedbacks
   - 🧪 Save and load layouts
   - 🧪 Drag-and-drop editing
   - 🧪 Snap-to-grid (20 unit grid)
   - ⬜ Signals and feedbacks are placed but have no link to automation or hardware yet

### Phase 3: Automation & Routes (3-4 weeks)
**Goal**: Route-based automation

1. **Route Management** ⬜
   - 🟡 Route model exists, Route Manager UI is a placeholder
   - ⬜ Route definition (sequence of blocks)
   - ⬜ Route creation UI with wizard
   - ⬜ Advanced route options (speed limits, dwell times)
   - ⬜ Route visualization

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

4. **Turnouts** ⬜
   - ⬜ Z21 turnout commands
   - ⬜ Turnout state tracking
   - ⬜ Turnout control in the switchboard

5. **Feedback / Sensor Integration** ⬜
   - ⬜ Z21 feedback message handling
   - 🟡 Feedback model exists
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
   - ⬜ Protocol parsing for incoming Z21 messages (only outgoing commands are built)
   - ⬜ Error handling and recovery

2. **Z21 Feature Support** ⬜
   - ⬜ Query Z21 system state and version
   - ⬜ Query locomotive info (speed, direction, functions)
   - ⬜ Track power on and off
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

---

## Immediate Priorities

1. ✅ **Test locomotive control against a real Z21.** Done: a train ran under TrainLink control (user test). Still to confirm on hardware: short-circuit detection, track power resume, and feedback. Connect, set speed and direction, emergency stop, and confirm the status heartbeat. Record results in `IMPLEMENTATION_SUMMARY.md`. Frame bytes have been checked against the Z21 LAN Protocol Specification v1.13.
2. ✅ Mock Z21 UDP responder (done)
3. ✅ Backend unit tests for the Z21 packet builder and the speed and direction logic (done, 30 passing)
4. ✅ REST routes for locomotives, blocks, signals, and routes (done, 79 passing). Layouts and command stations still need routes so blocks can be linked to signals and feedback.
5. 🟡 Z21 R-BUS feedback parsing and `GET /api/z21/feedback` done (spec section 7, 92 tests passing). Still to do: link feedback inputs to blocks, and verify on hardware.

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

### Backend Modules
1. **Layout Manager**: Load, save, validate layouts. ⬜
2. **Block Controller**: Track occupancy, manage blocks. ⬜
3. **Signal Controller**: Manage signal states from block state. ⬜
4. **Route Engine**: Execute routes, handle automation. ⬜
5. **Locomotive Manager**: Control trains, track state. 🧪 (Z21 speed and direction)
6. **Z21 Driver**: Abstract the Z21 UDP link. 🧪
7. **WebSocket Server**: Real-time multi-client communication. 🟡
8. **State Synchronizer**: Keep state consistent across clients. ⬜

### Frontend Components
1. **Layout Editor**: Draw and configure the layout. 🧪 (drawing, placement, save/load; not yet tested in a browser)
2. **Switchboard View**: Visual representation of the layout. 🟡 (placeholder)
3. **Locomotive Control**: Control individual trains. 🧪
4. **Route Manager**: Create and manage routes. 🟡 (placeholder)
5. **Block Monitor**: View block states. ⬜
6. **System Settings**: Z21 connection settings. 🧪
7. **Network Status**: Show connected clients. ⬜
8. **Event Log**: Display system events. ⬜

---

## File Structure

Current layout of the repository. Items marked ⬜ are planned.

```
trainlink/
├── backend/
│   ├── app/
│   │   ├── main.py                       # FastAPI app, WebSocket endpoint
│   │   ├── config.py
│   │   ├── database.py
│   │   ├── models/                       # SQLAlchemy models (block, command_station, feedback, layout, locomotive, route, signal)
│   │   ├── schemas/                      # Pydantic schemas (same entities)
│   │   ├── api/
│   │   │   ├── z21.py                    # ✅ Z21 routes
│   │   │   ├── locomotives.py            # ⬜
│   │   │   ├── blocks.py                 # ⬜
│   │   │   ├── signals.py                # ⬜
│   │   │   └── routes.py                 # ⬜
│   │   ├── ws/
│   │   │   └── connection_manager.py     # 🟡
│   │   ├── services/                     # ⬜ Business logic (empty)
│   │   │   ├── layout_service.py
│   │   │   ├── block_service.py
│   │   │   ├── route_service.py
│   │   │   └── locomotive_service.py
│   │   └── drivers/
│   │       ├── z21_driver.py             # 🧪
│   │       ├── base_driver.py            # ⬜ Interface for future adapters (Phase 7)
│   │       ├── marklin_p50x.py           # ⬜ Phase 7
│   │       ├── lenz_intellibox.py        # ⬜ Phase 7
│   │       └── loconet_driver.py         # ⬜ Phase 7
│   ├── tests/                            # ⬜ Empty
│   └── requirements.txt
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   │   ├── LocomotiveControl.jsx     # 🧪
│   │   │   ├── LocomotivePanel.jsx       # 🟡
│   │   │   ├── SystemSettings.jsx        # 🧪
│   │   │   ├── Switchboard.jsx           # 🟡
│   │   │   ├── RouteManager.jsx          # 🟡
│   │   │   └── LayoutEditor.jsx          # 🟡
│   │   ├── hooks/
│   │   │   └── useWebSocket.js           # 🟡
│   │   ├── store/
│   │   │   └── index.js                  # 🟡 Zustand
│   │   ├── App.jsx
│   │   └── main.jsx
│   └── package.json
├── electron/                             # 🟡 Scaffold
│   ├── main.js
│   ├── preload.js
│   └── package.json
├── docker-compose.yml                    # ✅ PostgreSQL + Redis
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
| Core Features | 3-4 weeks | 🧪 Loco control only | Full locomotive and block control |
| Automation | 3-4 weeks | ⬜ | Route-based automation working |
| Multi-Client | 2-3 weeks | ⬜ | Network support functional |
| Z21 Hardening | 2-3 weeks | ⬜ | Verified Z21 integration |
| Polish | 2 weeks | ⬜ | Production-ready executable |
| Additional Drivers | 2-3 weeks each | ⬜ (after Z21 complete) | Märklin, Intellibox, LocoNet, ECoS |
| **Total** | **~16-21 weeks for Z21, plus ~8-12 weeks for drivers** | | Full TrainLink |

---

## Getting Started Checklist

- [x] Choose backend framework (FastAPI)
- [x] Set up project repository with monorepo structure
- [x] Design database schema (PostgreSQL)
- [ ] Create basic Electron app shell (scaffold only)
- [x] Implement WebSocket server (echo only)
- [x] Build layout editor component (placeholder)
- [x] Create locomotive control UI
- [ ] Test locomotive control against real Z21
- [ ] Build mock Z21 for testing
- [ ] Build comprehensive test suite
- [ ] Document API and component interfaces
- [ ] Phase 7: Extract base driver interface (after Z21 complete)

---

## Resources & References

### Roco Z21
- Z21 LAN Protocol documentation (Roco / Fleischmann)
- Z21 default address: UDP port 21105

### Libraries
- **Python**: FastAPI, SQLAlchemy, Pydantic, uvicorn
- **Frontend**: React, Vite, Zustand, Tailwind CSS
- **To evaluate**: Konva.js or Fabric.js (layout editor), python-socketio (if WebSocket needs rooms)
- **Electron**: electron-builder or electron-forge (packaging)

---

## Success Criteria

✅ **MVP Complete when:**
- [ ] Z21 connection verified on real hardware
- [ ] Locomotive speed and direction control verified on real hardware
- [ ] Emergency stop verified on real hardware
- [ ] Layout editor functional
- [ ] Block occupancy detection functional
- [ ] Basic signal automation in place
- [ ] Multiple clients can connect and sync

✅ **Feature Complete (Z21) when:**
- [ ] Z21 turnout and track power control working
- [ ] Routes and automation fully working
- [ ] Multi-client support stable
- [ ] Export and import functionality working
- [ ] Performance acceptable for large layouts (100+ blocks)

✅ **Multi-Command-Station Complete when:**
- [ ] Feature Complete (Z21) criteria met
- [ ] At least one additional driver (Phase 7) verified on real hardware
- [ ] All planned drivers (Märklin P50x, Intellibox, LocoNet) verified on real hardware
