"""
Track library: manufacturer track pieces loaded from JSON files.

Each file in app/data/track_library/ holds one manufacturer and gauge.
Pieces are identified by "<manufacturer>-<gauge>-<part number>", lowercased.
"""
import json
from functools import lru_cache
from pathlib import Path

LIBRARY_DIR = Path(__file__).resolve().parent.parent / "data" / "track_library"

# Fields copied from the file into each piece. Geometry fields are present only where they apply
PIECE_FIELDS = (
    "part_number",
    "system",
    "name",
    "type",
    "length",
    "radius",
    "angle",
    "hand",
    "inner_radius",
    "outer_radius",
    "placeable",
    "angle_meaning",
    "note",
)


def _piece_id(manufacturer: str, gauge: str, part_number: str) -> str:
    return f"{manufacturer}-{gauge}-{part_number}".lower().replace(" ", "-")


@lru_cache(maxsize=1)
def load_library() -> dict:
    """Return {piece_id: piece} for every piece in every library file"""
    library = {}
    for path in sorted(LIBRARY_DIR.glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        for raw in data["pieces"]:
            piece_id = _piece_id(data["manufacturer"], data["gauge"], raw["part_number"])
            if piece_id in library:
                raise ValueError(f"Duplicate track library piece: {piece_id}")
            piece = {key: raw[key] for key in PIECE_FIELDS if key in raw}
            piece.update(
                id=piece_id,
                manufacturer=data["manufacturer"],
                gauge=data["gauge"],
                verified=data.get("verified", False),
                source=data.get("source", ""),
            )
            library[piece_id] = piece
    return library


def list_pieces(system=None, gauge=None, manufacturer=None) -> list:
    """Return pieces, optionally filtered. Sorted by manufacturer, system, then part number"""
    pieces = list(load_library().values())
    if system is not None:
        pieces = [piece for piece in pieces if piece["system"] == system]
    if gauge is not None:
        pieces = [piece for piece in pieces if piece["gauge"] == gauge]
    if manufacturer is not None:
        pieces = [piece for piece in pieces if piece["manufacturer"] == manufacturer]
    return sorted(pieces, key=lambda p: (p["manufacturer"], p["system"], p["part_number"]))
