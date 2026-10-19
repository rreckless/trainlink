"""
Block REST endpoints

Occupancy comes from feedback sensors (not built yet). The state set here is manual.
"""
from typing import List, Optional

from fastapi import APIRouter, Depends, HTTPException, Response, status
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import Block, Layout
from ..schemas import BlockCreate, BlockResponse, BlockUpdate
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/blocks", tags=["blocks"])

BLOCK_STATES = {"free", "occupied", "reserved"}


def _check_state(state: str):
    if state not in BLOCK_STATES:
        raise HTTPException(
            status_code=400,
            detail=f"State must be one of: {', '.join(sorted(BLOCK_STATES))}",
        )


@router.get("", response_model=List[BlockResponse])
def list_blocks(layout_id: Optional[int] = None, db: Session = Depends(get_db)):
    query = db.query(Block)
    if layout_id is not None:
        query = query.filter(Block.layout_id == layout_id)
    return query.order_by(Block.id).all()


@router.post("", response_model=BlockResponse, status_code=status.HTTP_201_CREATED)
def create_block(body: BlockCreate, db: Session = Depends(get_db)):
    get_or_404(db, Layout, body.layout_id, "Layout")

    block = Block(
        layout_id=body.layout_id,
        name=body.name,
        x=body.x,
        y=body.y,
        max_speed=body.max_speed,
        length=body.length,
    )
    db.add(block)
    db.commit()
    db.refresh(block)
    return block


@router.get("/{block_id}", response_model=BlockResponse)
def get_block(block_id: int, db: Session = Depends(get_db)):
    return get_or_404(db, Block, block_id, "Block")


@router.patch("/{block_id}", response_model=BlockResponse)
def update_block(block_id: int, body: BlockUpdate, db: Session = Depends(get_db)):
    changes = body.model_dump(exclude_unset=True)
    reject_nulls(changes)
    if "state" in changes:
        _check_state(changes["state"])

    block = get_or_404(db, Block, block_id, "Block")
    for field, value in changes.items():
        setattr(block, field, value)
    if "state" in changes:
        block.occupied = changes["state"] == "occupied"
    db.commit()
    db.refresh(block)
    return block


@router.delete("/{block_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_block(block_id: int, db: Session = Depends(get_db)):
    block = get_or_404(db, Block, block_id, "Block")
    db.delete(block)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="Block is referenced by a route")
    return Response(status_code=status.HTTP_204_NO_CONTENT)
