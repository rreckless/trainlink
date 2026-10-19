"""
TrainLink - Main FastAPI application
"""
import asyncio
import logging
from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Depends
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from contextlib import asynccontextmanager

from .config import settings
from .database import init_db, get_db
from .ws.connection_manager import ConnectionManager
from .api import z21, locomotives, blocks, signals, routes, layouts, feedbacks

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# WebSocket connection manager
manager = ConnectionManager()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifecycle management"""
    # Startup
    logger.info("Starting TrainLink server...")
    init_db()
    logger.info("Database initialized")
    yield
    # Shutdown
    logger.info("Shutting down TrainLink server...")


# Create FastAPI app
app = FastAPI(
    title="TrainLink",
    description="Digital Model Railroad Control System",
    version="0.1.0",
    lifespan=lifespan,
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.get_allowed_origins(),
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)
# Include API routes
app.include_router(z21.router)
app.include_router(locomotives.router)
app.include_router(blocks.router)
app.include_router(signals.router)
app.include_router(routes.router)
app.include_router(layouts.router)
app.include_router(feedbacks.router)

# Health check endpoint
@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {
        "status": "healthy",
        "version": "0.1.0",
        "database": "connected",
    }


# WebSocket endpoint
@app.websocket("/ws/{client_id}")
async def websocket_endpoint(websocket: WebSocket, client_id: str):
    """WebSocket endpoint for real-time communication"""
    await manager.connect(websocket, client_id)
    try:
        while True:
            data = await websocket.receive_json()
            logger.debug(f"Received message from {client_id}: {data}")

            # Broadcast to all connected clients
            await manager.broadcast({
                "client_id": client_id,
                "message": data,
            })
    except WebSocketDisconnect:
        manager.disconnect(client_id)
        await manager.broadcast({
            "event": "client_disconnected",
            "client_id": client_id,
        })
        logger.info(f"Client {client_id} disconnected")
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
        manager.disconnect(client_id)


# Root endpoint
@app.get("/")
async def root():
    """Root endpoint"""
    return {
        "name": "TrainLink",
        "version": "0.1.0",
        "description": "Digital Model Railroad Control System",
        "endpoints": {
            "health": "/health",
            "websocket": "/ws/{client_id}",
        },
    }


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        "main:app",
        host=settings.host,
        port=settings.port,
        reload=settings.debug,
        log_level="info",
    )
