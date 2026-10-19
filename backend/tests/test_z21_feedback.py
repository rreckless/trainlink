"""
R-BUS feedback tests (Z21 LAN spec sections 2.16 and 7).

Expected bytes are taken from the spec examples where they exist.
"""

import asyncio

import pytest

from app.drivers.z21_driver import (
    BROADCAST_DRIVING_SWITCHING,
    BROADCAST_RBUS_FEEDBACK,
    BROADCAST_SYSTEMSTATE,
    Z21Driver,
    build_rbus_getdata,
    build_set_broadcast_flags,
    parse_rbus_feedback,
)


def test_rbus_getdata_bytes():
    # Spec 7.2: DataLen 0x05, header 0x81, group index
    assert build_rbus_getdata(1) == bytes([0x05, 0x00, 0x81, 0x00, 0x01])


def test_rbus_getdata_rejects_unknown_group():
    with pytest.raises(ValueError):
        build_rbus_getdata(2)


def test_set_broadcast_flags_bytes():
    # Spec 2.16: DataLen 0x08, header 0x50, 32-bit flags little endian
    frame = build_set_broadcast_flags(BROADCAST_DRIVING_SWITCHING | BROADCAST_RBUS_FEEDBACK)
    assert frame == bytes([0x08, 0x00, 0x50, 0x00, 0x03, 0x00, 0x00, 0x00])


def test_parse_feedback_spec_example():
    # Spec 7.1 example: group 1, module 11 input 1; module 13 inputs 8, 7, 3, 1
    status = bytes([0x01, 0x00, 0xC5, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    states = parse_rbus_feedback(1, status)

    occupied = {key for key, value in states.items() if value}
    assert occupied == {(11, 1), (13, 1), (13, 3), (13, 7), (13, 8)}
    assert len(states) == 10 * 8


def test_parse_feedback_group_zero_numbering():
    states = parse_rbus_feedback(0, bytes([0x00] * 9 + [0x80]))
    assert states[(10, 8)] is True
    assert states[(1, 1)] is False


def test_parse_feedback_rejects_wrong_length():
    with pytest.raises(ValueError):
        parse_rbus_feedback(0, bytes(9))


async def _connected_driver(mock) -> Z21Driver:
    driver = Z21Driver(host=mock.host, port=mock.port)
    assert await driver.connect() is True
    # The mock learns the driver's address from its first frame
    assert mock.wait_for_frames(1)
    return driver


@pytest.mark.asyncio
async def test_connect_sends_broadcast_flags_and_feedback_polls(mock_z21):
    driver = await _connected_driver(mock_z21)
    try:
        assert mock_z21.wait_for_frames(4)
        frames = mock_z21.sent_frames()
        expected_flags = build_set_broadcast_flags(
            BROADCAST_DRIVING_SWITCHING | BROADCAST_RBUS_FEEDBACK | BROADCAST_SYSTEMSTATE
        )
        assert expected_flags in frames
        assert build_rbus_getdata(0) in frames
        assert build_rbus_getdata(1) in frames
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_feedback_frame_updates_state_and_reports_changes(mock_z21):
    driver = await _connected_driver(mock_z21)
    events = []

    async def on_status(event):
        events.append(event)

    driver.on_status_changed = on_status
    try:
        status = bytes([0x01, 0x00, 0xC5, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        frame = bytes([0x0F, 0x00, 0x80, 0x00, 0x01]) + status
        mock_z21.send_to_client(frame)
        await asyncio.sleep(0.1)

        assert driver.feedback[(13, 8)] is True
        assert driver.feedback[(11, 1)] is True
        assert driver.feedback[(11, 2)] is False

        changed_events = [e for e in events if e["status"] == "feedback_changed"]
        assert len(changed_events) == 1
        assert changed_events[0]["changes"][(13, 8)] is True
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_unchanged_feedback_reports_no_change(mock_z21):
    driver = await _connected_driver(mock_z21)
    events = []

    async def on_status(event):
        events.append(event)

    driver.on_status_changed = on_status
    try:
        frame = bytes([0x0F, 0x00, 0x80, 0x00, 0x00]) + bytes(10)
        mock_z21.send_to_client(frame)
        await asyncio.sleep(0.1)
        events.clear()

        mock_z21.send_to_client(frame)
        await asyncio.sleep(0.1)
        assert [e for e in events if e["status"] == "feedback_changed"] == []
    finally:
        await driver.disconnect()


@pytest.mark.asyncio
async def test_disconnect_clears_feedback(mock_z21):
    driver = await _connected_driver(mock_z21)
    frame = bytes([0x0F, 0x00, 0x80, 0x00, 0x00, 0x01]) + bytes(9)
    mock_z21.send_to_client(frame)
    await asyncio.sleep(0.1)
    assert driver.feedback

    await driver.disconnect()
    assert driver.feedback == {}
