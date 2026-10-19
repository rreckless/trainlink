"""
Block model - represents track blocks for collision avoidance
"""
from sqlalchemy import Column, Integer, String, Float, Boolean, DateTime, ForeignKey, Table
from sqlalchemy.orm import relationship
from datetime import datetime
from ..database import Base


# Association table for block-signal relationship
block_signals = Table(
    'block_signals',
    Base.metadata,
    Column('block_id', Integer, ForeignKey('blocks.id')),
    Column('signal_id', Integer, ForeignKey('signals.id'))
)

# Association table for block-feedback relationship
block_feedbacks = Table(
    'block_feedbacks',
    Base.metadata,
    Column('block_id', Integer, ForeignKey('blocks.id')),
    Column('feedback_id', Integer, ForeignKey('feedbacks.id'))
)


class Block(Base):
    """Model for track blocks"""
    __tablename__ = "blocks"

    id = Column(Integer, primary_key=True, index=True)
    layout_id = Column(Integer, ForeignKey("layouts.id"))
    name = Column(String, index=True)

    # Visual position
    x = Column(Float)
    y = Column(Float)

    # Block properties
    max_speed = Column(Integer, default=100)
    length = Column(Float, nullable=True)  # track length

    # State
    state = Column(String, default="free")  # free, occupied, reserved
    occupied = Column(Boolean, default=False)
    occupancy_locomotive_id = Column(Integer, ForeignKey("locomotives.id"), nullable=True)

    # Relationships
    signals = relationship(
        "Signal",
        secondary=block_signals,
        back_populates="blocks"
    )
    feedbacks = relationship(
        "Feedback",
        secondary=block_feedbacks,
        back_populates="blocks"
    )

    # Metadata
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    @property
    def signal_ids(self):
        return [s.id for s in self.signals]

    @property
    def feedback_ids(self):
        return [f.id for f in self.feedbacks]

    def to_dict(self):
        return {
            "id": self.id,
            "layout_id": self.layout_id,
            "name": self.name,
            "x": self.x,
            "y": self.y,
            "max_speed": self.max_speed,
            "state": self.state,
            "occupied": self.occupied,
            "occupancy_locomotive_id": self.occupancy_locomotive_id,
            "signal_ids": self.signal_ids,
            "feedback_ids": self.feedback_ids,
        }
