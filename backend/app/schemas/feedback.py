"""
Feedback schemas
"""
from pydantic import BaseModel
from typing import Optional, List
from datetime import datetime


class FeedbackCreate(BaseModel):
    """Schema for creating a feedback sensor"""
    layout_id: int
    name: str
    x: float
    y: float
    feedback_number: int


class FeedbackUpdate(BaseModel):
    """Schema for updating a feedback sensor"""
    name: Optional[str] = None
    state: Optional[bool] = None


class FeedbackResponse(BaseModel):
    """Schema for feedback response"""
    id: int
    layout_id: int
    name: str
    x: float
    y: float
    feedback_number: int
    state: bool
    block_ids: List[int]
    created_at: datetime

    class Config:
        from_attributes = True
