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
                # order[k] is the local end paired with world point k
                local_of_world = list(order)
                best = (origin, rotation, flip, local_of_world, error)
    return best


# Curved flex track: a cubic Bezier from the origin to (length, 0), with two handles off the track. The first
# handle lies along the start and the second along the end, so the track leaves and arrives at the angles the
# handles give. The shape is the same as the layout editor's (frontend/src/layout/curves.js), so a fit here is
# what the editor will draw
BEZIER_STEPS = 24


def bezier_path(length, handles):
    """Points along flex track of `length` curved by its two handles"""
    p0, p1 = (0.0, 0.0), handles[0]
    p2, p3 = handles[1], (length, 0.0)
    path = []
    for step in range(BEZIER_STEPS + 1):
        t = step / BEZIER_STEPS
        u = 1 - t
        a, b, c, d = u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t
        path.append((
            a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0],
            a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1],
        ))
    return path


def fit_handles(samples, length, start_angle, end_angle):
    """Handles for flex track whose path follows `samples` as closely as possible.

    samples are points of the curve in the track's own frame (origin at its start, x along its chord).
    The track leaves its start at start_angle and arrives at its end at end_angle, both in radians, and
    keeps its length. Each handle stays on its end's line, so the end angles are kept exactly.
    """
    def build(r1, r2):
        return [
            (r1 * math.cos(start_angle), r1 * math.sin(start_angle)),
            (length - r2 * math.cos(end_angle), -r2 * math.sin(end_angle)),
        ]

    def cost(r1, r2):
        path = bezier_path(length, build(r1, r2))
        return sum(min(math.dist(sample, point) for point in path) ** 2 for sample in samples)

    low = 0.02 * length
    best = (length / 3, length / 3, float("inf"))
    grid = 16
    for i in range(1, grid + 1):
        for j in range(1, grid + 1):
            r1 = low + (i / grid) * (1.2 * length - low)
            r2 = low + (j / grid) * (1.2 * length - low)
            value = cost(r1, r2)
            if value < best[2]:
                best = (r1, r2, value)
    spread = 1.2 * length / grid
    for _ in range(4):
        centre_1, centre_2 = best[0], best[1]
        for i in range(-3, 4):
            for j in range(-3, 4):
                r1 = max(low, centre_1 + i * spread / 3)
                r2 = max(low, centre_2 + j * spread / 3)
                value = cost(r1, r2)
                if value < best[2]:
                    best = (r1, r2, value)
        spread /= 3
    return build(best[0], best[1])


def arc_bend_samples(radius, sweep, flip, count=24):
    """Samples of a circular arc from the origin, in the frame where it starts along the x axis, and its chord.
    Returns (samples in the chord frame, chord length, chord angle in radians, start angle, end angle).
    The arc turns toward +y, or toward -y when flip is set. The chord frame is the one with the chord along x.
    """
    sign = -1.0 if flip else 1.0
    local = [
        (radius * math.sin(sweep * k / (count - 1)), sign * radius * (1 - math.cos(sweep * k / (count - 1))))
        for k in range(count)
    ]
    end = local[-1]
    chord = math.hypot(end[0], end[1])
    angle = math.atan2(end[1], end[0])
    samples = [_rotate_radians(point, -angle) for point in local]
    start_tangent = -angle
    end_tangent = sign * sweep - angle
    return samples, chord, angle, start_tangent, end_tangent


def bezier_bend_samples(start, control_start, control_end, end, count=24):
    """Samples of a cubic Bezier in the chord frame, with the chord length, the chord angle, and the end angles.
    Returns (samples, chord length, chord angle in radians, start angle, end angle), all in the chord frame.
    """
    def point(t):
        u = 1 - t
        return (
            u ** 3 * start[0] + 3 * u * u * t * control_start[0] + 3 * u * t * t * control_end[0] + t ** 3 * end[0],
            u ** 3 * start[1] + 3 * u * u * t * control_start[1] + 3 * u * t * t * control_end[1] + t ** 3 * end[1],
        )

    chord_vector = (end[0] - start[0], end[1] - start[1])
    chord = math.hypot(*chord_vector)
    angle = math.atan2(chord_vector[1], chord_vector[0])
    samples = [
        _rotate_radians((point(k / (count - 1))[0] - start[0], point(k / (count - 1))[1] - start[1]), -angle)
        for k in range(count)
    ]
    first = control_start if math.dist(start, control_start) > 1e-6 else control_end
    last = control_end if math.dist(end, control_end) > 1e-6 else control_start
    start_tangent = math.atan2(first[1] - start[1], first[0] - start[0]) - angle
    end_tangent = math.atan2(end[1] - last[1], end[0] - last[0]) - angle
    return samples, chord, angle, start_tangent, end_tangent


def _rotate_radians(point, radians):
    return (
        point[0] * math.cos(radians) - point[1] * math.sin(radians),
        point[0] * math.sin(radians) + point[1] * math.cos(radians),
    )
