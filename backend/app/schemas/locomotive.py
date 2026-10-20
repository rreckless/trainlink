"""
Locomotive schemas
"""
from datetime import datetime
from typing import List, Optional

from pydantic import BaseModel, Field, field_validator

FUNCTION_BUTTON_COUNT = 20
LABEL_MAX_LENGTH = 24
DEFAULT_FUNCTION_LABELS = ["Light"] + [f"F{index}" for index in range(1, FUNCTION_BUTTON_COUNT)]


class FunctionLabel(BaseModel):
    """What one function button does for a locomotive"""
    label: str = Field(min_length=1, max_length=LABEL_MAX_LENGTH)
    # Momentary functions (a horn, for example) are on only while the button is held
    momentary: bool = False

    @field_validator("label")
    @classmethod
    def strip_label(cls, value: str) -> str:
        stripped = value.strip()
        if not stripped:
            raise ValueError("A function label cannot be blank")
        return stripped


def default_function_labels() -> List[FunctionLabel]:
    return [FunctionLabel(label=label) for label in DEFAULT_FUNCTION_LABELS]


class LocomotiveCreate(BaseModel):
    """Schema for creating a locomotive"""
    name: str
    dcc_address: int
    locomotive_type: str
    # Optional: the Z21 is the only command station, so most locomotives leave this empty
    command_station_id: Optional[int] = None
    length: Optional[float] = None


class LocomotiveUpdate(BaseModel):
    """Schema for updating a locomotive. Speed and direction are set through the Z21 routes"""
    name: Optional[str] = None
    dcc_address: Optional[int] = None
    locomotive_type: Optional[str] = None
    length: Optional[float] = None
    # Accepted only so the route can refuse them. Set them through /api/z21/loco/speed
    speed: Optional[int] = None
    direction: Optional[str] = None
    functions: Optional[int] = None
    function_labels: Optional[List[FunctionLabel]] = None

    @field_validator("function_labels")
    @classmethod
    def exactly_twenty(cls, value):
        if value is not None and len(value) != FUNCTION_BUTTON_COUNT:
            raise ValueError(f"Give a label for each of the {FUNCTION_BUTTON_COUNT} function buttons")
        return value


class LocomotiveResponse(BaseModel):
    """Schema for locomotive response"""
    id: int
    name: str
    dcc_address: int
    locomotive_type: str
    length: Optional[float] = None
    speed: int
    direction: str
    functions: int
    function_labels: List[FunctionLabel]
    image_url: Optional[str] = None
    travelled_time: float
    travelled_distance: float
    created_at: datetime

    @field_validator("function_labels", mode="before")
    @classmethod
    def defaults_when_unset(cls, value):
        # Locomotives saved before labels existed have none. Show the defaults
        return value if value else default_function_labels()

    class Config:
        from_attributes = True
