"""
Track library tests: loading, the read-only API, and layout validation of placed pieces.
"""

import pytest

from app.services.track_library import list_pieces, load_library

SETRACK = "Setrack (Code 80)"
STREAMLINE = "Streamline (Code 55)"


@pytest.fixture
def layout_id(client):
    response = client.post(
        "/api/layouts",
        json={"name": "Piece test", "width": 800, "height": 600, "scale": "N"},
    )
    return response.json()["id"]


def _placed(piece_id, placed_id="p1", **overrides):
    piece = {"id": placed_id, "piece_id": piece_id, "x": 0, "y": 0, "rotation": 0, "flip": False}
    piece.update(overrides)
    return piece


def test_library_loads_peco_pieces():
    library = load_library()
    assert len(library) == 46
    assert library["peco-n-st-1"]["name"] == "Standard Straight"
    assert library["peco-n-st-1"]["system"] == SETRACK
    assert library["peco-n-st-1"]["verified"] is False


def test_library_pieces_carry_their_geometry():
    piece = load_library()["peco-n-st-5"]
    assert piece["type"] == "turnout"
    assert piece["hand"] == "right"
    assert piece["length"] == 87
    assert piece["radius"] == 228
    assert piece["angle"] == 22.5


def test_sl_u395f_is_placeable_with_frog_angle():
    piece = load_library()["peco-n-sl-u395f"]
    assert piece["length"] == 137
    assert piece["radius"] == 457
    assert piece["angle"] == 10
    assert piece["angle_meaning"] == "frog"
    assert piece["placeable"] is True
    assert piece["verified"] is False


def test_sl_u396f_is_placeable_left_hand_with_frog_angle():
    piece = load_library()["peco-n-sl-u396f"]
    assert piece["hand"] == "left"
    assert piece["length"] == 137
    assert piece["radius"] == 457
    assert piece["angle_meaning"] == "frog"
    assert piece["placeable"] is True


def test_list_pieces_filters_by_system():
    setrack = list_pieces(system=SETRACK)
    streamline = list_pieces(system=STREAMLINE)
    assert setrack and streamline
    assert all(piece["system"] == SETRACK for piece in setrack)
    assert len(setrack) + len(streamline) == 46


def test_library_endpoint_lists_and_filters(client):
    everything = client.get("/api/track-library").json()
    assert len(everything) == 46

    setrack = client.get("/api/track-library", params={"system": SETRACK}).json()
    assert all(piece["system"] == SETRACK for piece in setrack)


def test_library_piece_endpoint(client):
    response = client.get("/api/track-library/peco-n-st-3")
    assert response.status_code == 200
    assert response.json()["radius"] == 228

    assert client.get("/api/track-library/no-such-piece").status_code == 404


def test_layout_accepts_placed_pieces(client, layout_id):
    elements = {
        "tracks": [],
        "track_system": SETRACK,
        "track_pieces": [_placed("peco-n-st-1"), _placed("peco-n-st-3", "p2", x=87, flip=True)],
    }
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200
    assert len(response.json()["elements"]["track_pieces"]) == 2


def test_unknown_piece_rejected(client, layout_id):
    elements = {"track_system": SETRACK, "track_pieces": [_placed("peco-n-nothing")]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


def test_mixing_track_systems_rejected(client, layout_id):
    elements = {
        "track_system": SETRACK,
        "track_pieces": [
            _placed("peco-n-st-1", "p1"),
            _placed("peco-n-sl-e388f", "p2"),
        ],
    }
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400
    assert "mix" in response.json()["detail"]


def test_track_system_must_match_pieces(client, layout_id):
    elements = {"track_system": STREAMLINE, "track_pieces": [_placed("peco-n-st-1")]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


@pytest.mark.parametrize(
    "bad",
    [
        {"flip": "yes"},
        {"x": None},
        {"id": 5},
        {"rotation": "90"},
    ],
)
def test_placed_piece_fields_are_checked(client, layout_id, bad):
    elements = {"track_system": SETRACK, "track_pieces": [_placed("peco-n-st-1", **bad)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400
