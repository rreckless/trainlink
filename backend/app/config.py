"""
TrainLink Configuration
"""
from typing import List
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """Application settings"""

    class Config:
        env_file = ".env"
        case_sensitive = False

    # Database
    database_url: str = "postgresql://trainlink:trainlink@localhost:5432/trainlink"

    # Redis
    redis_url: str = "redis://localhost:6379/0"

    # Server
    host: str = "0.0.0.0"
    port: int = 8000
    debug: bool = False

    # WebSocket
    ws_max_connections: int = 10

    # Hardware
    command_station_port: str = "/dev/ttyUSB0"

    def get_allowed_origins(self) -> List[str]:
        """Return CORS allowed origins"""
        return [
            "http://localhost:3000",
            "http://localhost:5173",
            "http://127.0.0.1:3000",
        ]


settings = Settings()