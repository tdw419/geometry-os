import pytest
from unittest.mock import AsyncMock, MagicMock, patch
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))


class TestVisualMCPServer:
    @pytest.mark.asyncio
    async def test_list_tools(self):
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")
        tools = await server.list_tools()
        names = [t["name"] for t in tools]
        assert "query_memory" in names
        assert "query_visual_memory" in names
        assert "store_memory" in names

    @pytest.mark.asyncio
    async def test_query_memory_returns_tsv(self):
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        # Mock database
        server.db = MagicMock()
        server.db.get_tsv_export = AsyncMock(return_value="id\ttype\tcontent\n1\tnote\tTest")
        server._connected = True

        result = await server.call_tool("query_memory", {"limit": 10})
        assert result["format"] == "tsv"
        assert "id\ttype" in result["content"]

    @pytest.mark.asyncio
    async def test_query_visual_memory_returns_atlas(self):
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        # Mock database
        server.db = MagicMock()
        server.db.get_visual_memories = AsyncMock(return_value=[
            {"id": 1, "type": "note", "content": "Test", "priority": 0.5}
        ])
        server._connected = True

        result = await server.call_tool("query_visual_memory", {"limit": 10})
        assert result["format"] == "glyph_atlas"
        assert "image_base64" in result

    @pytest.mark.asyncio
    async def test_store_memory(self):
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        # Mock database
        server.db = MagicMock()
        server.db.store_memory = AsyncMock(return_value=42)
        server._connected = True

        result = await server.call_tool("store_memory", {
            "type": "note",
            "content": "Test memory",
            "priority": 0.8
        })
        assert result["id"] == 42
        assert result["status"] == "stored"
