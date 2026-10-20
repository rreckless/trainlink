"""
Z21 locomotive commands and replies, from the Z21 LAN protocol spec v1.13, sections 4.1, 4.3.1, and 4.4.

Locomotive addresses are 1-9999. The Z21 reports speed in the 128-step format used by the drive command.
"""

from typing import Optional

from .z21_frames import build_xbus_frame, encode_address

XBUS_LOCO_INFO_REQUEST = 0xE3
XBUS_LOCO_INFO_DB0 = 0xF0
XBUS_SET_FUNCTION = 0xE4
XBUS_SET_FUNCTION_DB0 = 0xF8
XBUS_LOCO_INFO_REPLY = 0xEF
# Speed step information in DB2 (KKK). 4 is 128 steps, the format this driver uses
SPEED_STEPS_128 = 4
MAX_FUNCTION = 31


def build_loco_info_request(address: int) -> bytes:
    """LAN_X_GET_LOCO_INFO (4.1). Also subscribes this client to the locomotive's replies"""
    ah, al = encode_address(address)
    return build_xbus_frame(bytes([XBUS_LOCO_INFO_REQUEST, XBUS_LOCO_INFO_DB0, ah, al]))


def build_set_function(address: int, index: int, on: bool) -> bytes:
    """LAN_X_SET_LOCO_FUNCTION (4.3.1): 0xE4 0xF8 Adr_MSB Adr_LSB TTNNNNNN, TT=01 on or 00 off"""
    if not (0 <= index <= MAX_FUNCTION):
        raise ValueError(f"Function must be 0-{MAX_FUNCTION}")
    ah, al = encode_address(address)
    switch = 0x40 if on else 0x00
    return build_xbus_frame(bytes([XBUS_SET_FUNCTION, XBUS_SET_FUNCTION_DB0, ah, al, switch | index]))


def parse_loco_info(data: bytes) -> Optional[dict]:
    """Decode a LAN_X_LOCO_INFO reply (4.4).

    data starts at the X-Header (0xEF) and excludes the XOR byte. Returns None if it is too short.
    Speed is None unless the reply is in the 128-step format.
    """
    # X-Header, DB0 to DB7 are required. DB8 (F29 to F31) is optional
    if len(data) < 9 or data[0] != XBUS_LOCO_INFO_REPLY:
        return None
    address = ((data[1] & 0x3F) << 8) + data[2]
    steps = data[3] & 0x07
    step = data[4] & 0x7F
    speed = None
    if steps == SPEED_STEPS_128:
        # Steps 0 (stop) and 1 (emergency stop) both mean 0 in the UI
        speed = 0 if step <= 1 else step - 1
    functions = {
        0: bool(data[5] & 0x10),
        1: bool(data[5] & 0x01),
        2: bool(data[5] & 0x02),
        3: bool(data[5] & 0x04),
        4: bool(data[5] & 0x08),
    }
    for group, first in ((data[6], 5), (data[7], 13), (data[8], 21)):
        for bit in range(8):
            functions[first + bit] = bool(group & (1 << bit))
    if len(data) > 9:
        for bit in range(3):
            functions[29 + bit] = bool(data[9] & (1 << bit))
    return {
        "address": address,
        "speed": speed,
        "direction": "forward" if data[4] & 0x80 else "reverse",
        "functions": [index for index, on in sorted(functions.items()) if on],
    }


async def request_loco_info(driver, address: int):
    """Ask the Z21 for a locomotive's state. This also subscribes to its later replies"""
    await driver._send_message(build_loco_info_request(address))


async def set_function(driver, address: int, index: int, on: bool):
    """Switch one locomotive function. Single-function commands only touch the function named"""
    await driver._send_message(build_set_function(address, index, on))
