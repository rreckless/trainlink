"""
Signal REST endpoints

Automatic aspect changes from block state are not built yet. The aspect set here is manual.
"""
from typing import List, Optional

from fastapi import APIRouter, Depends, HTTPException, Response, status
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import Layout, Signal
from ..schemas import SignalCreate, SignalResponse, SignalUpdate
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/signals", tags=["signals"])

SIGNAL_TYPES = {"main", "warning"}
SIGNAL_ASPECTS = {"stop", "caution", "go"}


def _check_choice(value: str, allowed: set, label: str):
    if value not in allowed:
        raise HTTPException(
            status_code=400,
            detail=f"{label} must be one of: {', '.join(sorted(allowed))}",
        )


@router.get("", response_model=List[SignalResponse])
def list_signals(layout_id: Optional[int] = None, db: Session = Depends(get_db)):
    query = db.query(Signal)
    if layout_id is not None:
        query = query.filter(Signal.layout_id == layout_id)
    return query.order_by(Signal.id).all()


@router.post("", response_model=SignalResponse, status_code=status.HTTP_201_CREATED)
def create_signal(body: SignalCreate, db: Session = Depends(get_db)):
    _check_choice(body.signal_type, SIGNAL_TYPES, "Signal type")
    get_or_404(db, Layout, body.layout_id, "Layout")

    signal = Signal(
        layout_id=body.layout_id,
        name=body.name,
        x=body.x,
        y=body.y,
        signal_type=body.signal_type,
    )
    db.add(signal)
    db.commit()
    db.refresh(signal)
    return signal


@router.get("/{signal_id}", response_model=SignalResponse)
def get_signal(signal_id: int, db: Session = Depends(get_db)):
    return get_or_404(db, Signal, signal_id, "Signal")


@router.patch("/{signal_id}", response_model=SignalResponse)
def update_signal(signal_id: int, body: SignalUpdate, db: Session = Depends(get_db)):
    changes = body.model_dump(exclude_unset=True)
    reject_nulls(changes)
    if "aspect" in changes:
        _check_choice(changes["aspect"], SIGNAL_ASPECTS, "Aspect")

    signal = get_or_404(db, Signal, signal_id, "Signal")
    for field, value in changes.items():
        setattr(signal, field, value)
    db.commit()
    db.refresh(signal)
    return signal


@router.delete("/{signal_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_signal(signal_id: int, db: Session = Depends(get_db)):
    signal = get_or_404(db, Signal, signal_id, "Signal")
    db.delete(signal)
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
