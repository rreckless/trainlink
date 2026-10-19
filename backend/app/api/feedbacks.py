"""
Feedback REST endpoints

Feedbacks are the occupancy sensors placed on a layout. Their hardware
link (R-BUS module and input) is not modelled yet. `state` can be set
manually until hardware feedback is linked.
"""
from typing import List, Optional

from fastapi import APIRouter, Depends, Response, status
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import Feedback, Layout
from ..schemas import FeedbackCreate, FeedbackResponse, FeedbackUpdate
from .common import get_or_404, reject_nulls

router = APIRouter(prefix="/api/feedbacks", tags=["feedbacks"])


@router.get("", response_model=List[FeedbackResponse])
def list_feedbacks(layout_id: Optional[int] = None, db: Session = Depends(get_db)):
    query = db.query(Feedback)
    if layout_id is not None:
        query = query.filter(Feedback.layout_id == layout_id)
    return query.order_by(Feedback.id).all()


@router.post("", response_model=FeedbackResponse, status_code=status.HTTP_201_CREATED)
def create_feedback(body: FeedbackCreate, db: Session = Depends(get_db)):
    get_or_404(db, Layout, body.layout_id, "Layout")

    feedback = Feedback(
        layout_id=body.layout_id,
        name=body.name,
        x=body.x,
        y=body.y,
        feedback_number=body.feedback_number,
    )
    db.add(feedback)
    db.commit()
    db.refresh(feedback)
    return feedback


@router.get("/{feedback_id}", response_model=FeedbackResponse)
def get_feedback(feedback_id: int, db: Session = Depends(get_db)):
    return get_or_404(db, Feedback, feedback_id, "Feedback")


@router.patch("/{feedback_id}", response_model=FeedbackResponse)
def update_feedback(feedback_id: int, body: FeedbackUpdate, db: Session = Depends(get_db)):
    changes = body.model_dump(exclude_unset=True)
    reject_nulls(changes)

    feedback = get_or_404(db, Feedback, feedback_id, "Feedback")
    for field, value in changes.items():
        setattr(feedback, field, value)
    db.commit()
    db.refresh(feedback)
    return feedback


@router.delete("/{feedback_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_feedback(feedback_id: int, db: Session = Depends(get_db)):
    feedback = get_or_404(db, Feedback, feedback_id, "Feedback")
    db.delete(feedback)
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
