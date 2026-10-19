"""
Track library REST endpoints (read-only)
"""
from typing import List, Optional

from fastapi import APIRouter, HTTPException

from ..services.track_library import list_pieces, load_library

router = APIRouter(prefix="/api/track-library", tags=["track-library"])


@router.get("")
def get_track_library(
    system: Optional[str] = None,
    gauge: Optional[str] = None,
    manufacturer: Optional[str] = None,
) -> List[dict]:
    return list_pieces(system=system, gauge=gauge, manufacturer=manufacturer)


@router.get("/{piece_id}")
def get_track_piece(piece_id: str) -> dict:
    piece = load_library().get(piece_id)
    if piece is None:
        raise HTTPException(status_code=404, detail=f"Track piece {piece_id} not found")
    return piece
