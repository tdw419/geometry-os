import pytest
from unittest.mock import AsyncMock, patch, MagicMock
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))


class TestDatabase:
    @pytest.mark.asyncio
    async def test_connect(self):
        from open_brain.db import Database
        db = Database("postgresql://test:test@localhost/openbrain_test")
        with patch("asyncpg.create_pool", new_callable=AsyncMock) as mock:
            mock_pool = MagicMock()
            mock_pool.close = AsyncMock()
            mock.return_value = mock_pool
            await db.connect()
            assert db._pool is not None
            await db.disconnect()

    @pytest.mark.asyncio
    async def test_store_memory(self):
        from open_brain.db import Database
        db = Database("postgresql://test:test@localhost/openbrain_test")

        mock_pool = MagicMock()
        mock_conn = MagicMock()
        mock_conn.execute = AsyncMock(return_value="INSERT 1")
        mock_conn.fetchrow = AsyncMock(return_value={"id": 42})

        mock_pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
        mock_pool.acquire.return_value.__aexit__ = AsyncMock()

        db._pool = mock_pool

        entry = {"type": "note", "content": "Test memory", "priority": 0.8}
        result = await db.store_memory(entry)
        assert result is not None

    @pytest.mark.asyncio
    async def test_get_visual_memories(self):
        from open_brain.db import Database
        db = Database("postgresql://test:test@localhost/openbrain_test")

        mock_pool = MagicMock()
        mock_conn = MagicMock()
        mock_conn.fetch = AsyncMock(return_value=[
            {"id": 1, "type": "note", "content": "Test", "priority": 0.5}
        ])

        mock_pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
        mock_pool.acquire.return_value.__aexit__ = AsyncMock()

        db._pool = mock_pool

        memories = await db.get_visual_memories(limit=10)
        assert len(memories) == 1
        assert memories[0]["type"] == "note"

    @pytest.mark.asyncio
    async def test_get_tsv_export(self):
        from open_brain.db import Database
        db = Database("postgresql://test:test@localhost/openbrain_test")

        mock_pool = MagicMock()
        mock_conn = MagicMock()
        mock_conn.fetch = AsyncMock(return_value=[
            {"id": 1, "type": "note", "content": "Test content", "priority": 0.5}
        ])

        mock_pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
        mock_pool.acquire.return_value.__aexit__ = AsyncMock()

        db._pool = mock_pool

        tsv = await db.get_tsv_export(limit=10)
        assert "id" in tsv
        assert "type" in tsv
        assert "note" in tsv
