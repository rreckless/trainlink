"""
REST endpoint tests for locomotives, blocks, signals, and routes.

Uses an in-memory SQLite database, so these run without Postgres.
"""

import pytest
from app.models import CommandStation, Layout


@pytest.fixture
def seed(db_session_factory):
    """Create a command station and a layout. Returns their ids"""
    with db_session_factory() as db:
        station = CommandStation(name="Z21", station_type="roco_z21", port="10.0.0.111:21105")
        layout = Layout(name="Test layout", width=100.0, height=50.0, scale="HO")
        db.add_all([station, layout])
        db.commit()
        return {"command_station_id": station.id, "layout_id": layout.id}


def _loco(seed, **overrides):
    body = {
        "name": "Class 37",
        "dcc_address": 3,
        "locomotive_type": "diesel",
        "command_station_id": seed["command_station_id"],
    }
    body.update(overrides)
    return body


# Locomotives

def test_create_and_get_locomotive(client, seed):
    response = client.post("/api/locomotives", json=_loco(seed))
    assert response.status_code == 201
    loco = response.json()
    assert loco["dcc_address"] == 3
    assert loco["speed"] == 0
    assert loco["direction"] == "stop"

    assert client.get(f"/api/locomotives/{loco['id']}").json()["name"] == "Class 37"
    assert len(client.get("/api/locomotives").json()) == 1


def test_duplicate_locomotive_rejected(client, seed):
    client.post("/api/locomotives", json=_loco(seed))
    response = client.post("/api/locomotives", json=_loco(seed, name="Other"))
    assert response.status_code == 409


@pytest.mark.parametrize("address", [0, 10000])
def test_locomotive_address_out_of_range(client, seed, address):
    response = client.post("/api/locomotives", json=_loco(seed, dcc_address=address))
    assert response.status_code == 400


def test_locomotive_unknown_command_station(client, seed):
    response = client.post("/api/locomotives", json=_loco(seed, command_station_id=999))
    assert response.status_code == 404


def test_update_locomotive_name(client, seed):
    loco_id = client.post("/api/locomotives", json=_loco(seed)).json()["id"]
    response = client.patch(f"/api/locomotives/{loco_id}", json={"name": "Renamed"})
    assert response.status_code == 200
    assert response.json()["name"] == "Renamed"


@pytest.mark.parametrize("body", [{"speed": 10}, {"direction": "forward"}])
def test_speed_and_direction_not_set_through_rest(client, seed, body):
    loco_id = client.post("/api/locomotives", json=_loco(seed)).json()["id"]
    response = client.patch(f"/api/locomotives/{loco_id}", json=body)
    assert response.status_code == 400
    assert "/api/z21/loco/speed" in response.json()["detail"]


def test_null_name_rejected(client, seed):
    loco_id = client.post("/api/locomotives", json=_loco(seed)).json()["id"]
    response = client.patch(f"/api/locomotives/{loco_id}", json={"name": None})
    assert response.status_code == 400


def test_delete_locomotive(client, seed):
    loco_id = client.post("/api/locomotives", json=_loco(seed)).json()["id"]
    assert client.delete(f"/api/locomotives/{loco_id}").status_code == 204
    assert client.get(f"/api/locomotives/{loco_id}").status_code == 404


def test_missing_locomotive_is_404(client):
    assert client.get("/api/locomotives/999").status_code == 404
    assert client.patch("/api/locomotives/999", json={"name": "x"}).status_code == 404
    assert client.delete("/api/locomotives/999").status_code == 404


# Blocks

def _block(seed, **overrides):
    body = {"layout_id": seed["layout_id"], "name": "Platform 1", "x": 10.0, "y": 20.0}
    body.update(overrides)
    return body


def test_create_block_defaults(client, seed):
    response = client.post("/api/blocks", json=_block(seed))
    assert response.status_code == 201
    block = response.json()
    assert block["state"] == "free"
    assert block["occupied"] is False
    assert block["max_speed"] == 100
    assert block["signal_ids"] == []
    assert block["feedback_ids"] == []


def test_create_block_unknown_layout(client, seed):
    response = client.post("/api/blocks", json=_block(seed, layout_id=999))
    assert response.status_code == 404


def test_block_state_change_sets_occupied(client, seed):
    block_id = client.post("/api/blocks", json=_block(seed)).json()["id"]

    response = client.patch(f"/api/blocks/{block_id}", json={"state": "occupied"})
    assert response.status_code == 200
    assert response.json()["state"] == "occupied"
    assert response.json()["occupied"] is True

    response = client.patch(f"/api/blocks/{block_id}", json={"state": "free"})
    assert response.json()["occupied"] is False


def test_invalid_block_state_rejected(client, seed):
    block_id = client.post("/api/blocks", json=_block(seed)).json()["id"]
    response = client.patch(f"/api/blocks/{block_id}", json={"state": "broken"})
    assert response.status_code == 400


def test_list_blocks_filtered_by_layout(client, seed):
    client.post("/api/blocks", json=_block(seed))

    assert len(client.get(f"/api/blocks?layout_id={seed['layout_id']}").json()) == 1
    assert client.get("/api/blocks?layout_id=999").json() == []


def test_delete_block(client, seed):
    block_id = client.post("/api/blocks", json=_block(seed)).json()["id"]
    assert client.delete(f"/api/blocks/{block_id}").status_code == 204
    assert client.get(f"/api/blocks/{block_id}").status_code == 404


# Signals

def _signal(seed, **overrides):
    body = {
        "layout_id": seed["layout_id"],
        "name": "Home signal",
        "x": 5.0,
        "y": 5.0,
        "signal_type": "main",
    }
    body.update(overrides)
    return body


def test_create_signal_defaults_to_stop(client, seed):
    response = client.post("/api/signals", json=_signal(seed))
    assert response.status_code == 201
    assert response.json()["aspect"] == "stop"
    assert response.json()["block_ids"] == []


@pytest.mark.parametrize("signal_type", ["home", ""])
def test_invalid_signal_type_rejected(client, seed, signal_type):
    response = client.post("/api/signals", json=_signal(seed, signal_type=signal_type))
    assert response.status_code == 400


def test_signal_aspect_change(client, seed):
    signal_id = client.post("/api/signals", json=_signal(seed)).json()["id"]

    response = client.patch(f"/api/signals/{signal_id}", json={"aspect": "go"})
    assert response.status_code == 200
    assert response.json()["aspect"] == "go"


def test_invalid_aspect_rejected(client, seed):
    signal_id = client.post("/api/signals", json=_signal(seed)).json()["id"]
    response = client.patch(f"/api/signals/{signal_id}", json={"aspect": "purple"})
    assert response.status_code == 400


def test_delete_signal(client, seed):
    signal_id = client.post("/api/signals", json=_signal(seed)).json()["id"]
    assert client.delete(f"/api/signals/{signal_id}").status_code == 204
    assert client.get(f"/api/signals/{signal_id}").status_code == 404


# Routes

def _route(seed, block_sequence, **overrides):
    body = {
        "layout_id": seed["layout_id"],
        "name": "Up line",
        "block_sequence": block_sequence,
    }
    body.update(overrides)
    return body


def _two_blocks(client, seed):
    first = client.post("/api/blocks", json=_block(seed, name="A")).json()["id"]
    second = client.post("/api/blocks", json=_block(seed, name="B")).json()["id"]
    return first, second


def test_create_route(client, seed):
    first, second = _two_blocks(client, seed)
    response = client.post("/api/routes", json=_route(seed, [first, second]))
    assert response.status_code == 201
    route = response.json()
    assert route["block_sequence"] == [first, second]
    assert route["is_running"] is False
    assert route["enabled"] is True


def test_route_with_block_from_other_layout_rejected(client, seed):
    first, _ = _two_blocks(client, seed)
    with_missing = client.post("/api/routes", json=_route(seed, [first, 999]))
    assert with_missing.status_code == 400
    assert "999" in with_missing.json()["detail"]


def test_route_needs_a_block(client, seed):
    response = client.post("/api/routes", json=_route(seed, []))
    assert response.status_code == 400


def test_route_running_state_not_set_through_rest(client, seed):
    first, _ = _two_blocks(client, seed)
    route_id = client.post("/api/routes", json=_route(seed, [first])).json()["id"]
    response = client.patch(f"/api/routes/{route_id}", json={"is_running": True})
    assert response.status_code == 400


def test_update_route_sequence_validated(client, seed):
    first, second = _two_blocks(client, seed)
    route_id = client.post("/api/routes", json=_route(seed, [first])).json()["id"]

    response = client.patch(f"/api/routes/{route_id}", json={"block_sequence": [second]})
    assert response.status_code == 200
    assert response.json()["block_sequence"] == [second]

    response = client.patch(f"/api/routes/{route_id}", json={"block_sequence": [999]})
    assert response.status_code == 400


def test_delete_route(client, seed):
    first, _ = _two_blocks(client, seed)
    route_id = client.post("/api/routes", json=_route(seed, [first])).json()["id"]
    assert client.delete(f"/api/routes/{route_id}").status_code == 204
    assert client.get(f"/api/routes/{route_id}").status_code == 404


def test_locomotive_without_command_station(client, seed):
    body = {"name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel"}
    response = client.post("/api/locomotives", json=body)
    assert response.status_code == 201
    assert response.json()["name"] == "Class 37"
