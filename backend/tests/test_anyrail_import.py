"""
AnyRail (JMRI layout-config XML) import: scale, turnout placement, and what is left unplaced.
"""

import math

import pytest

from app.services.anyrail_geometry import _turnout_ends, _world
from app.services.anyrail_import import import_jmri_layout
from app.services.track_library import load_library

# Two turnouts, a 100 px radius quarter curve from A1 to A2, and a 200 px straight from A1 down to A3.
# The curve's chord is 100 * sqrt(2), so its radius is 100 px and the scale is 263.5 / 100 = 2.635.
# At A1 the curve leaves upward while the straight leaves downward, so they run straight through
LAYOUT = """<layout-config>
<LayoutEditor>
<positionablepoint ident="A1" x="0" y="200" type="ANCHOR"/>
<positionablepoint ident="A2" x="100" y="100" type="ANCHOR"/>
<positionablepoint ident="A3" x="0" y="400" type="ANCHOR"/>
<layoutturnout ident="TOR1" type="RH_TURNOUT" connectaname="T-I-TOR1" connectbname="T-EL-TOR1" connectcname="T-ER-TOR1"
  xcen="100" ycen="100" xb="112.92" yb="100" xc="112.83" yc="102.5"/>
<layoutturnout ident="TOL2" type="LH_TURNOUT" connectaname="T-I-TOL2" connectbname="T-ER-TOL2" connectcname="T-EL-TOL2"
  xcen="300" ycen="100" xb="287.08" yb="100" xc="287.17" yc="102.5"/>
<layoutturnout ident="TOR3" type="RH_TURNOUT" connectaname="T-I-TOR3" connectbname="T-EL-TOR3" connectcname="T-ER-TOR3"
  xcen="500" ycen="100" xb="512.92" yb="100" xc="512.83" yc="97.5"/>
<tracksegment ident="F1" connect1name="A1" type1="POS_POINT" connect2name="A2" type2="POS_POINT" arc="yes" angle="90"/>
<tracksegment ident="F2" connect1name="A1" type1="POS_POINT" connect2name="A3" type2="POS_POINT"/>
</LayoutEditor>
</layout-config>"""


# A turnout whose diverging anchor P3 sits about 0.5 mm off where the diverging leg really ends.
# The track S joins the turnout at P3, and should meet the turnout at its leg, not at the anchor
TURNOUT_JOINED = """<layout-config>
<LayoutEditor>
<positionablepoint ident="A1" x="0" y="200" type="ANCHOR"/>
<positionablepoint ident="A2" x="100" y="100" type="ANCHOR"/>
<tracksegment ident="F1" connect1name="A1" type1="POS_POINT" connect2name="A2" type2="POS_POINT" arc="yes" angle="90"/>
<positionablepoint ident="P1" x="0" y="0" type="ANCHOR"/>
<positionablepoint ident="P2" x="52" y="0" type="ANCHOR"/>
<positionablepoint ident="P3" x="52" y="5.33" type="ANCHOR"/>
<positionablepoint ident="P4" x="152" y="5.33" type="ANCHOR"/>
<layoutturnout ident="TOR1" type="RH_TURNOUT" connectaname="T-I-TOR1" connectbname="T-EL-TOR1" connectcname="T-ER-TOR1"
  xcen="0" ycen="0" xb="52" yb="0" xc="52" yc="5.13"/>
<tracksegment ident="CA" connect1name="TOR1" type1="TURNOUT_A" connect2name="P1" type2="POS_POINT"/>
<tracksegment ident="CB" connect1name="TOR1" type1="TURNOUT_B" connect2name="P2" type2="POS_POINT"/>
<tracksegment ident="CC" connect1name="TOR1" type1="TURNOUT_C" connect2name="P3" type2="POS_POINT"/>
<tracksegment ident="S" connect1name="P3" type1="POS_POINT" connect2name="P4" type2="POS_POINT"/>
</LayoutEditor>
</layout-config>"""


def _by_id(pieces):
    return {piece["id"]: piece for piece in pieces}


def test_scale_comes_from_the_innermost_curve():
    result = import_jmri_layout(LAYOUT, inner_radius_mm=263.5)
    assert result["scale_mm_per_pixel"] == pytest.approx(2.635, abs=1e-4)


def test_right_hand_turnout_placed_as_sl_u395f_at_scaled_toe():
    pieces = _by_id(import_jmri_layout(LAYOUT, inner_radius_mm=263.5)["track_pieces"])
    piece = pieces["jmri-TOR1"]
    assert piece["piece_id"] == "peco-n-sl-u395f"
    # The layout is moved so its top left sits at the 50 mm margin: +50 in x and -213.5 in y here
    assert piece["x"] == pytest.approx(313.5)
    assert piece["y"] == pytest.approx(50.0)
    assert piece["rotation"] == 0
    assert piece["flip"] is False


def test_left_hand_turnout_points_back_along_its_straight():
    pieces = _by_id(import_jmri_layout(LAYOUT, inner_radius_mm=263.5)["track_pieces"])
    piece = pieces["jmri-TOL2"]
    assert piece["piece_id"] == "peco-n-sl-u396f"
    assert piece["rotation"] == 180


def test_turnout_whose_diverge_side_disagrees_with_hand_is_flagged():
    result = import_jmri_layout(LAYOUT, inner_radius_mm=263.5)
    assert result["hand_mismatches"] == ["TOR3"]


def test_straight_is_placed_as_flex_with_its_length():
    pieces = _by_id(import_jmri_layout(LAYOUT, inner_radius_mm=263.5)["track_pieces"])
    piece = pieces["jmri-F2"]
    assert piece["piece_id"] == "peco-n-sl-300f"
    assert piece["length"] == pytest.approx(527.0)
    assert piece["x"] == pytest.approx(50.0)
    assert piece["y"] == pytest.approx(313.5)
    assert piece["rotation"] == 90


def test_curve_is_placed_as_flex_track_bent_through_two_points():
    pieces = _by_id(import_jmri_layout(LAYOUT, inner_radius_mm=263.5)["track_pieces"])
    piece = pieces["jmri-F1"]
    assert piece["piece_id"] == "peco-n-sl-300f"
    # Chord from A1 (0,200) to A2 (100,100) in pixels, which is 100 * sqrt(2) px, scaled to mm
    assert piece["length"] == pytest.approx(263.5 * 2 ** 0.5, abs=0.01)
    assert piece["rotation"] == pytest.approx(315)
    assert len(piece["handles"]) == 2


def test_double_slips_are_reported_as_unplaced():
    xml = LAYOUT.replace("</LayoutEditor>", '<layoutSlip ident="SL1" slipType="DOUBLE_SLIP"/></LayoutEditor>')
    unplaced = import_jmri_layout(xml, inner_radius_mm=263.5)["unplaced"]
    assert unplaced == [{"ident": "SL1", "kind": "double_slip", "reason": "a slip end is not joined to an anchor"}]


def test_rejects_document_type_declarations():
    xml = '<!DOCTYPE x [<!ENTITY e "boom">]><layout-config/>'
    with pytest.raises(ValueError, match="DOCTYPE"):
        import_jmri_layout(xml, inner_radius_mm=263.5)


def test_rejects_invalid_xml():
    with pytest.raises(ValueError, match="Not valid XML"):
        import_jmri_layout("<layout-config>", inner_radius_mm=263.5)


def test_rejects_layout_without_curves():
    xml = '<layout-config><LayoutEditor><layoutturnout ident="T" type="RH_TURNOUT" connectaname="a" connectbname="b" connectcname="c" xcen="0" ycen="0" xb="1" yb="0" xc="1" yc="1"/></LayoutEditor></layout-config>'
    with pytest.raises(ValueError, match="90 degree curves"):
        import_jmri_layout(xml, inner_radius_mm=263.5)


@pytest.mark.parametrize("radius", [0, -5, "263.5", None])
def test_rejects_bad_radius(radius):
    with pytest.raises(ValueError):
        import_jmri_layout(LAYOUT, inner_radius_mm=radius)


def test_preview_endpoint_returns_pieces_without_saving(client):
    response = client.post("/api/layouts/anyrail-import", json={"xml": LAYOUT, "inner_radius_mm": 263.5})
    assert response.status_code == 200
    body = response.json()
    assert len(body["track_pieces"]) > 0
    assert body["track_system"] == "Code 55"
    assert client.get("/api/layouts").json() == []


def test_preview_endpoint_rejects_bad_input(client):
    bad_xml = client.post("/api/layouts/anyrail-import", json={"xml": "<layout-config>", "inner_radius_mm": 263.5})
    assert bad_xml.status_code == 400
    bad_radius = client.post("/api/layouts/anyrail-import", json={"xml": LAYOUT, "inner_radius_mm": 0})
    assert bad_radius.status_code == 422


def test_track_joins_a_turnout_leg_exactly_not_at_its_anchor():
    result = import_jmri_layout(TURNOUT_JOINED, inner_radius_mm=263.5)
    pieces = _by_id(result["track_pieces"])
    turnout = pieces["jmri-TOR1"]
    library = load_library()
    leg_c = _turnout_ends(library[turnout["piece_id"]])[2][0]
    leg_c_world = _world(leg_c, (turnout["x"], turnout["y"]), turnout["rotation"], turnout["flip"])
    track = pieces["jmri-S"]
    assert math.dist((track["x"], track["y"]), leg_c_world) < 1e-3


def _slip_joined_xml():
    """A single slip whose four anchors sit on its legs. Anchor D is about 0.5 mm off, and track S joins it"""
    scale = 2.635  # mm per px, as the arc below gives
    legs = {"A": (-6.48, -77.0), "B": (6.4, 77.0), "C": (7.12, -77.0), "D": (-7.03, 77.0)}
    anchors = []
    for name, (x, y) in legs.items():
        dx = 0.2 if name == "D" else 0.0
        anchors.append(f'<positionablepoint ident="P{name}" x="{x / scale + dx:.4f}" y="{y / scale:.4f}" type="ANCHOR"/>')
    connectors = [
        f'<tracksegment ident="C{name}" connect1name="SL1" type1="SLIP_{name}" connect2name="P{name}" type2="POS_POINT"/>'
        for name in legs
    ]
    return f"""<layout-config>
<LayoutEditor>
<positionablepoint ident="A1" x="0" y="200" type="ANCHOR"/>
<positionablepoint ident="A2" x="100" y="100" type="ANCHOR"/>
<tracksegment ident="F1" connect1name="A1" type1="POS_POINT" connect2name="A2" type2="POS_POINT" arc="yes" angle="90"/>
{chr(10).join(anchors)}
<positionablepoint ident="PFAR" x="200" y="60" type="ANCHOR"/>
{chr(10).join(connectors)}
<tracksegment ident="S" connect1name="PD" type1="POS_POINT" connect2name="PFAR" type2="POS_POINT"/>
<layoutSlip ident="SL1"/>
</LayoutEditor>
</layout-config>"""


def test_track_joins_a_slip_leg_exactly_not_at_its_anchor():
    result = import_jmri_layout(_slip_joined_xml(), inner_radius_mm=263.5)
    pieces = _by_id(result["track_pieces"])
    slip = pieces["jmri-SL1"]
    library = load_library()
    geometry = library[slip["piece_id"]]["slip_geometry"]
    leg_points = [
        _world(tuple(geometry[name]), (slip["x"], slip["y"]), slip["rotation"], slip["flip"])
        for name in ("track_a_top", "track_b_top", "track_a_bottom", "track_b_bottom")
    ]
    track = pieces["jmri-S"]
    assert min(math.dist((track["x"], track["y"]), point) for point in leg_points) < 1e-3
