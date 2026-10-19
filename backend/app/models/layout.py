"""
Layout model - represents a track layout
"""
from sqlalchemy import Column, Integer, String, Float, DateTime, ForeignKey, JSON
from datetime import datetime
from database import Base


class Layout(Base):
    """Model for track layouts"""
    __tablename__ = "layouts"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, unique=True, index=True)
    description = Column(String, nullable=True)

    # Dimensions
    width = Column(Float)
    height = Column(Float)

    # Scale and properties
    scale = Column(String)  # HO, N, Z, G, etc.
    track_gauge = Column(String, nullable=True)  # Standard gauge distance

    # Layout elements (stored as JSON for flexibility)
    elements = Column(JSON, default={})  # All visual elements

    # Metadata
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    def to_dict(self):
        return {
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "width": self.width,
            "height": self.height,
            "scale": self.scale,
            "track_gauge": self.track_gauge,
            "elements": self.elements,
        }
