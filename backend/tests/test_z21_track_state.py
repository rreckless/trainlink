"""
Track state tests: short circuit, track power off, and resume.

Covers the frame encoding, the driver's state handling against the MockZ21,
and the API refusals while the track isn't ready.
"""

import asyncio

import pytest
from fastapi.testclient import TestClient

from app.api.z21 import get_z21
from app.drivers.z21_driver import (
    CENTRAL_SHORT_CIRCUIT,
    CENTRAL_TRACK_OFF,
    Z21Driver,
    build_track_power_on,
    parse_central_state,
)
from app.main import app


# Status reply frame: LAN_X_STATUS_CHANGED (X-Bus 0x62 0x22), status byte, XOR checksum
def status_reply(status: int) -> bytes:
    checksum = 0x62 ^ 0x22 ^ status
    return bytes([0x08, 0x00, 0x40, 0x00, 0x62, 0x22, status, checksum])


def test_track_power_on_bytes():
    assert build_track_power_on() == bytes([0x07, 0x00, 0x40, 0x00, 0x21, 0x81, 0xA0])


def test_parse_central_state_normal():
    assert parse_central_state(0x00) == {
        "emergency_stop": False,
        "track_power_on": True,
        "short_circuit": False,
    }


def test_parse_central_state_short_circuit():
    state = parse_central_state(CENTRAL_SHORT_CIRCUIT | CENTRAL_TRACK_OFF)
    assert state["short_circuit"] is True
    assert state["track_power_on"] is False


async def _connected_driver(mock) -> Z21Driver:
    driver = Z21Driver(host=mock.host, port=mock.port)
    assert await driver.connect() is True
    assert mock.wait_for_frames(1)
    return driver


@pytest.mark.asyncio
async def test_state_is_unknown_until_first_report(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        assert driver.track_state == "unknown"
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_short_circuit_reply_sets_short_circuit_state(mock_z21):
    driver = await _connected_driver(mock_z21)
    events = []

    async def on_status(event):
        events.append(event)

    driver.on_status_changed = on_status
    try:
        mock_z21.send_to_client(status_reply(CENTRAL_SHORT_CIRCUIT | CENTRAL_TRACK_OFF))
        await asyncio.sleep(0.1)
        assert driver.track_state == "short_circuit"
        assert events[-1]["track_state"] == "short_circuit"
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_short_cleared_but_track_off_reports_track_power_off(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        mock_z21.send_to_client(status_reply(CENTRAL_SHORT_CIRCUIT | CENTRAL_TRACK_OFF))
        await asyncio.sleep(0.1)
        mock_z21.send_to_client(status_reply(CENTRAL_TRACK_OFF))
        await asyncio.sleep(0.1)
        assert driver.track_state == "track_power_off"
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_normal_status_reports_ready(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        mock_z21.send_to_client(status_reply(0x00))
        await asyncio.sleep(0.1)
        assert driver.track_state == "ready"
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_system_state_frame_updates_central_state(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        # 4-byte header, then 16 bytes of system state. CentralState is at data offset 12
        frame = bytearray(20)
        frame[0:4] = bytes([0x14, 0x00, 0x84, 0x00])
        frame[4 + 12] = CENTRAL_SHORT_CIRCUIT
        mock_z21.send_to_client(bytes(frame))
        await asyncio.sleep(0.1)
        assert driver.track_state == "short_circuit"
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_disconnected_state(mock_z21):
    driver = await _connected_driver(mock_z21)
    mock_z21.send_to_client(status_reply(0x00))
    await asyncio.sleep(0.1)
    await driver.disconnect()
    assert driver.track_state == "disconnected"
    assert driver.central_state is None


@pytest.mark.asyncio
async def test_track_power_on_sends_frame(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        assert await driver.track_power_on() is True
        await asyncio.sleep(0.05)
        assert mock_z21.sent_frames()[-1] == build_track_power_on()
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_reconnect_closes_old_socket_and_clears_state(mock_z21):
    driver = await _connected_driver(mock_z21)
    mock_z21.send_to_client(status_reply(CENTRAL_SHORT_CIRCUIT))
    await asyncio.sleep(0.1)
    assert driver.track_state == "short_circuit"

    assert await driver.connect() is True
    try:
        assert driver.track_state == "unknown"
    finally:
        await driver.disconnect()


# API tests. The driver is replaced with one that has a fake transport,
# so the state can be set directly and the sent frames can be checked.

class FakeTransport:
    def __init__(self):
        self.sent: list[bytes] = []

    def sendto(self, data):
        self.sent.append(data)

    def close(self):
        pass


def _api_driver(state: dict | None, connected: bool = True) -> Z21Driver:
    driver = Z21Driver()
    driver.transport = FakeTransport()
    driver.connected = connected
    driver.central_state = state
    return driver


@pytest.fixture
def client():
    with TestClient(app) as c:
        yield c
    app.dependency_overrides.clear()


def _use_driver(driver: Z21Driver):
    async def override():
        return driver

    app.dependency_overrides[get_z21] = override


NORMAL = {"emergency_stop": False, "track_power_on": True, "short_circuit": False}
SHORT = {"emergency_stop": False, "track_power_on": False, "short_circuit": True}
TRACK_OFF = {"emergency_stop": False, "track_power_on": False, "short_circuit": False}


def test_speed_refused_during_short_circuit(client):
    driver = _api_driver(SHORT)
    _use_driver(driver)
    response = client.post("/api/z21/loco/speed", json={"address": 3, "speed": 10, "direction": True})
    assert response.status_code == 409
    assert "short_circuit" in response.json()["detail"]
    assert all(frame[4] != 0xE4 for frame in driver.transport.sent)


def test_speed_refused_when_track_power_off(client):
    driver = _api_driver(TRACK_OFF)
    _use_driver(driver)
    response = client.post("/api/z21/loco/speed", json={"address": 3, "speed": 10, "direction": True})
    assert response.status_code == 409


def test_speed_allowed_when_ready(client):
    driver = _api_driver(NORMAL)
    _use_driver(driver)
    response = client.post("/api/z21/loco/speed", json={"address": 3, "speed": 10, "direction": True})
    assert response.status_code == 200
    assert any(frame[4] == 0xE4 for frame in driver.transport.sent)


def test_speed_refused_while_state_unknown(client):
    driver = _api_driver(None)
    _use_driver(driver)
    response = client.post("/api/z21/loco/speed", json={"address": 3, "speed": 10, "direction": True})
    assert response.status_code == 409


def test_emergency_stop_still_allowed_during_short(client):
    driver = _api_driver(SHORT)
    _use_driver(driver)
    response = client.post("/api/z21/emergency-stop", json={"confirm": True})
    assert response.status_code == 200


def test_track_power_on_refused_during_short(client):
    driver = _api_driver(SHORT)
    _use_driver(driver)
    response = client.post("/api/z21/track-power/on")
    assert response.status_code == 409
    assert driver.transport.sent == []


def test_track_power_on_sent_when_short_cleared(client):
    driver = _api_driver(TRACK_OFF)
    _use_driver(driver)
    response = client.post("/api/z21/track-power/on")
    assert response.status_code == 200
    assert driver.transport.sent[-1] == build_track_power_on()


def test_status_reports_state(client):
    driver = _api_driver(SHORT)
    _use_driver(driver)
    response = client.get("/api/z21/status")
    assert response.status_code == 200
    body = response.json()
    assert body["connected"] is True
    assert body["state"] == "short_circuit"
    assert body["central_state"] == SHORT


def test_status_disconnected(client):
    driver = _api_driver(None, connected=False)
    _use_driver(driver)
    response = client.get("/api/z21/status")
    assert response.json()["state"] == "disconnected"
