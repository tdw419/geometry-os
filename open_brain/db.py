"""Database Connection for Visual Open Brain."""
import asyncpg
from typing import Dict, Any, List, Optional
import json


class Database:
    """PostgreSQL database connection with pgvector support."""

    def __init__(self, connection_string: str):
        self.connection_string = connection_string
        self._pool: Optional[asyncpg.Pool] = None

    async def connect(self):
        """Create connection pool."""
        self._pool = await asyncpg.create_pool(self.connection_string)
        await self._ensure_schema()

    async def disconnect(self):
        """Close connection pool."""
        if self._pool:
            await self._pool.close()
            self._pool = None

    async def _ensure_schema(self):
        """Ensure database schema exists."""
        async with self._pool.acquire() as conn:
            # Enable pgvector
            await conn.execute("CREATE EXTENSION IF NOT EXISTS vector")

            # Create memory entries table
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS memory_entries (
                    id SERIAL PRIMARY KEY,
                    type VARCHAR(50) NOT NULL,
                    content JSONB NOT NULL,
                    embedding VECTOR(384),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    priority DOUBLE PRECISION DEFAULT 0.5,
                    tags TEXT[] DEFAULT '{}',
                    metadata JSONB DEFAULT '{}'
                )
            """)

            # Create visual metadata table
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS visual_metadata (
                    id SERIAL PRIMARY KEY,
                    memory_id INTEGER REFERENCES memory_entries(id),
                    rgb_encoding JSONB,
                    symmetry_type VARCHAR(20),
                    visual_density DOUBLE PRECISION,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)

    async def store_memory(
        self,
        entry: Dict[str, Any],
        embedding: Optional[List[float]] = None
    ) -> int:
        """Store a memory entry with optional embedding."""
        async with self._pool.acquire() as conn:
            result = await conn.fetchrow(
                """
                INSERT INTO memory_entries
                (type, content, priority, embedding, tags, metadata)
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id
                """,
                entry.get("type", "note"),
                json.dumps(entry.get("content", "")),
                entry.get("priority", 0.5),
                embedding,
                entry.get("tags", []),
                json.dumps(entry.get("metadata", {}))
            )
            return result["id"]

    async def get_visual_memories(
        self,
        limit: int = 256,
        offset: int = 0
    ) -> List[Dict[str, Any]]:
        """Retrieve memories for visual encoding."""
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(
                """
                SELECT id, type, content, priority, tags, metadata
                FROM memory_entries
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                """,
                limit,
                offset
            )
            return [dict(row) for row in rows]

    async def get_tsv_export(self, limit: int = 100) -> str:
        """Export memories as TSV for token-efficient AI consumption."""
        memories = await self.get_visual_memories(limit=limit)

        if not memories:
            return ""

        lines = ["id\ttype\tcontent\tpriority"]

        for m in memories:
            content = str(m.get("content", "")).replace("\t", " ").replace("\n", " ")
            tags = ",".join(m.get("tags", []))
            lines.append(f"{m['id']}\t{m['type']}\t{content}\t{m['priority']}")

        return "\n".join(lines)

    async def get_memory_by_id(self, memory_id: int) -> Optional[Dict[str, Any]]:
        """Get a single memory by ID."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT * FROM memory_entries WHERE id = $1",
                memory_id
            )
            return dict(row) if row else None

    async def search_by_embedding(
        self,
        embedding: List[float],
        limit: int = 10
    ) -> List[Dict[str, Any]]:
        """Search memories by embedding similarity using pgvector.

        Args:
            embedding: Query embedding vector (384-dimensional)
            limit: Maximum number of results

        Returns:
            List of memory entries ordered by cosine similarity
        """
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(
                """
                SELECT id, type, content, priority, tags, metadata,
                       1 - (embedding <=> $1) as similarity
                FROM memory_entries
                WHERE embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $2
                """,
                embedding,
                limit
            )
            return [dict(row) for row in rows]
