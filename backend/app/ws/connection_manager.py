"""
WebSocket connection manager for handling multiple clients
"""
import json
import logging
from fastapi import WebSocket
from typing import List, Dict

logger = logging.getLogger(__name__)


class ConnectionManager:
    """Manages WebSocket connections"""

    def __init__(self):
        self.active_connections: Dict[str, WebSocket] = {}

    async def connect(self, websocket: WebSocket, client_id: str):
        """Accept and register a new WebSocket connection"""
        await websocket.accept()
        self.active_connections[client_id] = websocket
        logger.info(f"Client {client_id} connected. Total: {len(self.active_connections)}")

    def disconnect(self, client_id: str):
        """Remove a client connection"""
        if client_id in self.active_connections:
            del self.active_connections[client_id]
            logger.info(f"Client {client_id} disconnected. Total: {len(self.active_connections)}")

    async def broadcast(self, message: dict):
        """Send message to all connected clients"""
        disconnected = []

        for client_id, connection in self.active_connections.items():
            try:
                await connection.send_json(message)
            except Exception as e:
                logger.error(f"Error sending to {client_id}: {e}")
                disconnected.append(client_id)

        # Clean up disconnected clients
        for client_id in disconnected:
            self.disconnect(client_id)

    async def send_personal_message(self, message: dict, client_id: str):
        """Send message to a specific client"""
        if client_id in self.active_connections:
            try:
                await self.active_connections[client_id].send_json(message)
            except Exception as e:
                logger.error(f"Error sending to {client_id}: {e}")
                self.disconnect(client_id)

    def get_active_clients(self) -> List[str]:
        """Get list of active client IDs"""
        return list(self.active_connections.keys())

    def get_connection_count(self) -> int:
        """Get number of active connections"""
        return len(self.active_connections)
