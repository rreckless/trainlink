# TrainLink Quick Start Guide

## 5-Minute Setup

### Step 1: Start the Database (2 min)

```bash
docker-compose up -d
```

Wait for both services to be healthy:
```bash
docker-compose ps
```

### Step 2: Start the Backend (2 min)

In a new terminal:

```bash
cd backend

# Create and activate virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Copy environment file
cp .env.example .env

# Run the server
python app/main.py
```

You'll see:
```
INFO:     Uvicorn running on http://0.0.0.0:8000
```

### Step 3: Start the Frontend (1 min)

In another new terminal:

```bash
cd frontend

# Install dependencies
npm install

# Start development server
npm run dev
```

You'll see:
```
VITE v5.0.7  ready in 234 ms

➜  Local:   http://localhost:3000
```

### Step 4: Open the Application

Visit `http://localhost:3000` in your browser!

You should see:
- ✅ Dark-themed UI with TrainLink branding
- ✅ Sidebar navigation with 5 main tabs
- ✅ Connection status indicator
- ✅ Placeholder for layout visualization

## Testing the Connection

### Check API Health
```bash
curl http://localhost:8000/health
```

Should return:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "database": "connected"
}
```

### View API Documentation
Visit `http://localhost:8000/docs` for interactive Swagger documentation

## What's Included (Phase 1 Foundation)

✅ **Backend Foundation**
- FastAPI server with WebSocket support
- SQLAlchemy ORM models (Locomotive, Block, Signal, Route, Layout, etc.)
- Pydantic schemas for validation
- PostgreSQL database with Docker
- Redis for caching
- Configuration management

✅ **Frontend Shell**
- React with Vite build system
- Zustand state management
- Tailwind CSS styling
- Five main sections:
  - **Switchboard**: Layout overview
  - **Locomotives**: Train control panel
  - **Routes**: Route automation
  - **Layout Editor**: Design tool
  - **Settings**: System configuration

✅ **Project Structure**
- Organized directory layout
- Git repository initialized
- Docker Compose for dependencies
- Environment configuration
- Comprehensive README

## Next Steps

### Option 1: Continue Building (Recommended)
Jump to **Phase 2: Core Features**
- Implement Locomotive CRUD endpoints
- Build block management UI
- Add signal control logic
- Create layout drawing tools

### Option 2: Explore the Code
- Read `backend/app/models/` to understand data structure
- Check `frontend/src/components/` for UI layout
- Review `backend/app/main.py` for API setup

### Option 3: Test the Architecture
- Create a locomotive via API
- Connect multiple clients to WebSocket
- Experiment with state updates

## Troubleshooting

### Port Already in Use
```bash
# Kill process on port 8000
lsof -ti :8000 | xargs kill -9

# Or change PORT in backend/.env
```

### Database Connection Error
```bash
# Check if containers are running
docker-compose ps

# View logs
docker-compose logs postgresql

# Restart services
docker-compose restart
```

### npm Dependencies Issue
```bash
# Clear cache and reinstall
rm -rf node_modules package-lock.json
npm install
```

### Python Dependency Issue
```bash
# Upgrade pip
pip install --upgrade pip

# Reinstall requirements
pip install -r requirements.txt --force-reinstall
```

## Key Files to Understand

| File | Purpose |
|------|---------|
| `backend/app/main.py` | FastAPI app initialization |
| `backend/app/models/` | Database schemas |
| `backend/app/ws/connection_manager.py` | WebSocket handling |
| `frontend/src/App.jsx` | Main React component |
| `frontend/src/store/index.js` | State management |
| `docker-compose.yml` | Database services |

## Development Commands

```bash
# Backend testing
pytest backend/tests/

# Frontend linting
npm run lint

# Code formatting
black backend/
prettier frontend/src/

# Database migrations
alembic revision --autogenerate -m "description"
alembic upgrade head
```

## Architecture Overview

```
User Browser
    ↓
Vite Dev Server (port 3000)
    ↓
FastAPI Backend (port 8000)
    ↓
PostgreSQL (port 5432) + Redis (port 6379)
```

## WebSocket Connection Flow

1. Frontend connects to `ws://localhost:8000/ws/{clientId}`
2. Backend accepts and registers connection
3. Frontend/backend exchange JSON messages
4. Broadcast messages to all connected clients

Example in browser console:
```javascript
const ws = new WebSocket('ws://localhost:8000/ws/test-client')
ws.onmessage = (e) => console.log(JSON.parse(e.data))
ws.send(JSON.stringify({type: 'test', data: {}}))
```

## Performance Notes

- Database: 5432 (PostgreSQL)
- Cache: 6379 (Redis)
- API: 8000 (FastAPI)
- Frontend: 3000 (Vite)

All services are isolated in Docker for easy scaling.

## Next Phase Checklist

- [ ] Implement locomotive control endpoints
- [ ] Create block management API
- [ ] Add signal automation logic
- [ ] Build layout drawing component
- [ ] Implement feed feedback sensors
- [ ] Add unit tests
- [ ] Write API documentation

---

**Questions?** Check README.md for detailed information or look at the implementation plan in your project docs.

**Ready to build more?** Let Claude know what you want to work on next! 🚂
