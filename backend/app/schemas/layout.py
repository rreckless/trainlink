"""
Layout schemas
"""
from pydantic import BaseModel
from typing import Optional, Dict, Any
from datetime import datetime


class LayoutCreate(BaseModel):
    """Schema for creating a layout"""
    name: str
    width: float
    height: float
    scale: str
    description: Optional[str] = None
    track_gauge: Optional[str] = None


class LayoutUpdate(BaseModel):
    """Schema for updating a layout"""
    name: Optional[str] = None
    description: Optional[str] = None
    width: Optional[float] = None
    height: Optional[float] = None
    elements: Optional[Dict[str, Any]] = None


class LayoutResponse(BaseModel):
    """Schema for layout response"""
    id: int
    name: str
    description: Optional[str]
    width: float
    height: float
    scale: str
    track_gauge: Optional[str]
    elements: Dict[str, Any]
    created_at: datetime

    class Config:
        from_attributes = True
