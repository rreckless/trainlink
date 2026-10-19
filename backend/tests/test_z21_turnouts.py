"""
Z21 turnout commands, checked against the LAN protocol spec v1.13 (sections 5.1 to 5.3)
and its worked example: turnout #25 / A2 is activated with 0xA9 and deactivated with 0xA1.
"""

import asyncio

import pytest

from app.drivers import z21_turnouts
from app.drivers.z21_driver import Z21Driver, parse_turnout_info
from app.drivers.z21_turnouts import (
    build_turnout_command,
    build_turnout_info_request,
    request_turnout_info,
    throw_turnout,
)


def test_set_turnout_matches_the_spec_example():
    # Turnout 25 is FAdr 24 (0x18). Output 2 is P=1. Q=1 and A=1 give 0xA9
    assert build_turnout_command(25, 1, activate=True) == bytes.fromhex("09004000" "530018A9E2")
    assert build_turnout_command(25, 1, activate=False) == bytes.fromhex("09004000" "530018A1EA")


def test_output_one_clears_the_output_bit():
    # Output 1 is P=0, so the last byte is 0xA8 when activated
    frame = build_turnout_command(25, 0, activate=True)
    assert frame[7] == 0xA8


def test_turnout_info_request_matches_the_spec():
    # 0x43 with FAdr 24: frame 08 00 40 00 43 00 18 5B
    assert build_turnout_info_request(25) == bytes.fromhex("08004000" "4300185B")


def test_function_address_is_split_into_msb_and_lsb():
    # Turnout 2048 is FAdr 2047, which is 0x07FF
    frame = build_turnout_info_request(2048)
    assert frame[4:7] == bytes([0x43, 0x07, 0xFF])


@pytest.mark.parametrize("turnout", [0, 2049, -1])
def test_turnout_out_of_range_is_refused(turnout):
    with pytest.raises(ValueError):
        build_turnout_command(turnout, 0, activate=True)


def test_output_must_be_zero_or_one():
    with pytest.raises(ValueError):
        build_turnout_command(25, 2, activate=True)


def test_parse_turnout_info_reports_the_output():
    assert parse_turnout_info(0x00, 0x18, 0x01) == (25, 0)
    assert parse_turnout_info(0x00, 0x18, 0x02) == (25, 1)


@pytest.mark.parametrize("zz", [0x00, 0x03])
def test_parse_turnout_info_unknown_or_invalid_is_none(zz):
    assert parse_turnout_info(0x00, 0x18, zz) == (25, None)


class FakeDriver(Z21Driver):
    """Records the frames the driver would send"""

    def __init__(self):
        super().__init__()
        self.sent = []

    async def _send_message(self, message: bytes):
        self.sent.append(message)


def test_throw_activates_then_deactivates(monkeypatch):
    monkeypatch.setattr(z21_turnouts, "THROW_SECONDS", 0)
    driver = FakeDriver()
    asyncio.run(throw_turnout(driver, 25, 1))
    assert driver.sent == [
        bytes.fromhex("09004000" "530018A9E2"),
        bytes.fromhex("09004000" "530018A1EA"),
    ]


def test_request_turnout_info_sends_one_request():
    driver = FakeDriver()
    asyncio.run(request_turnout_info(driver, 25))
    assert driver.sent == [bytes.fromhex("08004000" "4300185B")]


def test_turnout_info_reply_updates_driver_state():
    driver = Z21Driver()
    reply = bytes.fromhex("09004000" "43001801" "5A")
    asyncio.run(driver._handle_message(reply))
    assert driver.turnouts == {25: 0}
