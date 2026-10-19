"""
Command Station model - represents connected hardware command stations
"""
from sqlalchemy import Column, Integer, String, Boolean, DateTime, JSON
from datetime import datetime
from ..database import Base


class CommandStation(Base):
    """Model for command stations (hardware controllers)"""
    __tablename__ = "command_stations"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, unique=True, index=True)

    # Hardware identification
    station_type = Column(String)  # marklin_p50x, lenz_intellibox, roco_ecos, loconet, etc.
    port = Column(String)  # Serial port or network address

    # Connection state
    connected = Column(Boolean, default=False)
    last_heartbeat = Column(DateTime, nullable=True)

    # Capabilities
    firmware_version = Column(String, nullable=True)
    supported_features = Column(JSON, default={})

    # Settings
    settings = Column(JSON, default={})  # Protocol-specific settings

    # Metadata
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    def to_dict(self):
        return {
            "id": self.id,
            "name": self.name,
            "station_type": self.station_type,
            "port": self.port,
            "connected": self.connected,
            "last_heartbeat": self.last_heartbeat.isoformat() if self.last_heartbeat else None,
            "firmware_version": self.firmware_version,
            "supported_features": self.supported_features,
        }
