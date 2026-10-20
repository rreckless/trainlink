"""
Import a JMRI layout-config XML exported from AnyRail as TrainLink track pieces.

JMRI coordinates are pixels with no real size, so the caller gives the real radius in mm of the
innermost curve in the layout. The smallest 90 degree arc in the file is taken to be that curve,
and every coordinate is scaled by the same factor.

The file's anchor points (positionablepoint) are drawn to scale, but a turnout's own symbol points
are not. Each turnout therefore sits on its three anchors: its toe anchor is the toe, and its
straight and diverging anchors are the ends of its legs. The short connector segments between a
turnout and its anchors have no real length, so they are not placed.

Pieces:
- Turnouts: RH_TURNOUT as SL-U395F, LH_TURNOUT as SL-U396F.
- Straights: flex track (SL-300F) with the length set on the placement.
- Curves: 90 degree custom curves with the radius set on the placement. Cubic Beziers are fitted by
  a circular arc with the same ends and start tangent.

Double slips are reported as unplaced, since no slip geometry is matched to them yet.
"""
import math
import xml.etree.ElementTree as ET

from .anyrail_geometry import (
    _align,
    _angle,
    _fit_four,
    _line_difference,
    _normalize,
    _rotate,
    _three_way_ends,
    _turnout_ends,
    _world,
    arc_bend_samples,
    bezier_bend_samples,
    fit_handles,
)
from .track_library import load_library

TURNOUT_PIECES = {"RH_TURNOUT": "peco-n-sl-u395f", "LH_TURNOUT": "peco-n-sl-u396f"}
TURNOUT_KEYS = ("TURNOUT_A", "TURNOUT_B", "TURNOUT_C")
SLIP_PIECE = "peco-n-sl-e390f"
THREE_WAY_PIECE = "peco-n-sl-e399f"
SLIP_KEYS = ("SLIP_A", "SLIP_B", "SLIP_C", "SLIP_D")
STRAIGHT_PIECE = "peco-n-sl-300f"
MIN_CURVE_MM = 1.0
GAP_REPORT_MM = 1.0
MARGIN_MM = 50.0


def _parse(xml_text):
    if not isinstance(xml_text, str) or not xml_text.strip():
        raise ValueError("No layout XML supplied")
    # JMRI exports never declare a DTD. Refusing one blocks entity-expansion attacks
    if "<!DOCTYPE" in xml_text or "<!ENTITY" in xml_text:
        raise ValueError("XML with a DOCTYPE or ENTITY declaration is not accepted")
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError as error:
        raise ValueError(f"Not valid XML: {error}") from error
    if root.find(".//layoutturnout") is None and root.find(".//tracksegment") is None:
        raise ValueError("No layout track found in the XML")
    return root


def _xy(element, x_attr, y_attr):
    return (float(element.get(x_attr)), float(element.get(y_attr)))


























def _raw_points(root):
    """Every anchor and turnout connection to its JMRI pixel position"""
    raw = {}
    for point in root.iter("positionablepoint"):
        raw[(point.get("ident"), "POS_POINT")] = _xy(point, "x", "y")
    for turnout in root.iter("layoutturnout"):
        ident = turnout.get("ident")
        for key, (x_attr, y_attr) in zip(TURNOUT_KEYS, (("xcen", "ycen"), ("xb", "yb"), ("xc", "yc"))):
            raw[(ident, key)] = _xy(turnout, x_attr, y_attr)
    return raw


def _segment_keys(segment):
    return ((segment.get("connect1name"), segment.get("type1")), (segment.get("connect2name"), segment.get("type2")))


def _arc_radii_px(root, raw):
    """(ident, radius in pixels) of each 90 degree arc, from its chord"""
    radii = {}
    for segment in root.iter("tracksegment"):
        if segment.get("arc") != "yes":
            continue
        start, end = (raw.get(key) for key in _segment_keys(segment))
        if start is None or end is None:
            continue
        half_angle = math.radians(float(segment.get("angle"))) / 2
        radii[segment.get("ident")] = math.dist(start, end) / (2 * math.sin(half_angle))
    return radii


def _record(ident, piece_id, origin, rotation, flip, **dimensions):
    return {
        "id": f"jmri-{ident}",
        "piece_id": piece_id,
        "x": round(origin[0], 4),
        "y": round(origin[1], 4),
        "rotation": round(_normalize(rotation), 4),
        "flip": flip,
        **dimensions,
    }


def _connectors(root):
    """
    The other end of each turnout or slip leg's connector segment, keyed by the leg, and the connector idents.
    A connector is any segment with a turnout or slip at one end
    """
    links, connectors = {}, set()
    for segment in root.iter("tracksegment"):
        keys = _segment_keys(segment)
        if not any(key[1].startswith(("TURNOUT_", "SLIP_")) for key in keys):
            continue
        connectors.add(segment.get("ident"))
        for leg, other in ((keys[0], keys[1]), (keys[1], keys[0])):
            if leg[1].startswith(("TURNOUT_", "SLIP_")):
                links[leg] = other
    return links, connectors






def _place_turnouts(root, raw, scale, library, links, skip):
    """
    Turnout records, the world position and outward heading of each leg, the hand mismatches,
    the turnouts that were not placed, the legs that miss their anchor, and the turnouts with no anchor
    """
    specs, unplaced, mismatches = {}, [], []
    for turnout in root.iter("layoutturnout"):
        ident = turnout.get("ident")
        hand = turnout.get("type")
        if ident in skip:
            continue
        if hand not in TURNOUT_PIECES:
            unplaced.append({"ident": ident, "kind": "turnout", "reason": "unknown turnout type"})
            continue
        a, b, c = (raw[(ident, key)] for key in TURNOUT_KEYS)
        # Positive when the diverging track turns toward +y, the side a right-hand piece is drawn on
        cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        if (cross > 0) != (hand == "RH_TURNOUT"):
            mismatches.append(ident)
        specs[ident] = {"piece_id": TURNOUT_PIECES[hand], "keys": [(ident, key) for key in TURNOUT_KEYS], "symbol": (a, b)}

    records, ends, heading_out = [], {}, {}

    def place(ident, origin, rotation):
        spec = specs[ident]
        records.append(_record(ident, spec["piece_id"], origin, rotation, False))
        for (local_point, local_heading), key in zip(_turnout_ends(library[spec["piece_id"]]), spec["keys"]):
            ends[key] = _world(local_point, origin, rotation, False)
            heading_out[key] = _normalize(rotation + local_heading)
        pending.remove(ident)

    def anchor(key):
        link = links.get(key)
        if link is None or link[1] != "POS_POINT" or link not in raw:
            return None
        return (raw[link][0] * scale, raw[link][1] * scale)

    pending = list(specs)
    progress = True
    while pending and progress:
        progress = False
        for ident in list(pending):
            spec = specs[ident]
            piece_ends = _turnout_ends(library[spec["piece_id"]])
            # Two anchored legs fix the turnout: its local layout between them is known, so the rest follows
            anchored = [(index, anchor(key)) for index, key in enumerate(spec["keys"]) if anchor(key) is not None]
            if len(anchored) >= 2:
                (i, world_i), (j, world_j) = anchored[0], anchored[1]
                local_i, local_j = piece_ends[i][0], piece_ends[j][0]
                rotation = _angle(world_i, world_j) - _angle(local_i, local_j)
                rx, ry = _rotate(local_i, rotation)
                place(ident, (world_i[0] - rx, world_i[1] - ry), rotation)
                progress = True
                continue
            # Otherwise join a leg to a turnout that is already placed
            for index, key in enumerate(spec["keys"]):
                other = links.get(key)
                if other is not None and other[1].startswith("TURNOUT_") and other in ends:
                    origin, rotation = _align(piece_ends, index, ends[other], heading_out[other] + 180)
                    place(ident, origin, rotation)
                    progress = True
                    break

    # Turnouts that touch no anchor and no placed turnout: placed from the file's own symbol
    unanchored = []
    for ident in list(pending):
        a, b = specs[ident]["symbol"]
        unanchored.append(ident)
        place(ident, (a[0] * scale, a[1] * scale), _angle(a, b))

    gaps = []
    for ident, spec in specs.items():
        for key in spec["keys"]:
            link = links.get(key)
            if link is None:
                continue
            if link[1] == "POS_POINT" and link in raw:
                target = (raw[link][0] * scale, raw[link][1] * scale)
            elif link[1].startswith("TURNOUT_") and link in ends and key < link:
                target = ends[link]
            else:
                continue
            gap = math.dist(ends[key], target)
            if gap > GAP_REPORT_MM:
                gaps.append({"ident": ident, "joint": f"{ident} {key[1]}", "gap_mm": round(gap, 2)})

    return records, ends, heading_out, mismatches, unplaced, unanchored, gaps, set(specs)


def _joint_lines(links, heading_out):
    """Directions of the turnout legs at each anchor, for matching curve bulges"""
    lines = {}
    for key, heading in heading_out.items():
        link = links.get(key)
        if link is not None and link[1] == "POS_POINT":
            lines.setdefault(link, []).append(heading)
    return lines


def _segment_lines(root, raw):
    """Directions of the straight segments at each anchor"""
    lines = {}
    for segment in root.iter("tracksegment"):
        if segment.get("arc") == "yes" or segment.get("bezier") == "yes":
            continue
        start, end = _segment_keys(segment)
        if start in raw and end in raw:
            lines.setdefault(start, []).append(_angle(raw[start], raw[end]))
            lines.setdefault(end, []).append(_angle(raw[end], raw[start]))
    return lines


def _choose_curve_flip(start, end, start_key, end_key, lines):
    """
    Which way a 90 degree arc bulges. Each option gives the heading it starts on, and its ends should
    carry on the tangent of the track they join. Returns (flip, ambiguous)
    """
    chord = _angle(start, end)
    scores = {}
    for flip in (False, True):
        heading = chord + 45 if flip else chord - 45
        end_heading = heading - 90 if flip else heading + 90
        score, joined = 0.0, 0
        for key, tangent in ((start_key, heading), (end_key, end_heading)):
            candidates = lines.get(key)
            if candidates:
                score += min(_line_difference(tangent, line) for line in candidates)
                joined += 1
        scores[flip] = (score, joined)
    (false_score, false_joined), (true_score, true_joined) = scores[False], scores[True]
    if false_joined == 0 and true_joined == 0:
        return False, True
    return true_score < false_score, abs(false_score - true_score) < 1


def _three_way_groups(root, links):
    """
    Pairs of turnouts joined directly to each other, whose other four legs all go to anchors. AnyRail
    draws an asymmetric three-way as such a pair. Returns a list of (turnout idents, outer leg keys)
    """
    turnouts = {turnout.get("ident") for turnout in root.iter("layoutturnout")}
    groups, used = [], set()
    for leg, other in links.items():
        if not (leg[1].startswith("TURNOUT_") and other[1].startswith("TURNOUT_")):
            continue
        pair = {leg[0], other[0]}
        if len(pair) != 2 or not pair <= turnouts or pair & used:
            continue
        outer = [(ident, key) for ident in sorted(pair) for key in TURNOUT_KEYS if (ident, key) not in (leg, other)]
        if all(links.get(key, ("", ""))[1] == "POS_POINT" for key in outer):
            groups.append((tuple(sorted(pair)), outer))
            used |= pair
    return groups


def _place_three_ways(groups, raw, scale, library, links):
    """
    Records for the three-way pieces, the gaps at their ends, the turnouts they replace, and the exact
    point of each outer leg, keyed by the anchor its track joins
    """
    records, gaps, replaced, leg_points = [], [], set(), {}
    local = _three_way_ends(library[THREE_WAY_PIECE])
    for idents, outer in groups:
        world = [(raw[links[key]][0] * scale, raw[links[key]][1] * scale) for key in outer]
        origin, rotation, flip, local_of_world, _ = _fit_four(local, world)
        ident = f"{idents[0]}-3way"
        records.append(_record(ident, THREE_WAY_PIECE, origin, rotation, flip))
        replaced |= set(idents)
        for index, key in enumerate(outer):
            end = _world(local[local_of_world[index]], origin, rotation, flip)
            leg_points[links[key]] = tuple(end)
            gap = math.dist(end, world[index])
            if gap > GAP_REPORT_MM:
                gaps.append({"ident": ident, "joint": f"{key[0]} {key[1]}", "gap_mm": round(gap, 2)})
    return records, gaps, replaced, leg_points


def _place_slips(root, raw, scale, library, links):
    """
    Slip records, the gaps at each slip end, the slips that could not be placed, and the exact point of
    each slip end, keyed by the anchor its track joins
    """
    records, gaps, unplaced, placed, leg_points = [], [], [], set(), {}
    geometry = library[SLIP_PIECE]["slip_geometry"]
    local = [tuple(geometry[name]) for name in ("track_a_top", "track_b_top", "track_a_bottom", "track_b_bottom")]
    for slip in root.iter("layoutSlip"):
        ident = slip.get("ident")
        keys = [(ident, key) for key in SLIP_KEYS]
        anchors = [links.get(key) for key in keys]
        if any(link is None or link[1] != "POS_POINT" or link not in raw for link in anchors):
            unplaced.append({"ident": ident, "kind": "double_slip", "reason": "a slip end is not joined to an anchor"})
            continue
        world = [(raw[link][0] * scale, raw[link][1] * scale) for link in anchors]
        origin, rotation, flip, local_of_world, error = _fit_four(local, world)
        records.append(_record(ident, SLIP_PIECE, origin, rotation, flip))
        placed.add(ident)
        for index, key in enumerate(keys):
            end = _world(local[local_of_world[index]], origin, rotation, flip)
            leg_points[anchors[index]] = tuple(end)
            gap = math.dist(end, world[index])
            if gap > GAP_REPORT_MM:
                gaps.append({"ident": ident, "joint": f"{ident} {key[1]}", "gap_mm": round(gap, 2)})
    return records, gaps, unplaced, placed, leg_points


def _place_segments(root, raw, radii_px, scale, connectors, supported, lines, joined_points):
    """
    Records for the straights and curves, the ambiguous curves, and the unplaced segments.
    joined_points holds, in mm, the exact point of each track end that joins a turnout leg. Those ends use it,
    so the track meets the turnout exactly rather than at its anchor, which can be a fraction of a mm away
    """
    records, ambiguous, unplaced = [], [], []
    for segment in root.iter("tracksegment"):
        ident = segment.get("ident")
        arc = segment.get("arc") == "yes"
        bezier = segment.get("bezier") == "yes"
        kind = "curve" if arc or bezier else "straight"
        keys = _segment_keys(segment)

        if ident in connectors:
            # Connectors to turnouts and slips have no real length. Report only the ones that lead to an unplaced piece
            if any(key[1].startswith(("TURNOUT_", "SLIP_")) and key[0] not in supported for key in keys):
                unplaced.append({"ident": ident, "kind": kind, "reason": "connected to an unplaced turnout or slip"})
            continue
        if any(key not in raw for key in keys):
            unplaced.append({"ident": ident, "kind": kind, "reason": "unknown end point"})
            continue

        start_jmri, end_jmri = raw[keys[0]], raw[keys[1]]
        start = joined_points.get(keys[0]) or tuple(value * scale for value in start_jmri)
        end = joined_points.get(keys[1]) or tuple(value * scale for value in end_jmri)
        if bezier:
            controls = sorted(segment.iter("controlpoint"), key=lambda point: int(point.get("index")))
            if len(controls) != 2:
                unplaced.append({"ident": ident, "kind": "curve", "reason": "bezier without two control points"})
                continue
            control_points = [tuple(value * scale for value in _xy(point, "x", "y")) for point in controls]
            samples, chord, angle, start_angle, end_angle = bezier_bend_samples(
                start, control_points[0], control_points[1], end)
            if chord < MIN_CURVE_MM:
                unplaced.append({"ident": ident, "kind": "curve", "reason": "bezier shorter than 1 mm"})
                continue
            records.append(_curve_as_flex(ident, start, math.degrees(angle), chord, samples, start_angle, end_angle))
        elif arc:
            if float(segment.get("angle")) != 90:
                unplaced.append({"ident": ident, "kind": "curve", "reason": "only 90 degree curves are supported"})
                continue
            flip, is_ambiguous = _choose_curve_flip(start_jmri, end_jmri, keys[0], keys[1], lines)
            samples, arc_chord, _, start_angle, end_angle = arc_bend_samples(
                radii_px[ident] * scale, math.radians(90), flip)
            # The ends may have moved onto turnout legs, a fraction of a mm from the arc's own chord. Fit the shape to them
            chord = math.dist(start, end)
            factor = chord / arc_chord
            samples = [(x * factor, y * factor) for x, y in samples]
            records.append(_curve_as_flex(ident, start, _angle(start, end), chord, samples, start_angle, end_angle))
            if is_ambiguous:
                ambiguous.append(ident)
        else:
            records.append(_record(ident, STRAIGHT_PIECE, start, _angle(start, end), False,
                                   length=round(math.dist(start, end), 4)))
    return records, ambiguous, unplaced


def _curve_as_flex(ident, origin, rotation, chord, samples, start_angle, end_angle):
    """A curve as flex track of its chord length, curved by two handles that follow it"""
    handles = fit_handles(samples, chord, start_angle, end_angle)
    return _record(ident, STRAIGHT_PIECE, origin, rotation, False, length=round(chord, 4),
                   handles=[{"x": round(x, 4), "y": round(y, 4)} for x, y in handles])


def import_jmri_layout(xml_text, inner_radius_mm):
    """
    Convert a JMRI layout-config XML string.

    Returns a dict:
      track_pieces: placed pieces, ready to store in a layout's elements
      unplaced: pieces that were not placed, with the reason
      scale_mm_per_pixel: the scale worked out from the innermost curve
      hand_mismatches: turnouts whose drawn diverging side disagrees with their hand
      unanchored_turnouts: turnouts placed from the file's symbol because no anchor joins them
      ambiguous_curves: curves whose bulge could not be confirmed from the track they join
      gaps: turnout legs that don't reach their anchor, with the gap in mm, largest first
    """
    if not isinstance(inner_radius_mm, (int, float)) or not inner_radius_mm > 0:
        raise ValueError("The inner curve radius must be a positive number of mm")

    root = _parse(xml_text)
    raw = _raw_points(root)

    radii_px = _arc_radii_px(root, raw)
    if not radii_px:
        raise ValueError("No 90 degree curves in the layout, so the scale cannot be worked out")
    scale = inner_radius_mm / min(radii_px.values())

    library = load_library()
    links, connectors = _connectors(root)
    groups = _three_way_groups(root, links)
    group_members = {ident for idents, _ in groups for ident in idents}
    turnout_records, ends, heading_out, mismatches, unplaced, unanchored, gaps, supported = _place_turnouts(
        root, raw, scale, library, links, group_members
    )
    three_way_records, three_way_gaps, three_way_members, three_way_points = _place_three_ways(
        groups, raw, scale, library, links
    )
    slip_records, slip_gaps, slip_unplaced, slips, slip_points = _place_slips(root, raw, scale, library, links)
    gaps += slip_gaps + three_way_gaps
    unplaced += slip_unplaced
    supported = supported | slips | three_way_members
    lines = _segment_lines(root, raw)
    for anchor, headings in _joint_lines(links, heading_out).items():
        lines.setdefault(anchor, []).extend(headings)

    # Track ends that join a turnout, slip, or three-way leg take that leg's own point. The leg is where the
    # piece actually is, and the anchor can be a fraction of a mm away
    joined_points = {
        anchor: tuple(ends[leg])
        for leg, anchor in links.items()
        if leg in ends and anchor[1] == "POS_POINT"
    }
    joined_points.update(slip_points)
    joined_points.update(three_way_points)
    segment_records, ambiguous, segment_unplaced = _place_segments(
        root, raw, radii_px, scale, connectors, supported, lines, joined_points
    )

    unplaced += segment_unplaced

    pieces = turnout_records + three_way_records + slip_records + segment_records
    # Move the layout so it starts at the margin, and size the layout to fit it
    anchors = [(point[0] * scale, point[1] * scale) for key, point in raw.items() if key[1] == "POS_POINT"]
    xs = [point[0] for point in anchors] + [record["x"] for record in pieces]
    ys = [point[1] for point in anchors] + [record["y"] for record in pieces]
    dx, dy = MARGIN_MM - min(xs), MARGIN_MM - min(ys)
    for record in pieces:
        record["x"] = round(record["x"] + dx, 4)
        record["y"] = round(record["y"] + dy, 4)

    return {
        "track_pieces": pieces,
        "width": math.ceil(max(xs) - min(xs) + 2 * MARGIN_MM),
        "height": math.ceil(max(ys) - min(ys) + 2 * MARGIN_MM),
        "track_system": "Code 55",
        "unplaced": unplaced,
        "scale_mm_per_pixel": round(scale, 4),
        "hand_mismatches": mismatches,
        "unanchored_turnouts": unanchored,
        "ambiguous_curves": ambiguous,
        "gaps": sorted(gaps, key=lambda gap: -gap["gap_mm"]),
    }
