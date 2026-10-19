"""
TrainLink Configuration
"""
import os
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """Application settings"""

    # Database
    database_url: str = os.getenv(
        "DATABASE_URL",
        "postgresql://itrain:itrain@localhost:5432/itrain"
    )

    # Redis
    redis_url: str = os.getenv(
        "REDIS_URL",
        "redis://localhost:6379/0"
    )

    # Server
    host: str = "0.0.0.0"
    port: int = 8000
    debug: bool = os.getenv("DEBUG", "False").lower() == "true"

    # CORS
    allowed_origins: list = [
        "http://localhost:3000",
        "http://localhost:5173",
        "http://127.0.0.1:3000",
    ]

    # WebSocket
    ws_max_connections: int = 10

    # Hardware
    command_station_port: str = "/dev/ttyUSB0"  # Serial port for command station

    class Config:
        env_file = ".env"


settings = Settings()
