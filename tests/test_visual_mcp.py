import pytest
from unittest.mock import AsyncMock, MagicMock, patch
import sys
from pathlib import Path
import numpy as np

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
        assert "search_memory" in names

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

    @pytest.mark.asyncio
    async def test_store_with_embedding(self):
        """Test that store_memory generates embeddings automatically."""
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        # Mock database
        server.db = MagicMock()
        server.db.store_memory = AsyncMock(return_value=42)
        server._connected = True

        result = await server.call_tool("store_memory", {
            "type": "note",
            "content": "Test memory for embedding",
            "priority": 0.8
        })

        # Verify embedding was generated
        assert result["id"] == 42
        assert result["embedding_generated"] is True

        # Verify store_memory was called with embedding
        call_args = server.db.store_memory.call_args
        assert call_args is not None
        # Check that embedding was passed
        kwargs = call_args.kwargs
        assert "embedding" in kwargs
        embedding = kwargs["embedding"]
        assert isinstance(embedding, list)
        assert len(embedding) == 384  # all-MiniLM-L6-v2 dimension

    @pytest.mark.asyncio
    async def test_semantic_search(self):
        """Test that search_memory performs semantic search."""
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        # Mock database
        server.db = MagicMock()
        server.db.search_by_embedding = AsyncMock(return_value=[
            {
                "id": 1,
                "type": "note",
                "content": "The cat sat on the mat",
                "priority": 0.5,
                "similarity": 0.85,
                "tags": []
            },
            {
                "id": 2,
                "type": "note",
                "content": "A feline on a rug",
                "priority": 0.6,
                "similarity": 0.72,
                "tags": []
            }
        ])
        server._connected = True

        result = await server.call_tool("search_memory", {
            "query": "cat on mat",
            "limit": 10
        })

        assert result["format"] == "search_results"
        assert result["query"] == "cat on mat"
        assert len(result["results"]) == 2
        assert result["count"] == 2

        # Verify search_by_embedding was called with embedding
        call_args = server.db.search_by_embedding.call_args
        assert call_args is not None
        args, kwargs = call_args.args, call_args.kwargs
        # First positional arg should be embedding list
        embedding = args[0] if args else kwargs.get("embedding")
        assert isinstance(embedding, list)
        assert len(embedding) == 384

    @pytest.mark.asyncio
    async def test_semantic_search_empty_query(self):
        """Test search_memory with empty query returns empty results."""
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        server._connected = True

        result = await server.call_tool("search_memory", {
            "query": "",
            "limit": 10
        })

        assert result["format"] == "search_results"
        assert result["results"] == []
        assert result["message"] == "Empty query"

    @pytest.mark.asyncio
    async def test_embedding_generator_integrated(self):
        """Test that EmbeddingGenerator is properly initialized."""
        from open_brain.visual_mcp import VisualMCPServer
        from open_brain.embeddings import EmbeddingGenerator

        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        assert hasattr(server, "embedding_gen")
        assert isinstance(server.embedding_gen, EmbeddingGenerator)

    @pytest.mark.asyncio
    async def test_embedding_generator_custom_backend(self):
        """Test that custom embedding backend can be configured."""
        from open_brain.visual_mcp import VisualMCPServer

        server = VisualMCPServer(
            "postgresql://test:test@localhost/openbrain_test",
            embedding_backend="lm_studio",
            lm_studio_url="http://localhost:1234"
        )

        assert server.embedding_gen.backend == "lm_studio"
        assert server.embedding_gen.lm_studio_url == "http://localhost:1234"
