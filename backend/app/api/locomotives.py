"""
Locomotive REST endpoints

Driving a locomotive goes through /api/z21/loco/speed, not this API.
Speed and direction are not changed here.
"""
from pathlib import Path
from typing import List

from fastapi import APIRouter, Depends, File, HTTPException, Response, UploadFile, status
from fastapi.responses import FileResponse
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import CommandStation, Locomotive
from ..schemas import LocomotiveCreate, LocomotiveResponse, LocomotiveUpdate
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/locomotives", tags=["locomotives"])

# Pictures are stored here, one file per locomotive, named by its id
IMAGE_DIR = Path(__file__).resolve().parents[2] / "data" / "locomotive_images"
IMAGE_MAX_BYTES = 2 * 1024 * 1024
# Content type, file extension, and the bytes every file of that type starts with
IMAGE_TYPES = {
    "image/jpeg": ("jpg", b"\xff\xd8\xff"),
    "image/png": ("png", b"\x89PNG\r\n\x1a\n"),
    "image/webp": ("webp", b"RIFF"),
}

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
    if "dcc_address" in changes:
        _check_dcc_address(changes["dcc_address"])

    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    for field, value in changes.items():
        setattr(locomotive, field, value)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="Another locomotive already has this name or DCC address")
    db.refresh(locomotive)
    return locomotive


@router.post("/{locomotive_id}/image", response_model=LocomotiveResponse)
async def upload_locomotive_image(locomotive_id: int, file: UploadFile = File(...), db: Session = Depends(get_db)):
    """Set the locomotive's picture. JPEG, PNG, or WebP, up to 2 MB"""
    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    if file.content_type not in IMAGE_TYPES:
        raise HTTPException(status_code=400, detail="Picture must be a JPEG, PNG, or WebP image")
    data = await file.read(IMAGE_MAX_BYTES + 1)
    if len(data) > IMAGE_MAX_BYTES:
        raise HTTPException(status_code=413, detail="Picture must be 2 MB or smaller")
    extension, signature = IMAGE_TYPES[file.content_type]
    if not data.startswith(signature) or (file.content_type == "image/webp" and data[8:12] != b"WEBP"):
        raise HTTPException(status_code=400, detail="The file is not a valid image of that type")

    IMAGE_DIR.mkdir(parents=True, exist_ok=True)
    _delete_image_file(locomotive)
    name = f"{locomotive.id}.{extension}"
    (IMAGE_DIR / name).write_bytes(data)
    locomotive.image_file = name
    db.commit()
    db.refresh(locomotive)
    return locomotive


@router.get("/{locomotive_id}/image")
def get_locomotive_image(locomotive_id: int, db: Session = Depends(get_db)):
    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    if not locomotive.image_file or not (IMAGE_DIR / locomotive.image_file).is_file():
        raise HTTPException(status_code=404, detail="Locomotive has no picture")
    media_type = next(kind for kind, (extension, _) in IMAGE_TYPES.items() if locomotive.image_file.endswith(extension))
    return FileResponse(IMAGE_DIR / locomotive.image_file, media_type=media_type)


@router.delete("/{locomotive_id}/image", response_model=LocomotiveResponse)
def delete_locomotive_image(locomotive_id: int, db: Session = Depends(get_db)):
    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    _delete_image_file(locomotive)
    locomotive.image_file = None
    db.commit()
    db.refresh(locomotive)
    return locomotive


def _delete_image_file(locomotive: Locomotive):
    if locomotive.image_file:
        (IMAGE_DIR / locomotive.image_file).unlink(missing_ok=True)


@router.delete("/{locomotive_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_locomotive(locomotive_id: int, db: Session = Depends(get_db)):
    locomotive = get_or_404(db, Locomotive, locomotive_id, "Locomotive")
    _delete_image_file(locomotive)
    db.delete(locomotive)
    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        raise HTTPException(status_code=409, detail="Locomotive is in use by a block or route")
    return Response(status_code=status.HTTP_204_NO_CONTENT)
