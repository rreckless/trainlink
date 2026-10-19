"""
Locomotive model
"""
from sqlalchemy import Column, Integer, String, Float, Boolean, DateTime, ForeignKey
from sqlalchemy.orm import relationship
from datetime import datetime
from ..database import Base


class Locomotive(Base):
    """Model for locomotives/trains"""
    __tablename__ = "locomotives"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, unique=True, index=True)
    dcc_address = Column(Integer, unique=True)

    # Physical properties
    locomotive_type = Column(String)  # steam, diesel, electric, etc.
    length = Column(Float, nullable=True)  # in cm

    # State
    speed = Column(Integer, default=0)  # 0-100
    direction = Column(String, default="stop")  # forward, reverse, stop

    # Functions (lights, sounds, etc.)
    functions = Column(Integer, default=0)  # Bitmask for DCC functions

    # Tracking
    travelled_time = Column(Float, default=0)  # seconds
    travelled_distance = Column(Float, default=0)  # distance units

    # Metadata
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    # Relationships
    # Decoder profiles are Phase 5. No table yet, so no foreign key
    decoder_profile_id = Column(Integer, nullable=True)
    command_station_id = Column(Integer, ForeignKey("command_stations.id"))

    def to_dict(self):
        return {
            "id": self.id,
            "name": self.name,
            "dcc_address": self.dcc_address,
            "locomotive_type": self.locomotive_type,
            "speed": self.speed,
            "direction": self.direction,
            "functions": self.functions,
            "travelled_time": self.travelled_time,
            "travelled_distance": self.travelled_distance,
        }
