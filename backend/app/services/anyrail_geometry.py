"""
Geometry for the AnyRail import: angles, placements, and the shapes of turnouts, slips, three-ways, and arcs.

Angles are in degrees. A piece's ends are (point, heading) pairs in its own frame, where the heading is
the direction of travel leaving the piece through that end. Flip mirrors a piece across its x axis first.
"""
import itertools
import math


def _angle(start, end):
    return math.degrees(math.atan2(end[1] - start[1], end[0] - start[0]))


def _normalize(degrees):
    return ((degrees % 360) + 360) % 360


def _line_difference(a, b):
    """Angle between two undirected lines, 0 to 90 degrees"""
    d = (a - b) % 180
    return min(d, 180 - d)


def _rotate(point, degrees):
    r = math.radians(degrees)
    x, y = point
    return (x * math.cos(r) - y * math.sin(r), x * math.sin(r) + y * math.cos(r))


def _mirror(point, flip):
    return (point[0], -point[1]) if flip else point


def _heading_for(heading, flip):
    return -heading if flip else heading


def _world(point, origin, rotation, flip):
    rx, ry = _rotate(_mirror(point, flip), rotation)
    return (origin[0] + rx, origin[1] + ry)


def _align(ends, index, point, required_heading):
    """Origin and rotation that put end `index` of an unflipped piece at `point`, heading `required_heading`"""
    local_point, local_heading = ends[index]
    rotation = required_heading - local_heading
    rx, ry = _rotate(local_point, rotation)
    return (point[0] - rx, point[1] - ry), rotation


def _straight_ends(length):
    return [((0.0, 0.0), 180.0), ((length, 0.0), 0.0)]


def _curve_ends(radius, angle=90.0):
    """Ends of an arc of `angle` degrees from the origin, turning toward +y"""
    r = math.radians(angle)
    return [((0.0, 0.0), 180.0), ((radius * math.sin(r), radius * (1 - math.cos(r))), angle)]


def _turnout_ends(piece):
    """Toe, straight, and diverging ends of a turnout in its own frame. Its diverging side is +y for right-hand"""
    length = piece["length"]
    side = 1 if piece["hand"] == "right" else -1
    geometry = piece.get("diverge_geometry")
    if geometry:
        radius, total = geometry["radius"], geometry["length"]
        sweep = math.asin((total - geometry["toe_straight"]) / radius)
        diverge = (total, side * radius * (1 - math.cos(sweep)))
    else:
        radius = piece["radius"]
        sweep = math.asin(length / radius)
        diverge = (length, side * radius * (1 - math.cos(sweep)))
    return [((0.0, 0.0), 180.0), ((length, 0.0), 0.0), (diverge, side * math.degrees(sweep))]


def _bezier_arc(start, control_start, control_end, end):
    """
    Circular arc standing in for a cubic Bezier: same ends and same start tangent.
    Returns (radius in pixels, angle in degrees, flip, start heading), or None if the curve is straight
    """
    tangent_point = control_start if math.dist(start, control_start) > 1e-6 else control_end
    start_heading = _angle(start, tangent_point)
    turn = (_angle(start, end) - start_heading + 180) % 360 - 180
    if abs(turn) < 0.5:
        return None
    angle = 2 * abs(turn)
    radius = math.dist(start, end) / (2 * math.sin(math.radians(angle) / 2))
    return radius, angle, turn < 0, start_heading


def _three_way_ends(piece):
    """Toe, straight end, right branch end, and left branch end of a three-way, in its own frame"""
    geometry = piece["three_way_geometry"]
    length = geometry["length"]

    def branch_end(branch, side):
        # A measured branch gives its sweep. Otherwise the branch ends on the straight's length
        if "sweep" in branch:
            sweep = math.radians(branch["sweep"])
        else:
            sweep = math.asin((length - branch["depart"]) / branch["radius"])
        return (branch["depart"] + branch["radius"] * math.sin(sweep), side * branch["radius"] * (1 - math.cos(sweep)))

    return [(0.0, 0.0), (length, 0.0), branch_end(geometry["right"], 1), branch_end(geometry["left"], -1)]


def _fit_four(local, world):
    """
    Placement that puts four local ends on four world points, trying every pairing of ends and both hands.
    Returns (origin, rotation, flip, local index for each world point, RMS error)
    """
    best = None
    for flip in (False, True):
        local_points = [_mirror(point, flip) for point in local]
        for order in itertools.permutations(range(len(world))):
            paired = [local_points[i] for i in order]
            # Best rotation between the two sets of points, about their centroids
            cl = (sum(p[0] for p in paired) / 4, sum(p[1] for p in paired) / 4)
            cw = (sum(p[0] for p in world) / 4, sum(p[1] for p in world) / 4)
            num = sum((q[0] - cl[0]) * (w[1] - cw[1]) - (q[1] - cl[1]) * (w[0] - cw[0]) for q, w in zip(paired, world))
            den = sum((q[0] - cl[0]) * (w[0] - cw[0]) + (q[1] - cl[1]) * (w[1] - cw[1]) for q, w in zip(paired, world))
            rotation = math.degrees(math.atan2(num, den))
            rx, ry = _rotate(cl, rotation)
            origin = (cw[0] - rx, cw[1] - ry)
            error = math.sqrt(sum(
                math.dist(_world(q, origin, rotation, False), w) ** 2 for q, w in zip(paired, world)
            ) / 4)
            if best is None or error < best[4]:
                local_of_world = [0] * 4
                for local_index, world_index in enumerate(order):
                    local_of_world[world_index] = local_index
                best = (origin, rotation, flip, local_of_world, error)
    return best
