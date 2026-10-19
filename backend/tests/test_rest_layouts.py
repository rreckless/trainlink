"""
REST tests for layouts and feedbacks, and positions on blocks and signals.
"""

import pytest

from app.models import CommandStation, Layout


@pytest.fixture
def layout_id(client):
    response = client.post(
        "/api/layouts",
        json={"name": "Main", "width": 800, "height": 600, "scale": "HO"},
    )
    assert response.status_code == 201
    return response.json()["id"]


def _track(x1=0, y1=0, x2=40, y2=0):
    return {"x1": x1, "y1": y1, "x2": x2, "y2": y2}


# Layouts

def test_create_layout_starts_with_no_tracks(client):
    response = client.post(
        "/api/layouts",
        json={"name": "Yard", "width": 100, "height": 50, "scale": "N", "description": "Test"},
    )
    assert response.status_code == 201
    body = response.json()
    assert body["elements"] == {"tracks": []}
    assert body["description"] == "Test"


def test_duplicate_layout_name_rejected(client, layout_id):
    response = client.post(
        "/api/layouts",
        json={"name": "Main", "width": 10, "height": 10, "scale": "HO"},
    )
    assert response.status_code == 409


@pytest.mark.parametrize("width,height", [(0, 10), (10, -1)])
def test_layout_size_must_be_positive(client, width, height):
    response = client.post(
        "/api/layouts",
        json={"name": "Bad", "width": width, "height": height, "scale": "HO"},
    )
    assert response.status_code == 400


def test_save_tracks_to_layout(client, layout_id):
    tracks = [_track(), _track(40, 0, 40, 40)]
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": {"tracks": tracks}})
    assert response.status_code == 200
    assert response.json()["elements"]["tracks"] == tracks

    reloaded = client.get(f"/api/layouts/{layout_id}").json()
    assert reloaded["elements"]["tracks"] == tracks


def test_track_with_missing_coordinate_rejected(client, layout_id):
    bad = {"x1": 0, "y1": 0, "x2": 40}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": {"tracks": [bad]}})
    assert response.status_code == 400


def test_tracks_must_be_a_list(client, layout_id):
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": {"tracks": "nope"}})
    assert response.status_code == 400


def test_update_layout_size_checked_against_current_value(client, layout_id):
    response = client.patch(f"/api/layouts/{layout_id}", json={"width": -5})
    assert response.status_code == 400


def test_null_layout_name_rejected(client, layout_id):
    response = client.patch(f"/api/layouts/{layout_id}", json={"name": None})
    assert response.status_code == 400


def test_list_and_get_layouts(client, layout_id):
    assert [item["id"] for item in client.get("/api/layouts").json()] == [layout_id]
    assert client.get(f"/api/layouts/{layout_id}").json()["name"] == "Main"
    assert client.get("/api/layouts/999").status_code == 404


def test_delete_empty_layout(client, layout_id):
    assert client.delete(f"/api/layouts/{layout_id}").status_code == 204
    assert client.get(f"/api/layouts/{layout_id}").status_code == 404


def test_delete_layout_with_blocks_is_refused(client, layout_id):
    client.post(
        "/api/blocks",
        json={"layout_id": layout_id, "name": "A", "x": 0, "y": 0},
    )
    response = client.delete(f"/api/layouts/{layout_id}")
    assert response.status_code == 409
    assert client.get(f"/api/layouts/{layout_id}").status_code == 200


# Positions on blocks and signals

def test_block_position_can_be_moved(client, layout_id):
    block_id = client.post(
        "/api/blocks",
        json={"layout_id": layout_id, "name": "A", "x": 10, "y": 20},
    ).json()["id"]

    response = client.patch(f"/api/blocks/{block_id}", json={"x": 50, "y": 60})
    assert response.status_code == 200
    assert response.json()["x"] == 50
    assert response.json()["y"] == 60


def test_block_position_cannot_be_null(client, layout_id):
    block_id = client.post(
        "/api/blocks",
        json={"layout_id": layout_id, "name": "A", "x": 10, "y": 20},
    ).json()["id"]
    response = client.patch(f"/api/blocks/{block_id}", json={"x": None})
    assert response.status_code == 400


def test_signal_position_can_be_moved(client, layout_id):
    signal_id = client.post(
        "/api/signals",
        json={
            "layout_id": layout_id,
            "name": "S1",
            "x": 1,
            "y": 2,
            "signal_type": "main",
        },
    ).json()["id"]

    response = client.patch(f"/api/signals/{signal_id}", json={"x": 7, "y": 8})
    assert response.status_code == 200
    assert response.json()["x"] == 7


# Feedbacks

def _feedback(layout_id, **overrides):
    body = {
        "layout_id": layout_id,
        "name": "F1",
        "x": 100,
        "y": 100,
        "feedback_number": 1,
    }
    body.update(overrides)
    return body


def test_create_and_list_feedbacks(client, layout_id):
    response = client.post("/api/feedbacks", json=_feedback(layout_id))
    assert response.status_code == 201
    feedback = response.json()
    assert feedback["state"] is False
    assert feedback["block_ids"] == []

    listed = client.get(f"/api/feedbacks?layout_id={layout_id}").json()
    assert [item["id"] for item in listed] == [feedback["id"]]
    assert client.get("/api/feedbacks?layout_id=999").json() == []


def test_feedback_unknown_layout(client):
    response = client.post("/api/feedbacks", json=_feedback(999))
    assert response.status_code == 404


def test_feedback_position_and_state_update(client, layout_id):
    feedback_id = client.post("/api/feedbacks", json=_feedback(layout_id)).json()["id"]

    response = client.patch(
        f"/api/feedbacks/{feedback_id}",
        json={"x": 5, "y": 6, "state": True},
    )
    assert response.status_code == 200
    body = response.json()
    assert (body["x"], body["y"], body["state"]) == (5, 6, True)


def test_feedback_null_position_rejected(client, layout_id):
    feedback_id = client.post("/api/feedbacks", json=_feedback(layout_id)).json()["id"]
    response = client.patch(f"/api/feedbacks/{feedback_id}", json={"y": None})
    assert response.status_code == 400


def test_feedback_block_ids_follow_block_links(client, layout_id, db_session_factory):
    feedback_id = client.post("/api/feedbacks", json=_feedback(layout_id)).json()["id"]
    block_id = client.post(
        "/api/blocks",
        json={"layout_id": layout_id, "name": "A", "x": 0, "y": 0},
    ).json()["id"]

    from app.models import Block, Feedback

    with db_session_factory() as db:
        feedback = db.get(Feedback, feedback_id)
        feedback.blocks.append(db.get(Block, block_id))
        db.commit()

    assert client.get(f"/api/feedbacks/{feedback_id}").json()["block_ids"] == [block_id]


def test_delete_feedback(client, layout_id):
    feedback_id = client.post("/api/feedbacks", json=_feedback(layout_id)).json()["id"]
    assert client.delete(f"/api/feedbacks/{feedback_id}").status_code == 204
    assert client.get(f"/api/feedbacks/{feedback_id}").status_code == 404
