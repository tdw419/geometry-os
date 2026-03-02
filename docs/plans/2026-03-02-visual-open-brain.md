# Visual Open Brain Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a thin integration layer connecting Geometry OS visual encoding with Open Brain persistent memory, enabling AI agents to "see" memories as visual textures.

**Architecture:** Hybrid approach - keep Geometry OS for visual encoding/GPU, use existing Open Brain patterns (PostgreSQL + pgvector + MCP) for storage, and build a ~500-line integration layer with three components: memory-to-glyph encoder, visual MCP tool, and atlas generator.

**Tech Stack:** Python 3.10+, PostgreSQL with pgvector, MCP (Model Context Protocol), NumPy, PIL, asyncpg

---

## What We Build

```
open_brain/
├── __init__.py           # Package init
├── memory_glyph.py       # Memory entry → Glyph encoding (~150 lines)
├── visual_mcp.py         # MCP server with visual tools (~150 lines)
├── db.py                 # PostgreSQL connection (~100 lines)
└── atlas_generator.py    # Batch memories → Atlas tiles (~100 lines)

tests/
├── test_memory_glyph.py
├── test_db.py
├── test_visual_mcp.py
└── test_integration.py
```

---

## Task 1: Create Package Structure

**Files:**
- Create: `open_brain/__init__.py`
- Create: `open_brain/memory_glyph.py` (empty)
- Create: `open_brain/db.py` (empty)
- Create: `tests/test_open_brain.py`

**Step 1: Create directories**
```bash
mkdir -p open_brain tests
```

**Step 2: Create `open_brain/__init__.py`**
```python
"""Visual Open Brain - Geometry OS + Persistent Memory Integration."""
__version__ = "0.1.0"
from .memory_glyph import MemoryGlyphEncoder
from .db import Database
__all__ = ["MemoryGlyphEncoder", "Database"]
```

**Step 3: Create empty module files**
```bash
touch open_brain/memory_glyph.py open_brain/db.py
```

**Step 4: Create `tests/test_open_brain.py`**
```python
def test_package_imports():
    from open_brain import MemoryGlyphEncoder, Database
    assert MemoryGlyphEncoder is not None
    assert Database is not None
```

**Step 5: Run test**
```bash
python -m pytest tests/test_open_brain.py -v
```
Expected: PASS

**Step 6: Commit**
```bash
git add open_brain/ tests/test_open_brain.py
git commit -m "feat(open-brain): create package structure"
```

---

## Task 2: Implement Memory Glyph Encoder

**Files:**
- Modify: `open_brain/memory_glyph.py`
- Create: `tests/test_memory_glyph.py`

**Step 1: Write failing test `tests/test_memory_glyph.py`**
```python
import pytest
import numpy as np

class TestMemoryGlyphEncoder:
    def test_encode_note_type(self):
        from open_brain.memory_glyph import MemoryGlyphEncoder
        encoder = MemoryGlyphEncoder()
        entry = {"type": "note", "content": "Test", "priority": 0.8}
        glyph = encoder.encode(entry)
        assert glyph["char"] == "N"
        assert glyph["g"] == 0x10
        assert glyph["b"] == int(0.8 * 255)

    def test_encode_task_type(self):
        from open_brain.memory_glyph import MemoryGlyphEncoder
        encoder = MemoryGlyphEncoder()
        entry = {"type": "task", "content": "Build feature", "priority": 1.0}
        glyph = encoder.encode(entry)
        assert glyph["char"] == "T"
        assert glyph["g"] == 0x20

    def test_encode_to_pixels(self):
        from open_brain.memory_glyph import MemoryGlyphEncoder
        encoder = MemoryGlyphEncoder()
        entry = {"type": "note", "content": "Test", "priority": 0.5}
        pixels = encoder.encode_to_pixels(entry)
        assert pixels.shape == (16, 16, 4)
        assert pixels.dtype == np.uint8
```

**Step 2: Run test (should fail)**
```bash
python -m pytest tests/test_memory_glyph.py -v
```

**Step 3: Implement `open_brain/memory_glyph.py`**
```python
"""Memory to Glyph Encoding for Visual Open Brain."""
import numpy as np
from typing import Dict, Any
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))
from core.hilbert_util import HilbertCurve

CATEGORY_OPCODES = {
    "note": 0x10, "task": 0x20, "decision": 0x30, "idea": 0x40,
    "reference": 0x50, "code": 0x60, "meeting": 0x70, "project": 0x80,
}

TYPE_CHARS = {
    "note": "N", "task": "T", "decision": "D", "idea": "I",
    "reference": "R", "code": "C", "meeting": "M", "project": "P",
}

class MemoryGlyphEncoder:
    """Encodes memory entries to Geometry OS glyph format."""
    GLYPH_SIZE = 16

    def __init__(self):
        self.category_opcodes = CATEGORY_OPCODES.copy()
        self.type_chars = TYPE_CHARS.copy()

    def encode(self, entry: Dict[str, Any]) -> Dict[str, Any]:
        entry_type = entry.get("type", "note").lower()
        priority = entry.get("priority", 0.5)
        char = self.type_chars.get(entry_type, "N")
        g = self.category_opcodes.get(entry_type, 0x10)
        b = int(min(max(priority, 0.0), 1.0) * 255)
        return {"char": char, "r": 255, "g": g, "b": b, "symmetry": 0}

    def encode_to_pixels(self, entry: Dict[str, Any]) -> np.ndarray:
        glyph_meta = self.encode(entry)
        pixels = np.zeros((self.GLYPH_SIZE, self.GLYPH_SIZE, 4), dtype=np.uint8)
        pixels[:, :, 0] = glyph_meta["r"]
        pixels[:, :, 1] = glyph_meta["g"]
        pixels[:, :, 2] = glyph_meta["b"]
        pixels[:, :, 3] = 255
        return pixels

    def entries_to_atlas(self, entries: list, atlas_size: int = 512) -> np.ndarray:
        glyphs_per_row = atlas_size // self.GLYPH_SIZE
        max_glyphs = glyphs_per_row * glyphs_per_row
        atlas = np.zeros((atlas_size, atlas_size, 4), dtype=np.uint8)
        for i, entry in enumerate(entries[:max_glyphs]):
            col, row = i % glyphs_per_row, i // glyphs_per_row
            x, y = col * self.GLYPH_SIZE, row * self.GLYPH_SIZE
            pixels = self.encode_to_pixels(entry)
            atlas[y:y+self.GLYPH_SIZE, x:x+self.GLYPH_SIZE] = pixels
        return atlas
```

**Step 4: Run test (should pass)**
```bash
python -m pytest tests/test_memory_glyph.py -v
```

**Step 5: Commit**
```bash
git add open_brain/memory_glyph.py tests/test_memory_glyph.py
git commit -m "feat(open-brain): implement memory-to-glyph encoder"
```

---

## Task 3: Implement Database Connection

**Files:**
- Modify: `open_brain/db.py`
- Create: `tests/test_db.py`

**Step 1: Write failing test `tests/test_db.py`**
```python
import pytest
from unittest.mock import AsyncMock, patch, MagicMock

class TestDatabase:
    @pytest.mark.asyncio
    async def test_connect(self):
        from open_brain.db import Database
        db = Database("postgresql://test:test@localhost/openbrain_test")
        with patch("asyncpg.create_pool", new_callable=AsyncMock) as mock:
            mock.return_value = MagicMock()
            await db.connect()
            assert db._pool is not None

    @pytest.mark.asyncio
    async def test_store_memory(self):
        from open_brain.db import Database
        db = Database("postgresql://test:test@localhost/openbrain_test")
        db._pool = MagicMock()
        db._pool.acquire.return_value.__aenter__ = AsyncMock(return_value=MagicMock())
        db._pool.acquire.return_value.__aexit__ = AsyncMock()
        # ... test store
```

**Step 2: Implement `open_brain/db.py`**
```python
"""Database Connection for Visual Open Brain."""
import asyncpg
from typing import Dict, Any, List, Optional
import json

class Database:
    def __init__(self, connection_string: str):
        self.connection_string = connection_string
        self._pool: Optional[asyncpg.Pool] = None

    async def connect(self):
        self._pool = await asyncpg.create_pool(self.connection_string)
        await self._ensure_schema()

    async def disconnect(self):
        if self._pool:
            await self._pool.close()

    async def _ensure_schema(self):
        async with self._pool.acquire() as conn:
            await conn.execute("CREATE EXTENSION IF NOT EXISTS vector")
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS memory_entries (
                    id SERIAL PRIMARY KEY,
                    type VARCHAR(50) NOT NULL,
                    content JSONB NOT NULL,
                    embedding VECTOR(384),
                    created_at TIMESTAMP DEFAULT NOW(),
                    priority DOUBLE PRECISION DEFAULT 0.5,
                    tags TEXT[] DEFAULT '{}'
                )
            """)

    async def store_memory(self, entry: Dict, embedding: List[float] = None) -> int:
        async with self._pool.acquire() as conn:
            result = await conn.fetchrow(
                "INSERT INTO memory_entries (type, content, priority, embedding) VALUES ($1,$2,$3,$4) RETURNING id",
                entry.get("type", "note"), json.dumps(entry.get("content", "")),
                entry.get("priority", 0.5), embedding
            )
            return result["id"]

    async def get_visual_memories(self, limit: int = 256) -> List[Dict]:
        async with self._pool.acquire() as conn:
            rows = await conn.fetch("SELECT id, type, content, priority FROM memory_entries ORDER BY created_at DESC LIMIT $1", limit)
            return [dict(r) for r in rows]

    async def get_tsv_export(self, limit: int = 100) -> str:
        memories = await self.get_visual_memories(limit)
        if not memories: return ""
        lines = ["id\ttype\tcontent\tpriority"]
        for m in memories:
            c = str(m.get("content", "")).replace("\t", " ")
            lines.append(f"{m['id']}\t{m['type']}\t{c}\t{m['priority']}")
        return "\n".join(lines)
```

**Step 3: Run tests**
```bash
python -m pytest tests/test_db.py -v
```

**Step 4: Commit**
```bash
git add open_brain/db.py tests/test_db.py
git commit -m "feat(open-brain): implement database connection"
```

---

## Task 4: Implement MCP Server

**Files:**
- Create: `open_brain/visual_mcp.py`
- Create: `tests/test_visual_mcp.py`

**Step 1: Write test `tests/test_visual_mcp.py`**
```python
import pytest
from unittest.mock import AsyncMock, MagicMock

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
```

**Step 2: Implement `open_brain/visual_mcp.py`**
```python
"""Visual MCP Server for Open Brain."""
import json
import base64
from typing import Dict, Any, List
from io import BytesIO
from .db import Database
from .memory_glyph import MemoryGlyphEncoder

class VisualMCPServer:
    def __init__(self, connection_string: str):
        self.db = Database(connection_string)
        self.encoder = MemoryGlyphEncoder()
        self._connected = False

    async def connect(self):
        if not self._connected:
            await self.db.connect()
            self._connected = True

    async def list_tools(self) -> List[Dict]:
        return [
            {"name": "query_memory", "description": "Query memories as TSV",
             "inputSchema": {"type": "object", "properties": {"limit": {"type": "integer", "default": 100}}}},
            {"name": "query_visual_memory", "description": "Query memories as glyph atlas",
             "inputSchema": {"type": "object", "properties": {"limit": {"type": "integer", "default": 256}}}},
            {"name": "store_memory", "description": "Store new memory",
             "inputSchema": {"type": "object", "properties": {"type": {"type": "string"}, "content": {"type": "string"}}}},
        ]

    async def call_tool(self, name: str, args: Dict) -> Dict:
        await self.connect()
        if name == "query_memory":
            tsv = await self.db.get_tsv_export(args.get("limit", 100))
            return {"format": "tsv", "content": tsv}
        elif name == "query_visual_memory":
            memories = await self.db.get_visual_memories(args.get("limit", 256))
            atlas = self.encoder.entries_to_atlas(memories, args.get("atlas_size", 512))
            from PIL import Image
            img = Image.fromarray(atlas, mode="RGBA")
            buf = BytesIO()
            img.save(buf, format="PNG")
            return {"format": "glyph_atlas", "image_base64": base64.b64encode(buf.getvalue()).decode()}
        elif name == "store_memory":
            mid = await self.db.store_memory(args)
            return {"id": mid, "status": "stored"}
```

**Step 3: Run tests**
```bash
python -m pytest tests/test_visual_mcp.py -v
```

**Step 4: Commit**
```bash
git add open_brain/visual_mcp.py tests/test_visual_mcp.py
git commit -m "feat(open-brain): implement MCP server with visual tools"
```

---

## Task 5: Add Requirements and Run All Tests

**Step 1: Create `requirements-open-brain.txt`**
```
asyncpg>=0.29.0
numpy>=1.24.0
Pillow>=10.0.0
pytest>=7.4.0
pytest-asyncio>=0.21.0
```

**Step 2: Run all tests**
```bash
python -m pytest tests/ -v
```

**Step 3: Commit**
```bash
git add requirements-open-brain.txt
git commit -m "chore(open-brain): add requirements"
```

---

## Verification Checklist

- [ ] `open_brain/` package created
- [ ] `MemoryGlyphEncoder` converts entries to glyphs with RGB encoding
- [ ] `Database` manages PostgreSQL + pgvector
- [ ] `VisualMCPServer` exposes query_memory, query_visual_memory, store_memory
- [ ] All tests pass
- [ ] All commits made

---

## Token Efficiency Summary

| Format | 100 Memories | Savings |
|--------|--------------|---------|
| JSON | ~15,000 tokens | - |
| TSV | ~9,000 tokens | 40% |
| Glyph Atlas | ~5,000 tokens | 67% |
