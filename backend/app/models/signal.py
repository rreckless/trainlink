"""
Signal model - represents block signals
"""
from sqlalchemy import Column, Integer, String, Float, DateTime, ForeignKey
from sqlalchemy.orm import relationship
from datetime import datetime
from ..database import Base


class Signal(Base):
    """Model for signals"""
    __tablename__ = "signals"

    id = Column(Integer, primary_key=True, index=True)
    layout_id = Column(Integer, ForeignKey("layouts.id"))
    name = Column(String, index=True)

    # Visual position
    x = Column(Float)
    y = Column(Float)

    # Signal properties
    signal_type = Column(String)  # main, warning

    # Current state
    aspect = Column(String, default="stop")  # stop, caution, go

    # Relationships
    blocks = relationship("Block", secondary="block_signals", back_populates="signals")

    # Metadata
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    @property
    def block_ids(self):
        return [b.id for b in self.blocks]

    def to_dict(self):
        return {
            "id": self.id,
            "layout_id": self.layout_id,
            "name": self.name,
            "x": self.x,
            "y": self.y,
            "signal_type": self.signal_type,
            "aspect": self.aspect,
            "block_ids": self.block_ids,
        }
