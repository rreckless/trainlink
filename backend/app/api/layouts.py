"""
Layout REST endpoints

Track segments are stored in layout.elements["tracks"]. Blocks, signals, and
feedbacks are separate rows that reference the layout by id.
"""
from typing import List

from fastapi import APIRouter, Depends, HTTPException, Response, status
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import Block, Feedback, Layout, Route, Signal
from ..schemas import AnyRailImportRequest, LayoutCreate, LayoutResponse, LayoutUpdate
from ..services.anyrail_import import import_jmri_layout
from ..services.track_library import load_library
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/layouts", tags=["layouts"])

TRACK_KEYS = ("x1", "y1", "x2", "y2")


def _check_positive(width: float, height: float):
    if width <= 0 or height <= 0:
        raise HTTPException(status_code=400, detail="Width and height must be positive")


def _check_elements(elements: dict):
    """Validate straight track segments. Other keys are stored as given"""
    tracks = elements.get("tracks", [])
    if not isinstance(tracks, list):
        raise HTTPException(status_code=400, detail="elements.tracks must be a list")
    for track in tracks:
        if not isinstance(track, dict) or not all(
            isinstance(track.get(key), (int, float)) for key in TRACK_KEYS
        ):
            raise HTTPException(
                status_code=400,
                detail=f"Each track needs numeric {', '.join(TRACK_KEYS)}",
            )
    _check_track_pieces(elements)


# Per-placement dimensions and the piece types that accept each one, with the largest value allowed (mm or degrees)
PLACEMENT_DIMENSIONS = {
    "length": (("straight", "flex"), 100_000),
    "radius": (("curve",), 10_000),
    "angle": (("curve",), 360),
}


def _valid_dimensions(placed: dict, piece: dict) -> bool:
    """Optional length, radius, and angle on a placement, which must suit the piece's type"""
    for key, (types, largest) in PLACEMENT_DIMENSIONS.items():
        if key not in placed:
            continue
        value = placed[key]
        if piece["type"] not in types or isinstance(value, bool) or not isinstance(value, (int, float)):
            return False
        if not 0 < value <= largest:
            return False
    # A curve without a radius from the library needs one on the placement
    return not (piece["type"] == "curve" and "radius" not in placed and "radius" not in piece)


def _valid_dcc(placed: dict, piece: dict) -> bool:
    """DCC address (1-2048) and straight output (0 or 1), which only turnouts may carry"""
    if "dcc_address" not in placed and "dcc_output_straight" not in placed:
        return True
    if piece["type"] != "turnout":
        return False
    address = placed.get("dcc_address")
    if "dcc_address" in placed and (isinstance(address, bool) or not isinstance(address, int)
                                    or not 1 <= address <= 2048):
        return False
    straight = placed.get("dcc_output_straight", 0)
    return isinstance(straight, int) and not isinstance(straight, bool) and straight in (0, 1)


def _valid_handles(placed: dict, piece: dict) -> bool:
    """Two curve handles (x and y in mm, in the piece's own frame), which only flex track may have"""
    if "handles" not in placed:
        return True
    if piece["type"] != "flex":
        return False
    handles = placed["handles"]
    if not isinstance(handles, list) or len(handles) != 2:
        return False
    for point in handles:
        if not isinstance(point, dict):
            return False
        for key in ("x", "y"):
            value = point.get(key)
            if isinstance(value, bool) or not isinstance(value, (int, float)) or abs(value) > 10_000:
                return False
    return True


def _check_track_pieces(elements: dict):
    """Placed library pieces must exist, share one track system, and match elements.track_system"""
    pieces = elements.get("track_pieces", [])
    if not isinstance(pieces, list):
        raise HTTPException(status_code=400, detail="elements.track_pieces must be a list")

    library = load_library()
    systems = set()
    for placed in pieces:
        valid = (
            isinstance(placed, dict)
            and isinstance(placed.get("id"), str)
            and placed.get("piece_id") in library
            and all(isinstance(placed.get(key), (int, float)) for key in ("x", "y", "rotation"))
            and isinstance(placed.get("flip"), bool)
        )
        if not valid:
            raise HTTPException(
                status_code=400,
                detail="Each track piece needs an id, a known piece_id, x, y, rotation, and a flip flag",
            )
        if not _valid_dimensions(placed, library[placed["piece_id"]]):
            raise HTTPException(
                status_code=400,
                detail="A placed length, radius, or angle must be a positive number that suits the piece",
            )
        if not _valid_handles(placed, library[placed["piece_id"]]):
            raise HTTPException(
                status_code=400,
                detail="Bend points need two x and y values in mm, and only flex track can have them",
            )
        if not _valid_dcc(placed, library[placed["piece_id"]]):
            raise HTTPException(
                status_code=400,
                detail="A DCC address (1-2048) or straight output (0 or 1) must suit the piece. Only turnouts take them",
            )
        systems.add(library[placed["piece_id"]]["system"])

    addresses = [placed["dcc_address"] for placed in pieces if "dcc_address" in placed]
    if len(addresses) != len(set(addresses)):
        raise HTTPException(status_code=400, detail="A DCC turnout address can only be used once in a layout")

    if len(systems) > 1:
        raise HTTPException(status_code=400, detail="A layout cannot mix track systems")
    if systems and elements.get("track_system") != next(iter(systems)):
        raise HTTPException(
            status_code=400,
            detail=f"elements.track_system must be {next(iter(systems))!r} for these pieces",
        )


@router.get("", response_model=List[LayoutResponse])
def list_layouts(db: Session = Depends(get_db)):
    return db.query(Layout).order_by(Layout.id).all()


@router.post("", response_model=LayoutResponse, status_code=status.HTTP_201_CREATED)
def create_layout(body: LayoutCreate, db: Session = Depends(get_db)):
    _check_positive(body.width, body.height)

    layout = Layout(
        name=body.name,
        description=body.description,
        width=body.width,
        height=body.height,
        scale=body.scale,
        track_gauge=body.track_gauge,
        elements={"tracks": []},
    )
    db.add(layout)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="A layout with this name already exists")
    db.refresh(layout)
    return layout


@router.post("/anyrail-import")
def preview_anyrail_import(body: AnyRailImportRequest):
    """Convert an AnyRail export to track pieces. Nothing is saved; the editor saves the pieces with the layout"""
    try:
        result = import_jmri_layout(body.xml, body.inner_radius_mm)
    except ValueError as error:
        raise HTTPException(status_code=400, detail=str(error)) from error
    _check_track_pieces({"track_system": result["track_system"], "track_pieces": result["track_pieces"]})
    return result


@router.get("/{layout_id}", response_model=LayoutResponse)
def get_layout(layout_id: int, db: Session = Depends(get_db)):
    return get_or_404(db, Layout, layout_id, "Layout")


@router.patch("/{layout_id}", response_model=LayoutResponse)
def update_layout(layout_id: int, body: LayoutUpdate, db: Session = Depends(get_db)):
    changes = body.model_dump(exclude_unset=True)
    reject_nulls(changes)
    if "width" in changes or "height" in changes:
        layout = get_or_404(db, Layout, layout_id, "Layout")
        _check_positive(changes.get("width", layout.width), changes.get("height", layout.height))
    if "elements" in changes:
        _check_elements(changes["elements"])
    if "name" in changes:
        changes["name"] = changes["name"].strip()
        if not changes["name"]:
            raise HTTPException(status_code=400, detail="A layout needs a name")

    layout = get_or_404(db, Layout, layout_id, "Layout")
    for field, value in changes.items():
        setattr(layout, field, value)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="A layout with this name already exists")
    db.refresh(layout)
    return layout


@router.delete("/{layout_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_layout(layout_id: int, db: Session = Depends(get_db)):
    layout = get_or_404(db, Layout, layout_id, "Layout")

    # Checked here rather than left to the database, so it behaves the same on every backend
    in_use = [
        model.__name__.lower() + "s"
        for model in (Block, Signal, Feedback, Route)
        if db.query(model).filter(model.layout_id == layout_id).first() is not None
    ]
    if in_use:
        raise HTTPException(
            status_code=409,
            detail=f"Layout still has {', '.join(in_use)}. Delete them first",
        )

    db.delete(layout)
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
