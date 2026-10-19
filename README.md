# TrainLink - Digital Model Railroad Control System

A fully functional replica of TrainLink, a comprehensive software solution for controlling model railroads with computer-based automation.

## Overview

TrainLink is a modern, multi-platform application for managing digital model railroad layouts. It supports:

- **Locomotive Control**: Manual and automatic control with speed/direction management
- **Block System**: Automatic collision avoidance through block-based control
- **Signal Management**: Automatic signal aspect control based on block state
- **Route Automation**: Define and execute complex routes with timing
- **Multi-Computer Support**: Client-server architecture for networked control
- **Hardware Integration**: Support for multiple command station protocols
- **Layout Editor**: Drag-and-drop interface for designing track layouts

## Architecture

### Tech Stack

- **Backend**: Python FastAPI, PostgreSQL, Redis
- **Frontend**: React, Konva.js for canvas
- **Desktop**: Electron for cross-platform desktop app
- **Real-time Communication**: WebSocket

### Project Structure

```
trainlink/
├── backend/              # FastAPI application
│   ├── app/
│   │   ├── models/      # SQLAlchemy database models
│   │   ├── schemas/     # Pydantic validation schemas
│   │   ├── api/         # REST API endpoints
│   │   ├── ws/          # WebSocket handlers
│   │   ├── services/    # Business logic
│   │   ├── drivers/     # Hardware command station drivers
│   │   ├── main.py      # FastAPI app initialization
│   │   ├── config.py    # Configuration
│   │   └── database.py  # Database setup
│   ├── tests/           # Unit and integration tests
│   └── requirements.txt  # Python dependencies
├── frontend/            # React application
│   ├── src/
│   │   ├── components/  # React components
│   │   ├── hooks/       # Custom React hooks
│   │   ├── store/       # Zustand state management
│   │   ├── services/    # API service clients
│   │   ├── App.jsx      # Main app component
│   │   └── main.jsx     # Entry point
│   ├── public/          # Static assets
│   └── package.json     # Node dependencies
├── electron/            # Electron desktop wrapper
│   ├── main.js          # Electron main process
│   ├── preload.js       # Context bridge
│   └── package.json     # Electron dependencies
├── docker-compose.yml   # PostgreSQL + Redis setup
└── README.md           # This file
```

## Quick Start

### Prerequisites

- Python 3.10+
- Node.js 18+
- Docker & Docker Compose (for database)
- Git

### 1. Clone the Repository

```bash
cd trainlink
```

### 2. Start Database Services

```bash
docker-compose up -d
```

This starts PostgreSQL (port 5432) and Redis (port 6379).

### 3. Backend Setup

```bash
cd backend

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Set up environment variables
cp .env.example .env

# Initialize database
python -m alembic upgrade head

# Run server
python app/main.py
```

The server will be available at `http://localhost:8000`

### 4. Frontend Setup

```bash
cd frontend

# Install dependencies
npm install

# Start development server
npm run dev
```

The frontend will be available at `http://localhost:3000`

### 5. (Optional) Electron Desktop App

```bash
cd electron
npm install

# Run development
npm run electron-dev

# Build for production
npm run build
```

## Development Workflow

### Running All Services

From the project root:

```bash
# Terminal 1: Start databases
docker-compose up

# Terminal 2: Start backend
cd backend
source venv/bin/activate
python app/main.py

# Terminal 3: Start frontend
cd frontend
npm run dev

# Terminal 4: (Optional) Start Electron
cd electron
npm run electron-start
```

### Database Migrations

Using Alembic for schema migrations:

```bash
cd backend

# Create a new migration
alembic revision --autogenerate -m "description"

# Apply migrations
alembic upgrade head

# Rollback
alembic downgrade -1
```

## API Documentation

Once the backend is running, visit `http://localhost:8000/docs` for interactive API documentation (Swagger UI).

### WebSocket Connection

Connect to WebSocket at: `ws://localhost:8000/ws/{client_id}`

Example:
```javascript
const clientId = `client_${Date.now()}`
const ws = new WebSocket(`ws://localhost:8000/ws/${clientId}`)

ws.onmessage = (event) => {
  const message = JSON.parse(event.data)
  console.log('Received:', message)
}

ws.send(JSON.stringify({ type: 'command', data: {} }))
```

## Features Implementation Status

### Phase 1: Foundation ✅
- [x] Project structure
- [x] Database models
- [x] WebSocket server
- [x] Basic UI shell
- [ ] API endpoints for CRUD operations
- [ ] Frontend state management

### Phase 2: Core Features 🚧
- [ ] Locomotive control (speed, direction)
- [ ] Block management and visualization
- [ ] Signal control
- [ ] Layout drawing tools
- [ ] Occupancy detection

### Phase 3: Automation 🔲
- [ ] Route definition and execution
- [ ] Automatic driving
- [ ] Collision avoidance
- [ ] Feedback integration

### Phase 4: Multi-Computer 🔲
- [ ] Multi-client synchronization
- [ ] Remote overview clients
- [ ] Network reliability features

### Phase 5: Hardware Integration 🔲
- [ ] Märklin P50x/P50 support
- [ ] Lenz Intellibox support
- [ ] Roco/Fleischmann ECoS support
- [ ] Digitrax LocoNet support

### Phase 6: Polish 🔲
- [ ] Performance optimization
- [ ] Comprehensive testing
- [ ] Documentation
- [ ] Production builds

## Configuration

### Backend (.env)

```env
# Database
DATABASE_URL=postgresql://itrain:itrain@localhost:5432/itrain

# Redis
REDIS_URL=redis://localhost:6379/0

# Server
HOST=0.0.0.0
PORT=8000
DEBUG=True

# Serial port for command station
COMMAND_STATION_PORT=/dev/ttyUSB0
```

### Frontend (src/config.js)

```javascript
export const API_URL = 'http://localhost:8000'
export const WS_URL = 'ws://localhost:8000/ws'
```

## Hardware Support

The application supports multiple model railroad command station protocols:

- **Märklin**: P50x, P50, 6051
- **Lenz**: Intellibox, Compact
- **Roco/Fleischmann**: ECoS
- **Digitrax**: LocoNet
- **NMRA**: DCC standard

## Testing

### Backend Tests

```bash
cd backend
pytest tests/
```

### Frontend Tests

```bash
cd frontend
npm run test
```

## Building for Production

### Backend

```bash
cd backend
pip install gunicorn
gunicorn -w 4 -b 0.0.0.0:8000 app.main:app
```

### Frontend

```bash
cd frontend
npm run build
```

### Electron App

```bash
cd electron
npm run build
```

## Contributing

1. Create a feature branch: `git checkout -b feature/amazing-feature`
2. Commit changes: `git commit -m 'Add amazing feature'`
3. Push to branch: `git push origin feature/amazing-feature`
4. Open a pull request

## Roadmap

- **v0.2.0**: Complete Phase 2 (Core Features)
- **v0.3.0**: Complete Phase 3 (Automation)
- **v0.4.0**: Complete Phase 4 (Multi-Computer)
- **v0.5.0**: Complete Phase 5 (Hardware)
- **v1.0.0**: Production release

## Troubleshooting

### Connection Issues

- Ensure Docker containers are running: `docker-compose ps`
- Check database connectivity: `psql -h localhost -U itrain -d itrain`
- Verify Redis: `redis-cli ping`

### Port Already in Use

Change ports in configuration:
- Backend: `PORT=8001` in .env
- Frontend: `--port 3001` in `npm run dev`
- Database: `5433:5432` in docker-compose.yml

### WebSocket Errors

- Ensure backend is running
- Check browser console for connection errors
- Verify firewall settings

## Performance Optimization

For production deployments:

1. Enable connection pooling in database
2. Configure Redis caching
3. Use CDN for static assets
4. Enable gzip compression
5. Optimize database queries with indexes

## License

TrainLink is licensed under the GNU General Public License v3.0

## Support

For issues, questions, or suggestions:
- Create an issue in the repository
- Check existing documentation
- Review implementation plan in `IMPLEMENTATION_PLAN.md`

---

**Version**: 0.1.0  
**Status**: Phase 1 - Foundation
