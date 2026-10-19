"""
Driver tests against the MockZ21 UDP responder.
"""

import asyncio

import pytest

from app.drivers.z21_driver import Z21Driver, build_emergency_stop, build_loco_drive


async def _connected_driver(mock) -> Z21Driver:
    driver = Z21Driver(host=mock.host, port=mock.port)
    assert await driver.connect() is True
    # connect() sends a status request first. Tests count frames from that point on
    assert mock.wait_for_frames(1)
    return driver


async def _wait_for_driver_send(mock, count: int):
    # Frames are sent from the event loop. Give the mock thread time to record them
    await asyncio.sleep(0.05)
    assert mock.wait_for_frames(count), f"expected {count} frames, got {len(mock.sent_frames())}"


@pytest.mark.asyncio
async def test_connect_sends_status_request(mock_z21):
    driver = Z21Driver(host=mock_z21.host, port=mock_z21.port)
    try:
        assert await driver.connect() is True
        assert driver.connected is True
        assert mock_z21.wait_for_frames(1)
        assert mock_z21.sent_frames()[0][4] == 0x21
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_set_speed_sends_loco_drive_frame(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        assert await driver.set_loco_speed(3, 50, True) is True
        await _wait_for_driver_send(mock_z21, 2)
        assert mock_z21.sent_frames()[-1] == build_loco_drive(3, 50, True)
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_set_speed_reverse(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        await driver.set_loco_speed(4, 20, False)
        await _wait_for_driver_send(mock_z21, 2)
        assert mock_z21.sent_frames()[-1] == build_loco_drive(4, 20, False)
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_set_speed_invalid_address_returns_false_and_sends_nothing(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        await asyncio.sleep(0.05)
        before = len(mock_z21.sent_frames())
        assert await driver.set_loco_speed(0, 10, True) is False
        await asyncio.sleep(0.05)
        assert len(mock_z21.sent_frames()) == before  # invalid command sends nothing
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_set_speed_when_disconnected_returns_false():
    driver = Z21Driver(host="127.0.0.1", port=1)
    assert await driver.set_loco_speed(3, 10, True) is False


@pytest.mark.asyncio
async def test_emergency_stop_sends_xbus_stop(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        assert await driver.emergency_stop() is True
        await _wait_for_driver_send(mock_z21, 2)
        assert mock_z21.sent_frames()[-1] == build_emergency_stop()
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_emergency_stop_when_disconnected_returns_false():
    driver = Z21Driver(host="127.0.0.1", port=1)
    assert await driver.emergency_stop() is False


@pytest.mark.asyncio
async def test_loco_info_request_sent(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        result = await driver.get_loco_info(3)
        assert result == {"address": 3, "status": "requested"}
        await _wait_for_driver_send(mock_z21, 2)
        assert mock_z21.sent_frames()[-1][4:6] == bytes([0xE3, 0xF0])
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_feedback_frame_triggers_status_callback(mock_z21):
    driver = await _connected_driver(mock_z21)
    events = []

    async def on_status(event):
        events.append(event)

    driver.on_status_changed = on_status
    try:
        # Header 0x0080 = RMBUS data changed
        mock_z21.send_to_client(bytes([0x06, 0x00, 0x80, 0x00, 0x00, 0x00]))
        await asyncio.sleep(0.1)
        assert {"status": "feedback_received"} in events
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_system_state_frame_triggers_status_callback(mock_z21):
    driver = await _connected_driver(mock_z21)
    events = []

    async def on_status(event):
        events.append(event)

    driver.on_status_changed = on_status
    try:
        # Header 0x0084 = system state changed
        mock_z21.send_to_client(bytes([0x04, 0x00, 0x84, 0x00]))
        await asyncio.sleep(0.1)
        assert {"status": "system_state_changed"} in events
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_any_frame_updates_heartbeat(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        before = driver.last_heartbeat
        await asyncio.sleep(0.01)
        mock_z21.send_to_client(bytes([0x04, 0x00, 0x84, 0x00]))
        await asyncio.sleep(0.1)
        assert driver.last_heartbeat > before
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_short_frame_ignored(mock_z21):
    driver = await _connected_driver(mock_z21)
    events = []

    async def on_status(event):
        events.append(event)

    driver.on_status_changed = on_status
    try:
        mock_z21.send_to_client(bytes([0x04, 0x00]))
        await asyncio.sleep(0.1)
        assert events == []
    finally:
        await driver.disconnect()
