"""
Track library tests: loading, the read-only API, and layout validation of placed pieces.
"""

import pytest

from app.services.track_library import list_pieces, load_library

CODE_80 = "Code 80"
CODE_55 = "Code 55"


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
    assert len(library) == 53 + 1  # PECO pieces plus the custom curve
    assert library["peco-n-st-1"]["name"] == "Standard Straight"
    assert library["peco-n-st-1"]["system"] == CODE_80
    assert library["peco-n-st-1"]["series"] == "Setrack"


def test_verified_is_per_piece_and_sourced_from_peco():
    library = load_library()
    assert library["peco-n-st-1"]["verified"] is True
    assert library["peco-n-st-1"]["source"].startswith("https://peco-uk.com/products/")
    # Radius not printed on the PECO page, so not verified
    assert library["peco-n-st-3"]["verified"] is False


def test_library_pieces_carry_their_geometry():
    piece = load_library()["peco-n-st-5"]
    assert piece["type"] == "turnout"
    assert piece["hand"] == "right"
    assert piece["length"] == 87
    assert piece["radius"] == 228
    assert piece["angle"] == 22.5


def test_sl_u395f_matches_peco_page_and_is_placeable():
    piece = load_library()["peco-n-sl-u395f"]
    assert piece["length"] == 137
    assert piece["radius"] == 457
    assert piece["angle"] == 10
    assert piece["angle_meaning"] == "frog"
    assert piece["placeable"] is True
    assert piece["verified"] is True


def test_sl_u396f_is_placeable_left_hand_with_frog_angle():
    piece = load_library()["peco-n-sl-u396f"]
    assert piece["hand"] == "left"
    assert piece["length"] == 137
    assert piece["radius"] == 457
    assert piece["angle_meaning"] == "frog"
    assert piece["placeable"] is True


def test_code_80_streamline_is_compatible_with_setrack():
    library = load_library()
    assert library["peco-n-sl-395"]["series"] == "Streamline Code 80"
    assert library["peco-n-sl-395"]["system"] == CODE_80
    assert library["peco-n-sl-395"]["length"] == 123.7


def test_flex_track_is_placeable_without_turnout_geometry():
    library = load_library()
    for piece_id in ("peco-n-sl-300f", "peco-n-sl-302f", "peco-n-sl-300", "peco-n-sl-303"):
        assert library[piece_id]["type"] == "flex"
        assert library[piece_id]["length"] == 914
        assert library[piece_id]["placeable"] is True


def test_single_slip_geometry_from_plan_sheet():
    library = load_library()
    for piece_id in ("peco-n-sl-380f", "peco-n-sl-e380f"):
        piece = library[piece_id]
        assert piece["type"] == "slip"
        assert piece["placeable"] is True
        assert set(piece["slip_geometry"]) == {"track_a_top", "track_a_bottom", "track_b_top", "track_b_bottom"}


def test_curved_turnout_geometry_from_plan_sheet():
    library = load_library()
    for piece_id in ("peco-n-sl-386", "peco-n-sl-387", "peco-n-sl-e386", "peco-n-sl-e387"):
        piece = library[piece_id]
        assert piece["placeable"] is True
        assert piece["length"] == 158.04
        assert piece["curve_geometry"]["inner_radius"] == 457
        assert piece["curve_geometry"]["outer_radius"] == 914


def test_list_pieces_filters_by_system():
    code_80 = list_pieces(system=CODE_80)
    code_55 = list_pieces(system=CODE_55)
    assert code_80 and code_55
    assert all(piece["system"] == CODE_80 for piece in code_80)
    assert len(code_80) + len(code_55) == 54


def test_library_endpoint_lists_and_filters(client):
    everything = client.get("/api/track-library").json()
    assert len(everything) == 54

    code_80 = client.get("/api/track-library", params={"system": CODE_80}).json()
    assert all(piece["system"] == CODE_80 for piece in code_80)


def test_library_piece_endpoint(client):
    response = client.get("/api/track-library/peco-n-st-3")
    assert response.status_code == 200
    assert response.json()["radius"] == 228

    assert client.get("/api/track-library/no-such-piece").status_code == 404


def test_layout_accepts_placed_pieces(client, layout_id):
    elements = {
        "tracks": [],
        "track_system": CODE_80,
        "track_pieces": [_placed("peco-n-st-1"), _placed("peco-n-st-3", "p2", x=87, flip=True)],
    }
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200
    assert len(response.json()["elements"]["track_pieces"]) == 2


def test_setrack_and_streamline_code_80_can_share_a_layout(client, layout_id):
    elements = {
        "track_system": CODE_80,
        "track_pieces": [_placed("peco-n-st-1", "p1"), _placed("peco-n-sl-395", "p2", x=87)],
    }
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200


def test_unknown_piece_rejected(client, layout_id):
    elements = {"track_system": CODE_80, "track_pieces": [_placed("peco-n-nothing")]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


def test_mixing_track_systems_rejected(client, layout_id):
    elements = {
        "track_system": CODE_80,
        "track_pieces": [
            _placed("peco-n-st-1", "p1"),
            _placed("peco-n-sl-e388f", "p2"),
        ],
    }
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400
    assert "mix" in response.json()["detail"]


def test_track_system_must_match_pieces(client, layout_id):
    elements = {"track_system": CODE_55, "track_pieces": [_placed("peco-n-st-1")]}
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
    elements = {"track_system": CODE_80, "track_pieces": [_placed("peco-n-st-1", **bad)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


def test_custom_curve_needs_a_radius_on_the_placement(client, layout_id):
    elements = {"track_system": CODE_55, "track_pieces": [_placed("custom-n-custom-curve")]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400

    elements["track_pieces"] = [_placed("custom-n-custom-curve", radius=263.5, angle=90)]
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200
    assert response.json()["elements"]["track_pieces"][0]["radius"] == 263.5


def test_flex_track_takes_a_custom_length(client, layout_id):
    elements = {"track_system": CODE_55, "track_pieces": [_placed("peco-n-sl-300f", length=123.4)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200
    assert response.json()["elements"]["track_pieces"][0]["length"] == 123.4


@pytest.mark.parametrize(
    "piece_id, dimensions",
    [
        ("peco-n-sl-300f", {"radius": 100}),
        ("peco-n-sl-300f", {"length": 0}),
        ("peco-n-sl-300f", {"length": -5}),
        ("peco-n-sl-300f", {"length": "100"}),
        ("custom-n-custom-curve", {"radius": 0, "angle": 90}),
        ("custom-n-custom-curve", {"radius": 263.5, "angle": 400}),
        ("peco-n-sl-u395f", {"length": 100}),
    ],
)
def test_placed_dimensions_must_suit_the_piece(client, layout_id, piece_id, dimensions):
    elements = {"track_system": CODE_55, "track_pieces": [_placed(piece_id, **dimensions)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


def test_three_way_branches_are_measured_from_the_layout():
    piece = load_library()["peco-n-sl-e399f"]
    geometry = piece["three_way_geometry"]
    assert geometry["provisional"] is False
    assert "AnyRail" in geometry["measured_from"]
    assert geometry["right"]["sweep"] == 19.17
    assert geometry["left"]["sweep"] == 18.74


def test_dcc_address_is_accepted_on_a_turnout(client, layout_id):
    elements = {"track_system": CODE_55, "track_pieces": [_placed("peco-n-sl-u395f", dcc_address=25, dcc_output_straight=1)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200
    placed = response.json()["elements"]["track_pieces"][0]
    assert placed["dcc_address"] == 25
    assert placed["dcc_output_straight"] == 1


@pytest.mark.parametrize(
    "piece_id, dcc",
    [
        ("peco-n-sl-300f", {"dcc_address": 25}),
        ("peco-n-sl-u395f", {"dcc_address": 0}),
        ("peco-n-sl-u395f", {"dcc_address": 2049}),
        ("peco-n-sl-u395f", {"dcc_address": "25"}),
        ("peco-n-sl-u395f", {"dcc_output_straight": 2}),
    ],
)
def test_dcc_fields_must_suit_the_piece(client, layout_id, piece_id, dcc):
    elements = {"track_system": CODE_55, "track_pieces": [_placed(piece_id, **dcc)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


def test_two_turnouts_cannot_share_an_address(client, layout_id):
    elements = {
        "track_system": CODE_55,
        "track_pieces": [
            _placed("peco-n-sl-u395f", "p1", dcc_address=25),
            _placed("peco-n-sl-u396f", "p2", dcc_address=25),
        ],
    }
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400


def test_flex_track_takes_handles(client, layout_id):
    handles = [{"x": 30, "y": 15}, {"x": 60, "y": -15}]
    elements = {"track_system": CODE_55, "track_pieces": [_placed("peco-n-sl-300f", length=100, handles=handles)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 200
    assert response.json()["elements"]["track_pieces"][0]["handles"] == handles


@pytest.mark.parametrize(
    "piece_id, handles",
    [
        ("peco-n-sl-300f", [{"x": 1, "y": 1}]),
        ("peco-n-sl-300f", [{"x": 1, "y": 1}, {"x": "2", "y": 1}]),
        ("peco-n-sl-300f", [{"x": 1, "y": 1}, {"x": 2, "y": 20000}]),
        ("peco-n-sl-u395f", [{"x": 1, "y": 1}, {"x": 2, "y": 2}]),
    ],
)
def test_handles_must_suit_the_piece(client, layout_id, piece_id, handles):
    elements = {"track_system": CODE_55, "track_pieces": [_placed(piece_id, handles=handles)]}
    response = client.patch(f"/api/layouts/{layout_id}", json={"elements": elements})
    assert response.status_code == 400
