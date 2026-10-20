"""
Moving time per locomotive: the seconds a locomotive is at a speed above zero.

Time is counted from the speed the Z21 reports, and from the speed commands this server sends. It is
kept in memory while the locomotive moves and saved to its travelled_time when it stops, and at most
every FLUSH_SECONDS while it moves. A crash loses at most the current run since the last save.
"""

import time
from typing import Callable, Optional

from app.database import SessionLocal
from app.models.locomotive import Locomotive

FLUSH_SECONDS = 30


class MovingTimeTracker:
    def __init__(self, session_factory: Callable = SessionLocal, clock: Callable[[], float] = time.monotonic):
        self._session_factory = session_factory
        self._clock = clock
        # address -> clock time when the locomotive started moving, or was last saved
        self._since: dict[int, float] = {}
        self._last_flush = clock()

    def observe(self, address: int, speed: Optional[int]):
        """Record the locomotive's current speed. Speed None (unknown) counts as not moving"""
        now = self._clock()
        moving = bool(speed and speed > 0)
        if moving and address not in self._since:
            self._since[address] = now
        elif not moving and address in self._since:
            self._save(address, now - self._since.pop(address))

    def flush_if_due(self):
        """Save the time of moving locomotives, at most every FLUSH_SECONDS"""
        now = self._clock()
        if now - self._last_flush < FLUSH_SECONDS:
            return
        self._last_flush = now
        for address in list(self._since):
            self._save(address, now - self._since[address])
            self._since[address] = now

    def _save(self, address: int, seconds: float):
        if seconds <= 0:
            return
        db = self._session_factory()
        try:
            locomotive = db.query(Locomotive).filter(Locomotive.dcc_address == address).first()
            if locomotive is not None:
                locomotive.travelled_time = (locomotive.travelled_time or 0) + seconds
                db.commit()
        finally:
            db.close()


# The server's tracker, shared by the Z21 driver and the speed route
tracker = MovingTimeTracker()
