"""
Byte-level tests for the Z21 frame builders.

Expected bytes are worked out by hand from the frame layout:
DataLen (2, LE) + Header (2, LE) + X-Bus bytes + XOR checksum.
"""

import pytest

from app.drivers.z21_driver import (
    build_emergency_stop,
    build_loco_drive,
    build_loco_info_request,
    build_status_request,
    encode_address,
    encode_speed_step,
    xor_checksum,
)


def test_xor_checksum_covers_all_bytes():
    assert xor_checksum(bytes([0xE4, 0x13, 0x00, 0x03, 0x80])) == 0x74


def test_loco_drive_forward_speed_zero():
    # Step 0 with forward bit: stop, but direction set
    assert build_loco_drive(3, 0, True) == bytes(
        [0x0A, 0x00, 0x40, 0x00, 0xE4, 0x13, 0x00, 0x03, 0x80, 0x74]
    )


def test_loco_drive_speed_one_is_step_two_not_estop():
    # UI speed 1 must not become step 1 (emergency stop)
    frame = build_loco_drive(3, 1, True)
    assert frame[8] == 0x82
    assert frame[9] == 0x76


def test_loco_drive_reverse_clears_direction_bit():
    frame = build_loco_drive(3, 10, False)
    assert frame[8] == 0x0B  # step 11, bit 7 clear


def test_loco_drive_max_speed_is_step_127():
    frame = build_loco_drive(3, 126, True)
    assert frame[8] == 0xFF  # 0x80 | 127


def test_speed_step_encoding_table():
    assert encode_speed_step(0, True) == 0x80
    assert encode_speed_step(0, False) == 0x00
    assert encode_speed_step(1, False) == 0x02
    assert encode_speed_step(126, False) == 0x7F


def test_short_address_has_zero_high_byte():
    assert encode_address(3) == (0x00, 0x03)
    assert encode_address(127) == (0x00, 0x7F)


def test_long_address_sets_0xc0_flag():
    # 1234 = 0x04D2
    assert encode_address(128) == (0xC0, 0x80)
    assert encode_address(1234) == (0xC4, 0xD2)
    assert encode_address(9999) == (0xC0 | (9999 >> 8), 9999 & 0xFF)


@pytest.mark.parametrize("address", [0, -1, 10000])
def test_invalid_address_rejected(address):
    with pytest.raises(ValueError):
        encode_address(address)


@pytest.mark.parametrize("speed", [-1, 127, 200])
def test_invalid_speed_rejected(speed):
    with pytest.raises(ValueError):
        build_loco_drive(3, speed, True)


def test_loco_info_request_bytes():
    assert build_loco_info_request(3) == bytes(
        [0x09, 0x00, 0x40, 0x00, 0xE3, 0xF0, 0x00, 0x03, 0x10]
    )


def test_emergency_stop_is_xbus_stop_not_address_zero():
    assert build_emergency_stop() == bytes([0x06, 0x00, 0x40, 0x00, 0x80, 0x80])


def test_status_request_bytes():
    assert build_status_request() == bytes(
        [0x08, 0x00, 0x40, 0x00, 0x21, 0x24, 0x05, 0x00]
    )


def test_frame_length_field_matches_frame_size():
    for frame in (
        build_loco_drive(1234, 50, True),
        build_loco_info_request(3),
        build_emergency_stop(),
        build_status_request(),
    ):
        assert int.from_bytes(frame[0:2], "little") == len(frame)
