"""
Shared test fixtures.

MockZ21 is a UDP responder on localhost. It records every frame the driver
sends and can send frames back, so the driver can be tested without hardware.
"""

import socket
import threading
import time

import pytest


class MockZ21:
    """Local UDP stand-in for a Roco Z21"""

    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.bind(("127.0.0.1", 0))
        self.sock.settimeout(0.1)
        self.port = self.sock.getsockname()[1]
        self.host = "127.0.0.1"
        self.received: list[bytes] = []
        self.client_addr = None
        self._lock = threading.Lock()
        self._running = True
        self._thread = threading.Thread(target=self._receive_loop, daemon=True)
        self._thread.start()

    def _receive_loop(self):
        while self._running:
            try:
                data, addr = self.sock.recvfrom(2048)
            except socket.timeout:
                continue
            except OSError:
                break
            with self._lock:
                self.received.append(data)
                self.client_addr = addr

    def sent_frames(self) -> list[bytes]:
        """Return a copy of every frame received so far"""
        with self._lock:
            return list(self.received)

    def wait_for_frames(self, count: int, timeout: float = 1.0) -> bool:
        """Wait until at least `count` frames have arrived"""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if len(self.sent_frames()) >= count:
                return True
            time.sleep(0.01)
        return len(self.sent_frames()) >= count

    def send_to_client(self, frame: bytes):
        """Send a frame from the Z21 to the driver"""
        if self.client_addr is None:
            raise RuntimeError("Driver has not sent anything yet")
        self.sock.sendto(frame, self.client_addr)

    def close(self):
        self._running = False
        self.sock.close()
        self._thread.join(timeout=1)


@pytest.fixture
def mock_z21():
    mock = MockZ21()
    yield mock
    mock.close()
