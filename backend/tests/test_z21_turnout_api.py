"""Turnout endpoints: commands are refused unless the track is ready, and bad input is rejected"""

import pytest
from fastapi.testclient import TestClient

from app.api.z21 import get_z21
from app.drivers import z21_turnouts
from app.drivers.z21_driver import Z21Driver
from app.main import app


class FakeTransport:
    def __init__(self):
        self.sent = []

    def sendto(self, data):
        self.sent.append(data)

    def close(self):
        pass


def _driver(connected: bool = True, track_on: bool = True) -> Z21Driver:
    driver = Z21Driver()
    driver.transport = FakeTransport()
    driver.connected = connected
    if connected:
        driver.central_state = {"emergency_stop": False, "track_power_on": track_on, "short_circuit": False}
    return driver


@pytest.fixture
def client():
    yield TestClient(app)
    app.dependency_overrides.clear()


def _use_driver(driver: Z21Driver):
    async def override():
        return driver

    app.dependency_overrides[get_z21] = override


def test_throw_refused_when_not_connected(client):
    _use_driver(_driver(connected=False))
    assert client.post("/api/z21/turnouts/25/throw", json={"output": 1}).status_code == 503


def test_throw_refused_when_track_power_is_off(client):
    _use_driver(_driver(track_on=False))
    assert client.post("/api/z21/turnouts/25/throw", json={"output": 1}).status_code == 409


def test_throw_sends_the_command_when_ready(client, monkeypatch):
    monkeypatch.setattr(z21_turnouts, "THROW_SECONDS", 0)
    driver = _driver()
    _use_driver(driver)
    response = client.post("/api/z21/turnouts/25/throw", json={"output": 1})
    assert response.status_code == 200
    assert response.json() == {"status": "throw_sent", "turnout": 25, "output": 1}
    # Activate, then deactivate
    assert driver.transport.sent == [
        bytes.fromhex("09004000" "530018A9E2"),
        bytes.fromhex("09004000" "530018A1EA"),
    ]


@pytest.mark.parametrize("turnout", [0, 2049])
def test_throw_rejects_turnouts_out_of_range(client, turnout):
    _use_driver(_driver())
    assert client.post(f"/api/z21/turnouts/{turnout}/throw", json={"output": 0}).status_code == 400


def test_throw_rejects_bad_output(client):
    _use_driver(_driver())
    assert client.post("/api/z21/turnouts/25/throw", json={"output": 2}).status_code == 422


def test_states_asks_the_z21_and_returns_known_states(client):
    driver = _driver()
    driver.turnouts = {25: 1}
    _use_driver(driver)
    body = client.get("/api/z21/turnouts", params={"addresses": "1,25"}).json()
    assert body == {"connected": True, "states": {"1": None, "25": 1}}
    assert len(driver.transport.sent) == 2


def test_states_rejects_bad_turnouts(client):
    assert client.get("/api/z21/turnouts", params={"addresses": "abc"}).status_code == 400
    assert client.get("/api/z21/turnouts", params={"addresses": "0"}).status_code == 400
    assert client.get("/api/z21/turnouts", params={"addresses": "2049"}).status_code == 400


def test_states_reports_unknown_when_not_connected(client):
    _use_driver(_driver(connected=False))
    body = client.get("/api/z21/turnouts", params={"addresses": "1,25"}).json()
    assert body == {"connected": False, "states": {"1": None, "25": None}}
