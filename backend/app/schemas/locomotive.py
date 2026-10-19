"""
Locomotive schemas
"""
from pydantic import BaseModel
from typing import Optional
from datetime import datetime


class LocomotiveCreate(BaseModel):
    """Schema for creating a locomotive"""
    name: str
    dcc_address: int
    locomotive_type: str
    command_station_id: int
    length: Optional[float] = None


class LocomotiveUpdate(BaseModel):
    """Schema for updating a locomotive"""
    name: Optional[str] = None
    speed: Optional[int] = None
    direction: Optional[str] = None
    functions: Optional[int] = None


class LocomotiveResponse(BaseModel):
    """Schema for locomotive response"""
    id: int
    name: str
    dcc_address: int
    locomotive_type: str
    speed: int
    direction: str
    functions: int
    travelled_time: float
    travelled_distance: float
    created_at: datetime

    class Config:
        from_attributes = True
