"""
Shared helpers for REST routers
"""
from fastapi import HTTPException
from sqlalchemy.orm import Session


def get_or_404(db: Session, model, item_id: int, label: str):
    """Return the row with this id, or raise a 404 naming the entity"""
    item = db.get(model, item_id)
    if item is None:
        raise HTTPException(status_code=404, detail=f"{label} {item_id} not found")
    return item


def reject_nulls(changes: dict):
    """Refuse explicit nulls in a PATCH body. Nullable columns are not used for these fields"""
    nulls = [field for field, value in changes.items() if value is None]
    if nulls:
        raise HTTPException(status_code=400, detail=f"Fields cannot be null: {', '.join(nulls)}")
