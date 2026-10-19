"""
Layout schemas
"""
from pydantic import BaseModel, Field
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


class AnyRailImportRequest(BaseModel):
    """A JMRI layout-config XML export from AnyRail, and the real radius in mm of its innermost curve"""
    xml: str = Field(min_length=1, max_length=5_000_000)
    inner_radius_mm: float = Field(gt=0, le=10_000)
