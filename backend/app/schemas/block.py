"""
Block schemas
"""
from pydantic import BaseModel
from typing import Optional, List
from datetime import datetime


class BlockCreate(BaseModel):
    """Schema for creating a block"""
    layout_id: int
    name: str
    x: float
    y: float
    max_speed: Optional[int] = 100
    length: Optional[float] = None


class BlockUpdate(BaseModel):
    """Schema for updating a block"""
    name: Optional[str] = None
    x: Optional[float] = None
    y: Optional[float] = None
    max_speed: Optional[int] = None
    state: Optional[str] = None


class BlockResponse(BaseModel):
    """Schema for block response"""
    id: int
    layout_id: int
    name: str
    x: float
    y: float
    state: str
    occupied: bool
    max_speed: int
    signal_ids: List[int]
    feedback_ids: List[int]
    created_at: datetime

    class Config:
        from_attributes = True
