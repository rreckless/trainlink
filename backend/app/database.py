"""
Database configuration and session management
"""
from sqlalchemy import create_engine, text
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import sessionmaker
from .config import settings

# Create database engine
engine = create_engine(
    settings.database_url,
    echo=settings.debug,
    pool_pre_ping=True,
    pool_recycle=3600,
)

# Create session factory
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

# Base class for models
Base = declarative_base()


def get_db():
    """Dependency for getting database session"""
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


# Columns added to existing tables after they were created. create_all does not add columns to a table that exists
ADDED_COLUMNS = (
    ("locomotives", "function_labels", "JSON"),
    ("locomotives", "image_file", "VARCHAR"),
)


def init_db():
    """Initialize database tables, and add any columns newer than the table"""
    Base.metadata.create_all(bind=engine)
    if engine.dialect.name != "postgresql":
        return
    with engine.begin() as connection:
        for table, column, column_type in ADDED_COLUMNS:
            connection.execute(text(f"ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {column} {column_type}"))
