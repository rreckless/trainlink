"""
Route REST endpoints

Route execution (automatic driving) is not built yet. is_running cannot be set here.
"""
from typing import List, Optional

from fastapi import APIRouter, Depends, HTTPException, Response, status
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import Block, Layout, Route
from ..schemas import RouteCreate, RouteResponse, RouteUpdate
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/routes", tags=["routes"])


def _check_block_sequence(db: Session, layout_id: int, block_sequence: List[int]):
    """Every block in the sequence must exist and belong to the same layout"""
    if not block_sequence:
        raise HTTPException(status_code=400, detail="Route needs at least one block")

    found = {
        block_id
        for (block_id,) in db.query(Block.id).filter(
            Block.id.in_(block_sequence), Block.layout_id == layout_id
        )
    }
    missing = [block_id for block_id in block_sequence if block_id not in found]
    if missing:
        raise HTTPException(
            status_code=400,
            detail=f"Blocks not in layout {layout_id}: {missing}",
        )


@router.get("", response_model=List[RouteResponse])
def list_routes(layout_id: Optional[int] = None, db: Session = Depends(get_db)):
    query = db.query(Route)
    if layout_id is not None:
        query = query.filter(Route.layout_id == layout_id)
    return query.order_by(Route.id).all()


@router.post("", response_model=RouteResponse, status_code=status.HTTP_201_CREATED)
def create_route(body: RouteCreate, db: Session = Depends(get_db)):
    get_or_404(db, Layout, body.layout_id, "Layout")
    _check_block_sequence(db, body.layout_id, body.block_sequence)

    route = Route(
        layout_id=body.layout_id,
        name=body.name,
        block_sequence=body.block_sequence,
        speed_profile=body.speed_profile,
        dwell_times=body.dwell_times,
        auto_return=body.auto_return,
    )
    db.add(route)
    db.commit()
    db.refresh(route)
    return route


@router.get("/{route_id}", response_model=RouteResponse)
def get_route(route_id: int, db: Session = Depends(get_db)):
    return get_or_404(db, Route, route_id, "Route")


@router.patch("/{route_id}", response_model=RouteResponse)
def update_route(route_id: int, body: RouteUpdate, db: Session = Depends(get_db)):
    changes = body.model_dump(exclude_unset=True)
    if "is_running" in changes:
        raise HTTPException(status_code=400, detail="Route execution is not built yet")
    reject_nulls(changes)

    route = get_or_404(db, Route, route_id, "Route")
    if "block_sequence" in changes:
        _check_block_sequence(db, route.layout_id, changes["block_sequence"])
    for field, value in changes.items():
        setattr(route, field, value)
    db.commit()
    db.refresh(route)
    return route


@router.delete("/{route_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_route(route_id: int, db: Session = Depends(get_db)):
    route = get_or_404(db, Route, route_id, "Route")
    if route.is_running:
        raise HTTPException(status_code=409, detail="Cannot delete a running route")
    db.delete(route)
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
