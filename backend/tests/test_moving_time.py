"""Moving time: seconds at a speed above zero, saved to the locomotive's travelled_time"""

from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from app.database import Base
from app.models.locomotive import Locomotive
from app.services import moving_time
from app.services.moving_time import MovingTimeTracker


class Clock:
    def __init__(self):
        self.now = 1000.0

    def __call__(self):
        return self.now


def _setup():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(bind=engine)
    session_factory = sessionmaker(bind=engine)
    db = session_factory()
    db.add(Locomotive(name="Class 37", dcc_address=3, locomotive_type="diesel", travelled_time=0))
    db.commit()
    db.close()
    clock = Clock()
    return MovingTimeTracker(session_factory, clock), clock, session_factory


def _time(session_factory, address):
    db = session_factory()
    try:
        return db.query(Locomotive).filter(Locomotive.dcc_address == address).first().travelled_time
    finally:
        db.close()


def test_time_is_saved_when_the_locomotive_stops():
    tracker, clock, sessions = _setup()
    tracker.observe(3, 40)
    clock.now += 25
    tracker.observe(3, 0)
    assert _time(sessions, 3) == 25


def test_stopped_locomotive_counts_nothing():
    tracker, clock, sessions = _setup()
    tracker.observe(3, 0)
    clock.now += 60
    tracker.observe(3, None)
    assert _time(sessions, 3) == 0


def test_repeated_moving_reports_do_not_restart_the_clock():
    tracker, clock, sessions = _setup()
    tracker.observe(3, 20)
    clock.now += 10
    tracker.observe(3, 35)
    clock.now += 10
    tracker.observe(3, 0)
    assert _time(sessions, 3) == 20


def test_flush_saves_running_time_without_double_counting():
    tracker, clock, sessions = _setup()
    tracker.observe(3, 20)
    clock.now += moving_time.FLUSH_SECONDS
    tracker.flush_if_due()
    assert _time(sessions, 3) == moving_time.FLUSH_SECONDS
    clock.now += 5
    tracker.observe(3, 0)
    assert _time(sessions, 3) == moving_time.FLUSH_SECONDS + 5


def test_flush_waits_for_the_interval():
    tracker, clock, sessions = _setup()
    tracker.observe(3, 20)
    clock.now += 5
    tracker.flush_if_due()
    assert _time(sessions, 3) == 0


def test_unknown_address_is_ignored():
    tracker, clock, sessions = _setup()
    tracker.observe(99, 20)
    clock.now += 10
    tracker.observe(99, 0)
    assert _time(sessions, 3) == 0
