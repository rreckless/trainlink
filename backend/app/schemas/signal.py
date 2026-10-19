"""
Signal schemas
"""
from pydantic import BaseModel
from typing import Optional, List
from datetime import datetime


class SignalCreate(BaseModel):
    """Schema for creating a signal"""
    layout_id: int
    name: str
    x: float
    y: float
    signal_type: str  # main or warning


class SignalUpdate(BaseModel):
    """Schema for updating a signal"""
    name: Optional[str] = None
    x: Optional[float] = None
    y: Optional[float] = None
    aspect: Optional[str] = None  # stop, caution, go


class SignalResponse(BaseModel):
    """Schema for signal response"""
    id: int
    layout_id: int
    name: str
    x: float
    y: float
    signal_type: str
    aspect: str
    block_ids: List[int]
    created_at: datetime

    class Config:
        from_attributes = True
