"""
Route schemas
"""
from pydantic import BaseModel
from typing import Optional, List, Dict
from datetime import datetime


class RouteCreate(BaseModel):
    """Schema for creating a route"""
    layout_id: int
    name: str
    block_sequence: List[int]
    speed_profile: Optional[Dict] = None
    dwell_times: Optional[Dict] = None
    auto_return: bool = False


class RouteUpdate(BaseModel):
    """Schema for updating a route"""
    name: Optional[str] = None
    block_sequence: Optional[List[int]] = None
    speed_profile: Optional[Dict] = None
    enabled: Optional[bool] = None
    is_running: Optional[bool] = None


class RouteResponse(BaseModel):
    """Schema for route response"""
    id: int
    layout_id: int
    name: str
    block_sequence: List[int]
    speed_profile: Optional[Dict]
    dwell_times: Optional[Dict]
    auto_return: bool
    enabled: bool
    is_running: bool
    current_block_index: int
    created_at: datetime

    class Config:
        from_attributes = True
