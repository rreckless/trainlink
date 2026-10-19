"""
Route model - represents automated train routes
"""
from sqlalchemy import Column, Integer, String, Float, Boolean, DateTime, ForeignKey, JSON
from datetime import datetime
from ..database import Base


class Route(Base):
    """Model for routes (sequences of blocks for automation)"""
    __tablename__ = "routes"

    id = Column(Integer, primary_key=True, index=True)
    layout_id = Column(Integer, ForeignKey("layouts.id"))
    name = Column(String, index=True)

    # Route definition
    block_sequence = Column(JSON)  # List of block IDs in order

    # Speed profile
    speed_profile = Column(JSON, nullable=True)  # Speed for each segment

    # Timing
    dwell_times = Column(JSON, nullable=True)  # Time to wait at each block

    # Options
    auto_return = Column(Boolean, default=False)  # Return to start automatically
    enabled = Column(Boolean, default=True)

    # Execution state
    is_running = Column(Boolean, default=False)
    current_locomotive_id = Column(Integer, ForeignKey("locomotives.id"), nullable=True)
    current_block_index = Column(Integer, default=0)

    # Metadata
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    def to_dict(self):
        return {
            "id": self.id,
            "layout_id": self.layout_id,
            "name": self.name,
            "block_sequence": self.block_sequence,
            "speed_profile": self.speed_profile,
            "dwell_times": self.dwell_times,
            "auto_return": self.auto_return,
            "enabled": self.enabled,
            "is_running": self.is_running,
            "current_locomotive_id": self.current_locomotive_id,
            "current_block_index": self.current_block_index,
        }
