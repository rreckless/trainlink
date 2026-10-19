"""
Feedback model - represents occupancy sensors
"""
from sqlalchemy import Column, Integer, String, Float, Boolean, DateTime, ForeignKey
from sqlalchemy.orm import relationship
from datetime import datetime
from ..database import Base


class Feedback(Base):
    """Model for feedback sensors (occupancy detection)"""
    __tablename__ = "feedbacks"

    id = Column(Integer, primary_key=True, index=True)
    layout_id = Column(Integer, ForeignKey("layouts.id"))
    name = Column(String, index=True)

    # Visual position
    x = Column(Float)
    y = Column(Float)

    # Feedback properties
    feedback_number = Column(Integer)  # Address/number on command station
    state = Column(Boolean, default=False)  # occupied or not

    # Relationships
    blocks = relationship("Block", secondary="block_feedbacks", back_populates="feedbacks")

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
            "feedback_number": self.feedback_number,
            "state": self.state,
            "block_ids": self.block_ids,
        }
