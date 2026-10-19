"""
Z21 turnout commands (DCC accessory decoders), from the Z21 LAN protocol spec v1.13, sections 5.1 to 5.3.

Turnouts are numbered 1-2048. On the wire the function address is one less: turnout 25 is sent as 24.
The output is 0 or 1, which the spec calls P=0 and P=1.
"""

import asyncio

from .z21_driver import XBUS_TURNOUT_INFO, Z21Driver, build_xbus_frame

MIN_TURNOUT = 1
MAX_TURNOUT = 2048
XBUS_SET_TURNOUT = 0x53
# Queue flag (Z21 firmware V1.24 or later): the Z21 queues the command and sends it at the next chance
QUEUE_FLAG = 0x20
ACTIVATE_FLAG = 0x08
# Time the output stays active before it is deactivated. The spec's Q=1 example uses 150 ms
THROW_SECONDS = 0.15


def _function_address(turnout: int) -> tuple[int, int]:
    if not MIN_TURNOUT <= turnout <= MAX_TURNOUT:
        raise ValueError(f"Turnout must be {MIN_TURNOUT}-{MAX_TURNOUT}")
    address = turnout - 1
    return (address >> 8) & 0xFF, address & 0xFF


def build_turnout_command(turnout: int, output: int, activate: bool) -> bytes:
    """LAN_X_SET_TURNOUT with Q=1: 0x53 FAdr_MSB FAdr_LSB 10Q0A00P"""
    if output not in (0, 1):
        raise ValueError("Output must be 0 or 1")
    msb, lsb = _function_address(turnout)
    flags = 0x80 | QUEUE_FLAG | (ACTIVATE_FLAG if activate else 0) | output
    return build_xbus_frame(bytes([XBUS_SET_TURNOUT, msb, lsb, flags]))


def build_turnout_info_request(turnout: int) -> bytes:
    """LAN_X_GET_TURNOUT_INFO: 0x43 FAdr_MSB FAdr_LSB. The reply is a LAN_X_TURNOUT_INFO"""
    msb, lsb = _function_address(turnout)
    return build_xbus_frame(bytes([XBUS_TURNOUT_INFO, msb, lsb]))


async def throw_turnout(driver: Z21Driver, turnout: int, output: int):
    """Activate one output, then deactivate it after the throw time, as the spec's example does"""
    await driver._send_message(build_turnout_command(turnout, output, True))
    await asyncio.sleep(THROW_SECONDS)
    await driver._send_message(build_turnout_command(turnout, output, False))


async def request_turnout_info(driver: Z21Driver, turnout: int):
    """Ask the Z21 for one turnout's output. The reply updates driver.turnouts"""
    await driver._send_message(build_turnout_info_request(turnout))
