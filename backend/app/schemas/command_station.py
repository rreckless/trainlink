"""
Command Station schemas
"""
from pydantic import BaseModel
from typing import Optional, Dict, Any
from datetime import datetime


class CommandStationCreate(BaseModel):
    """Schema for creating a command station"""
    name: str
    station_type: str
    port: str
    settings: Optional[Dict[str, Any]] = None


class CommandStationUpdate(BaseModel):
    """Schema for updating a command station"""
    name: Optional[str] = None
    port: Optional[str] = None
    settings: Optional[Dict[str, Any]] = None


class CommandStationResponse(BaseModel):
    """Schema for command station response"""
    id: int
    name: str
    station_type: str
    port: str
    connected: bool
    last_heartbeat: Optional[datetime]
    firmware_version: Optional[str]
    supported_features: Dict[str, Any]
    created_at: datetime

    class Config:
        from_attributes = True
