"""
AnyRail (JMRI layout-config XML) import: scale, turnout placement, and what is left unplaced.
"""

import pytest

from app.services.anyrail_import import import_jmri_layout

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


def _by_id(pieces):
    return {piece["id"]: piece for piece in pieces}


def test_scale_comes_from_the_innermost_curve():
    result = import_jmri_layout(LAYOUT, inner_radius_mm=263.5)
    assert result["scale_mm_per_pixel"] == pytest.approx(2.635, abs=1e-4)


def test_right_hand_turnout_placed_as_sl_u395f_at_scaled_toe():
    pieces = _by_id(import_jmri_layout(LAYOUT, inner_radius_mm=263.5)["track_pieces"])
    piece = pieces["jmri-TOR1"]
    assert piece["piece_id"] == "peco-n-sl-u395f"
    assert piece["x"] == pytest.approx(263.5)
    assert piece["y"] == pytest.approx(263.5)
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
    assert piece["x"] == 0
    assert piece["y"] == pytest.approx(527.0)
    assert piece["rotation"] == 90


def test_curve_is_placed_with_its_radius_and_leaves_the_joint_upward():
    pieces = _by_id(import_jmri_layout(LAYOUT, inner_radius_mm=263.5)["track_pieces"])
    piece = pieces["jmri-F1"]
    assert piece["piece_id"] == "custom-n-custom-curve"
    assert piece["radius"] == pytest.approx(263.5)
    assert piece["angle"] == 90
    assert piece["flip"] is False
    assert piece["rotation"] == 270


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
