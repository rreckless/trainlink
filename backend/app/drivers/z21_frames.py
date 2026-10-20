"""
Z21 LAN frames: the 4-byte header and the X-Bus wrapper with its XOR checksum.

Frame layout (little-endian): DataLen (2 bytes, whole frame), Header (2 bytes), Data.
"""

import struct

MIN_ADDRESS = 1
MAX_ADDRESS = 9999


LAN_X_HEADER = 0x0040

def xor_checksum(data: bytes) -> int:
    """XOR of all bytes, used as the X-Bus checksum"""
    result = 0
    for byte in data:
        result ^= byte
    return result

def build_frame(header: int, data: bytes = b"") -> bytes:
    """Wrap data in a LAN frame: DataLen(2) + Header(2) + Data"""
    return struct.pack('<HH', 4 + len(data), header) + data

def build_xbus_frame(xbus: bytes) -> bytes:
    """Wrap an X-Bus command in a LAN_X frame and append its checksum"""
    return build_frame(LAN_X_HEADER, xbus + bytes([xor_checksum(xbus)]))

def encode_address(address: int) -> tuple[int, int]:
    """Encode a DCC address as (high byte, low byte).

    Addresses above 127 are long addresses and need the 0xC0 flag in the high byte.
    """
    if not (MIN_ADDRESS <= address <= MAX_ADDRESS):
        raise ValueError(f"Address must be {MIN_ADDRESS}-{MAX_ADDRESS}")
    if address < 128:
        return 0x00, address
    return 0xC0 | ((address >> 8) & 0x3F), address & 0xFF
