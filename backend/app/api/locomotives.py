"""
Locomotive REST endpoints

Driving a locomotive goes through /api/z21/loco/speed, not this API.
Speed and direction are not changed here.
"""
from typing import List

from fastapi import APIRouter, Depends, HTTPException, Response, status
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import CommandStation, Locomotive
from ..schemas import LocomotiveCreate, LocomotiveResponse, LocomotiveUpdate
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/locomotives", tags=["locomotives"])

MIN_DCC_ADDRESS = 1
MAX_DCC_ADDRESS = 9999


def _check_dcc_address(address: int):
    if not (MIN_DCC_ADDRESS <= address <= MAX_DCC_ADDRESS):
        raise HTTPException(
            status_code=400,
            detail=f"DCC address must be {MIN_DCC_ADDRESS}-{MAX_DCC_ADDRESS}",
        )


@router.get("", response_model=List[LocomotiveResponse])
def list_locomotives(db: Session = Depends(get_db)):
    return db.query(Locomotive).order_by(Locomotive.id).all()


@router.post("", response_model=LocomotiveResponse, status_code=status.HTTP_201_CREATED)
def create_locomotive(body: LocomotiveCreate, db: Session = Depends(get_db)):
    _check_dcc_address(body.dcc_address)
    if body.command_station_id is not None:
        get_or_404(db, CommandStation, body.command_station_id, "Command station")

    locomotive = Locomotive(
        name=body.name,
        dcc_address=body.dcc_address,
        locomotive_type=body.locomotive_type,
        length=body.length,
        command_station_id=body.command_station_id,
    )
    db.add(locomotive)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="Locomotive name or DCC address already exists")
    db.refresh(locomotive)
    return locomotive


@router.get("/{locomotive_id}", response_model=LocomotiveResponse)
def get_locomotive(locomotive_id: int, db: Session = Depends(get_db)):
    return get_or_404(db, Locomotive, locomotive_id, "Locomotive")


@router.patch("/{locomotive_id}", response_model=LocomotiveResponse)
def update_locomotive(locomotive_id: int, body: LocomotiveUpdate, db: Session = Depends(get_db)):
    changes = body.model_dump(exclude_unset=True)
    if "speed" in changes or "direction" in changes:
        raise HTTPException(
            status_code=400,
            detail="Speed and direction are set through /api/z21/loco/speed",
        )
    reject_nulls(changes)

    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    for field, value in changes.items():
        setattr(locomotive, field, value)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="Locomotive name already exists")
    db.refresh(locomotive)
    return locomotive


@router.delete("/{locomotive_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_locomotive(locomotive_id: int, db: Session = Depends(get_db)):
    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    db.delete(locomotive)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="Locomotive is in use by a block or route")
    return Response(status_code=status.HTTP_204_NO_CONTENT)
