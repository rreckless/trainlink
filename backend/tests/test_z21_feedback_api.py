"""
API tests for GET /api/z21/feedback.

The driver is replaced with one that has a fake transport, so the feedback
state can be set directly and the polls can be checked.
"""

import pytest
from fastapi.testclient import TestClient

from app.api.z21 import get_z21
from app.drivers.z21_driver import Z21Driver, build_rbus_getdata
from app.main import app


class FakeTransport:
    def __init__(self):
        self.sent: list[bytes] = []

    def sendto(self, data):
        self.sent.append(data)

    def close(self):
        pass


def _api_driver(connected: bool = True) -> Z21Driver:
    driver = Z21Driver()
    driver.transport = FakeTransport()
    driver.connected = connected
    return driver


@pytest.fixture
def client():
    yield TestClient(app)
    app.dependency_overrides.clear()


def _use_driver(driver: Z21Driver):
    async def override():
        return driver

    app.dependency_overrides[get_z21] = override


def test_feedback_lists_inputs_sorted_with_state(client):
    driver = _api_driver()
    driver.feedback = {(13, 8): True, (11, 1): False, (11, 2): True}
    _use_driver(driver)

    body = client.get("/api/z21/feedback").json()

    assert body["connected"] is True
    assert body["inputs"] == [
        {"module": 11, "input": 1, "occupied": False},
        {"module": 11, "input": 2, "occupied": True},
        {"module": 13, "input": 8, "occupied": True},
    ]


def test_feedback_polls_both_groups(client):
    driver = _api_driver()
    _use_driver(driver)

    client.get("/api/z21/feedback")

    assert build_rbus_getdata(0) in driver.transport.sent
    assert build_rbus_getdata(1) in driver.transport.sent


def test_feedback_when_disconnected(client):
    driver = _api_driver(connected=False)
    _use_driver(driver)

    body = client.get("/api/z21/feedback").json()

    assert body == {"connected": False, "inputs": []}
    assert driver.transport.sent == []
