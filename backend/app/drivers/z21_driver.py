"""
Roco Z21 Driver - Z21 LAN protocol over UDP

Frame layout (little-endian):
    DataLen (2 bytes, total frame length including these 4 bytes)
    Header  (2 bytes)
    Data    (DataLen - 4 bytes)

Locomotive and track commands are X-Bus commands carried inside a LAN_X
frame (header 0x0040). The last X-Bus byte is an XOR checksum over all
preceding X-Bus bytes.

Protocol Reference:
- Default host: 10.0.0.111
- Port: 21105
- Verify byte values against the Z21 LAN Protocol spec before relying on them
"""

import asyncio
import struct
import logging
from typing import Optional, Callable
from datetime import datetime

from .z21_frames import LAN_X_HEADER, build_frame, build_xbus_frame, encode_address, xor_checksum  # noqa: F401
from .z21_locos import XBUS_LOCO_INFO_REPLY, build_loco_info_request, parse_loco_info  # noqa: F401

logger = logging.getLogger(__name__)

# LAN frame headers (2 bytes, little-endian on the wire)
LAN_SET_BROADCASTFLAGS = 0x0050
LAN_RMBUS_DATACHANGED = 0x0080
LAN_RMBUS_GETDATA = 0x0081
LAN_SYSTEMSTATE_DATACHANGED = 0x0084

# Broadcast flags (LAN_SET_BROADCASTFLAGS, spec 2.16)
BROADCAST_DRIVING_SWITCHING = 0x00000001
BROADCAST_RBUS_FEEDBACK = 0x00000002
BROADCAST_SYSTEMSTATE = 0x00000100

# R-BUS feedback: group 0 = modules 1-10, group 1 = modules 11-20. 8 inputs per module
RBUS_GROUPS = (0, 1)
RBUS_MODULES_PER_GROUP = 10
RBUS_INPUTS_PER_MODULE = 8

# X-Bus command bytes
XBUS_LOCO_DRIVE = 0xE4
XBUS_STOP = 0x80
XBUS_GET_STATUS = (0x21, 0x24, 0x05)
XBUS_TRACK_POWER_ON = (0x21, 0x81)
XBUS_STATUS_CHANGED = 0x62
XBUS_STATUS_DB0 = 0x22
XBUS_TURNOUT_INFO = 0x43  # LAN_X_TURNOUT_INFO, spec 5.3

# Central state bits (status byte in LAN_X status replies and LAN_SYSTEMSTATE_DATACHANGED)
CENTRAL_EMERGENCY_STOP = 0x01
CENTRAL_TRACK_OFF = 0x02
CENTRAL_SHORT_CIRCUIT = 0x04

# Offset of the CentralState byte in a LAN_SYSTEMSTATE_DATACHANGED frame
SYSTEMSTATE_CENTRAL_STATE_OFFSET = 16

# 128 speed step mode
SPEED_STEP_MODE_128 = 0x13

MIN_SPEED = 0
MAX_SPEED = 126
MIN_ADDRESS = 1
MAX_ADDRESS = 9999










def encode_speed_step(speed: int, forward: bool) -> int:
    """Encode a 0-126 UI speed as a 128-step speed byte.

    Step 0 is stop and step 1 is emergency stop, so UI speed 1 maps to step 2.
    Bit 7 is the direction (1 = forward).
    """
    if not (MIN_SPEED <= speed <= MAX_SPEED):
        raise ValueError(f"Speed must be {MIN_SPEED}-{MAX_SPEED}")
    step = 0 if speed == 0 else speed + 1
    return (0x80 if forward else 0x00) | step


def build_loco_drive(address: int, speed: int, forward: bool) -> bytes:
    """Build a LAN_X_SET_LOCO_DRIVE frame"""
    ah, al = encode_address(address)
    xbus = bytes([XBUS_LOCO_DRIVE, SPEED_STEP_MODE_128, ah, al, encode_speed_step(speed, forward)])
    return build_xbus_frame(xbus)


def build_emergency_stop() -> bytes:
    """Build a LAN_X_SET_STOP frame. Stops all locomotives and keeps track power on"""
    return build_xbus_frame(bytes([XBUS_STOP]))


def build_status_request() -> bytes:
    """Build a LAN_X_GET_STATUS frame"""
    return build_xbus_frame(bytes(XBUS_GET_STATUS))


def build_track_power_on() -> bytes:
    """Build a LAN_X_SET_TRACK_POWER_ON frame"""
    return build_xbus_frame(bytes(XBUS_TRACK_POWER_ON))


def parse_central_state(state: int) -> dict:
    """Decode a central state byte into flags"""
    return {
        "emergency_stop": bool(state & CENTRAL_EMERGENCY_STOP),
        "track_power_on": not (state & CENTRAL_TRACK_OFF),
        "short_circuit": bool(state & CENTRAL_SHORT_CIRCUIT),
    }


def parse_turnout_info(msb: int, lsb: int, zz: int) -> tuple[int, Optional[int]]:
    """Decode a LAN_X_TURNOUT_INFO reply into (turnout, output). Output is None unless ZZ is 01 or 10"""
    output = {1: 0, 2: 1}.get(zz & 0x03)
    return ((msb << 8) + lsb) + 1, output


def build_set_broadcast_flags(flags: int) -> bytes:
    """Build a LAN_SET_BROADCASTFLAGS frame"""
    return build_frame(LAN_SET_BROADCASTFLAGS, struct.pack('<I', flags))


def build_rbus_getdata(group: int) -> bytes:
    """Build a LAN_RMBUS_GETDATA frame to poll one R-BUS feedback group"""
    if group not in RBUS_GROUPS:
        raise ValueError(f"R-BUS group must be one of {RBUS_GROUPS}")
    return build_frame(LAN_RMBUS_GETDATA, bytes([group]))


def parse_rbus_feedback(group: int, status: bytes) -> dict:
    """Decode the 10 status bytes of an R-BUS group.

    Each byte is one module (in ascending address order), with one bit per input.
    Returns {(module_address, input_number): occupied} with inputs numbered 1-8.
    """
    if len(status) != RBUS_MODULES_PER_GROUP:
        raise ValueError(f"R-BUS status must be {RBUS_MODULES_PER_GROUP} bytes")

    states = {}
    for index, byte in enumerate(status):
        module = group * RBUS_MODULES_PER_GROUP + index + 1
        for bit in range(RBUS_INPUTS_PER_MODULE):
            states[(module, bit + 1)] = bool(byte & (1 << bit))
    return states


class Z21UDPProtocol(asyncio.DatagramProtocol):
    """UDP Protocol handler for Z21"""

    def __init__(self, driver: 'Z21Driver'):
        self.driver = driver
        self.transport = None

    def connection_made(self, transport):
        self.transport = transport
        logger.debug("UDP connection established with Z21")

    def datagram_received(self, data: bytes, addr):
        asyncio.create_task(self.driver._handle_message(data))

    def error_received(self, exc):
        logger.error(f"UDP error: {exc}")

    def connection_lost(self, exc):
        logger.info("UDP connection lost")
        self.driver.connected = False


class Z21Driver:
    """Roco Z21 Command Station Driver"""

    def __init__(self, host: str = "10.0.0.111", port: int = 21105):
        """Initialize Z21 driver

        Args:
            host: Z21 IP address
            port: Z21 port (default 21105)
        """
        self.host = host
        self.port = port
        self.connected = False
        self.transport: Optional[asyncio.DatagramTransport] = None
        self.protocol: Optional[Z21UDPProtocol] = None
        self.last_heartbeat = datetime.now()
        self.on_status_changed: Optional[Callable] = None
        # None until the Z21 reports its central state
        self.central_state: Optional[dict] = None
        # R-BUS feedback: {(module_address, input_number): occupied}
        self.feedback: dict = {}
        # Turnout outputs as the Z21 last reported them: {turnout: output or None}
        self.turnouts: dict = {}
        # Locomotive state as the Z21 last reported it: {address: {speed, direction, functions}}
        self.locos: dict = {}
        # Called with (address, speed) for every locomotive reply
        self.loco_observer: Optional[Callable] = None

    @property
    def track_state(self) -> str:
        """Overall track state: disconnected, unknown, short_circuit, track_power_off, or ready"""
        if not self.connected:
            return "disconnected"
        if self.central_state is None:
            return "unknown"
        if self.central_state["short_circuit"]:
            return "short_circuit"
        if not self.central_state["track_power_on"]:
            return "track_power_off"
        return "ready"

    async def connect(self) -> bool:
        """Connect to Z21 system using UDP

        UDP has no handshake, so this only opens the socket. The connection is
        confirmed by the first reply from the Z21, not by this call.

        Returns:
            True if the socket was opened
        """
        try:
            logger.info(f"Connecting to Z21 at {self.host}:{self.port} (UDP)")

            # Close any existing socket so this can be used to reconnect
            if self.transport:
                self.transport.close()
            self.central_state = None

            loop = asyncio.get_running_loop()
            self.transport, self.protocol = await loop.create_datagram_endpoint(
                lambda: Z21UDPProtocol(self),
                remote_addr=(self.host, self.port)
            )

            self.connected = True
            logger.info("Connected to Z21 via UDP")

            await self.get_status()
            # Broadcast flags are per client and must be set after every connect
            await self._send_message(build_set_broadcast_flags(
                BROADCAST_DRIVING_SWITCHING | BROADCAST_RBUS_FEEDBACK | BROADCAST_SYSTEMSTATE
            ))
            for group in RBUS_GROUPS:
                await self._send_message(build_rbus_getdata(group))

            return True
        except Exception as e:
            logger.error(f"Failed to connect to Z21: {e}")
            self.connected = False
            return False

    async def disconnect(self):
        """Disconnect from Z21"""
        if self.transport:
            self.transport.close()
        self.connected = False
        self.central_state = None
        self.feedback = {}
        self.turnouts = {}
        self.locos = {}
        logger.info("Disconnected from Z21")

    async def get_status(self):
        """Request Z21 status"""
        if not self.connected:
            return
        await self._send_message(build_status_request())

    async def request_feedback(self, group: int):
        """Poll one R-BUS feedback group. The reply arrives asynchronously"""
        if not self.connected:
            return
        await self._send_message(build_rbus_getdata(group))

    async def set_loco_speed(self, address: int, speed: int, direction: bool = True) -> bool:
        """Set locomotive speed

        Args:
            address: Locomotive DCC address (1-9999)
            speed: Speed 0-126 (126 = max speed)
            direction: True = forward, False = reverse

        Returns:
            True if the command was sent
        """
        if not self.connected:
            logger.warning("Not connected to Z21")
            return False

        try:
            await self._send_message(build_loco_drive(address, speed, direction))
            logger.debug(f"Set loco {address} speed={speed} direction={direction}")
            return True
        except ValueError as e:
            logger.error(f"Invalid locomotive command: {e}")
            return False

    async def get_loco_info(self, address: int):
        """Request locomotive information from Z21. The reply arrives asynchronously.

        Args:
            address: Locomotive DCC address

        Returns:
            Dictionary with request status, or None if the request failed
        """
        if not self.connected:
            return None

        try:
            await self._send_message(build_loco_info_request(address))
            return {"address": address, "status": "requested"}
        except ValueError as e:
            logger.error(f"Invalid locomotive address: {e}")
            return None

    async def emergency_stop(self) -> bool:
        """Stop all locomotives. Track power stays on.

        Returns:
            True if the command was sent
        """
        if not self.connected:
            return False

        await self._send_message(build_emergency_stop())
        logger.warning("Emergency stop sent")
        return True

    async def track_power_on(self) -> bool:
        """Turn track power on. Only call this on an explicit user request.

        Returns:
            True if the command was sent
        """
        if not self.connected:
            return False

        await self._send_message(build_track_power_on())
        logger.info("Track power on sent")
        return True

    async def _send_message(self, message: bytes):
        """Send a frame to Z21 via UDP

        Args:
            message: Frame bytes to send
        """
        if not self.transport:
            logger.error("Transport not available")
            return

        try:
            self.transport.sendto(message)
        except Exception as e:
            logger.error(f"Error sending message to Z21: {e}")
            self.connected = False

    async def _handle_message(self, message: bytes):
        """Handle a frame received from Z21

        Args:
            message: Frame bytes from Z21
        """
        if len(message) < 4:
            return

        try:
            _, header = struct.unpack_from('<HH', message)
            self.last_heartbeat = datetime.now()

            if header == LAN_X_HEADER and len(message) >= 7 \
                    and message[4] == XBUS_STATUS_CHANGED and message[5] == XBUS_STATUS_DB0:
                # Reply to LAN_X_GET_STATUS
                await self._apply_central_state(message[6])

            elif header == LAN_X_HEADER and len(message) >= 13 and message[4] == XBUS_LOCO_INFO_REPLY:
                await self._apply_loco_info(message[4:-1])

            elif header == LAN_X_HEADER and len(message) >= 9 and message[4] == XBUS_TURNOUT_INFO:
                await self._apply_turnout_info(message[5], message[6], message[7])

            elif header == LAN_SYSTEMSTATE_DATACHANGED:
                logger.debug("Z21 system state changed")
                if len(message) > SYSTEMSTATE_CENTRAL_STATE_OFFSET:
                    await self._apply_central_state(message[SYSTEMSTATE_CENTRAL_STATE_OFFSET])
                if self.on_status_changed:
                    await self.on_status_changed({"status": "system_state_changed"})

            elif header == LAN_RMBUS_DATACHANGED:
                logger.debug("Z21 feedback data received")
                if len(message) >= 15:
                    await self._apply_feedback(message[4], message[5:15])
                if self.on_status_changed:
                    await self.on_status_changed({"status": "feedback_received"})

            else:
                logger.debug(f"Z21 message header: 0x{header:04x}")

        except Exception as e:
            logger.error(f"Error handling message: {e}")

    async def _apply_central_state(self, state: int):
        """Store a new central state and notify listeners if it changed"""
        new_state = parse_central_state(state)
        if new_state == self.central_state:
            return

        self.central_state = new_state
        logger.info(f"Z21 track state: {self.track_state}")
        if self.on_status_changed:
            await self.on_status_changed({
                "status": "central_state_changed",
                "track_state": self.track_state,
            })

    async def _apply_feedback(self, group: int, status: bytes):
        """Store R-BUS feedback for one group and notify listeners of changed inputs"""
        new_states = parse_rbus_feedback(group, status)
        changes = {
            key: occupied
            for key, occupied in new_states.items()
            if self.feedback.get(key) != occupied
        }
        self.feedback.update(new_states)
        if changes and self.on_status_changed:
            await self.on_status_changed({
                "status": "feedback_changed",
                "changes": changes,
            })

    async def _apply_turnout_info(self, msb: int, lsb: int, zz: int):
        """Store one turnout's reported output and notify listeners if it changed"""
        turnout, output = parse_turnout_info(msb, lsb, zz)
        if self.turnouts.get(turnout, "unknown") == output:
            return
        self.turnouts[turnout] = output
        if self.on_status_changed:
            await self.on_status_changed({"status": "turnout_changed", "turnout": turnout, "output": output})

    async def _apply_loco_info(self, data: bytes):
        """Store a locomotive's reported speed, direction, and functions. Notify listeners if they changed"""
        info = parse_loco_info(data)
        if info is None:
            return
        if self.loco_observer:
            self.loco_observer(info["address"], info["speed"])
        state = {key: info[key] for key in ("speed", "direction", "functions")}
        if self.locos.get(info["address"]) == state:
            return
        self.locos[info["address"]] = state
        if self.on_status_changed:
            await self.on_status_changed({"status": "loco_changed", "address": info["address"]})

    def get_connection_status(self) -> dict:
        """Get Z21 connection status

        Returns:
            Dictionary with connection and track state information
        """
        return {
            "connected": self.connected,
            "state": self.track_state,
            "central_state": self.central_state,
            "host": self.host,
            "port": self.port,
            "last_heartbeat": self.last_heartbeat.isoformat(),
        }


# Global Z21 instance
_z21_driver: Optional[Z21Driver] = None


async def get_z21_driver(host: str = "10.0.0.111", port: int = 21105) -> Z21Driver:
    """Get or create Z21 driver instance"""
    global _z21_driver

    if _z21_driver is None:
        _z21_driver = Z21Driver(host, port)

    return _z21_driver
