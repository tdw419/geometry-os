# The Local Visual Substrate Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Connect LM Studio (local LLM) to Visual Open Brain, enabling the AI to "see" and remember geometric state, spatial layouts, and semantic opcodes on the infinite map.

**Architecture:** Add embedding generation layer (sentence-transformers), extend memory types to include geometric state (x, y, opcode, symmetry), configure MCP server for LM Studio discovery, and bootstrap with existing Geometry OS documentation.

**Tech Stack:** Python 3.10+, sentence-transformers, LM Studio (local), PostgreSQL + pgvector, MCP protocol

---

## Current State Analysis

### What Exists

| Component | File | Status |
|-----------|------|--------|
| MemoryGlyphEncoder | `open_brain/memory_glyph.py` | ✅ RGB encoding |
| Database | `open_brain/db.py` | ✅ PostgreSQL + pgvector |
| VisualMCPServer | `open_brain/visual_mcp.py` | ✅ MCP tools |
| Tests | `tests/test_*.py` | ✅ 14 tests passing |

### What We Build

```
open_brain/
├── embeddings.py         # Local embedding generation
├── geometric_state.py    # Spatial position + opcode memory
├── bootstrap.py          # Encode existing docs/code
└── lm_studio_bridge.py   # MCP discovery for LM Studio

scripts/
├── start_mcp_server.py   # Standalone MCP server
└── bootstrap_knowledge.py # Import Geometry OS docs
```

---

## Task 1: Add Embedding Integration

**Files:**
- Create: `open_brain/embeddings.py`
- Create: `tests/test_embeddings.py`

**Step 1: Write the failing test**

```python
# tests/test_embeddings.py
import pytest
import numpy as np
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))


class TestEmbeddingGenerator:
    def test_generate_embedding_shape(self):
        """Embedding has correct 384-dimensional shape."""
        from open_brain.embeddings import EmbeddingGenerator
        gen = EmbeddingGenerator()
        embedding = gen.generate("Test memory content")
        assert len(embedding) == 384
        assert isinstance(embedding, list)

    def test_similar_texts_have_similar_embeddings(self):
        """Semantically similar texts produce similar vectors."""
        from open_brain.embeddings import EmbeddingGenerator
        gen = EmbeddingGenerator()

        e1 = gen.generate("The API uses REST endpoints")
        e2 = gen.generate("REST API endpoints are used")
        e3 = gen.generate("The weather is sunny today")

        # Cosine similarity
        def cosine_sim(a, b):
            return np.dot(a, b) / (np.linalg.norm(a) * np.linalg.norm(b))

        sim_12 = cosine_sim(e1, e2)
        sim_13 = cosine_sim(e1, e3)

        assert sim_12 > sim_13  # Similar texts should be closer

    def test_batch_generation(self):
        """Can generate embeddings for multiple texts."""
        from open_brain.embeddings import EmbeddingGenerator
        gen = EmbeddingGenerator()

        texts = ["First", "Second", "Third"]
        embeddings = gen.generate_batch(texts)

        assert len(embeddings) == 3
        assert all(len(e) == 384 for e in embeddings)

    def test_empty_text_returns_zeros(self):
        """Empty input returns zero vector."""
        from open_brain.embeddings import EmbeddingGenerator
        gen = EmbeddingGenerator()

        embedding = gen.generate("")
        assert all(v == 0.0 for v in embedding)
```

**Step 2: Run test to verify it fails**

Run: `cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit && python3 -m pytest tests/test_embeddings.py -v`
Expected: FAIL with "ModuleNotFoundError: No module named 'open_brain.embeddings'"

**Step 3: Implement embeddings module**

```python
# open_brain/embeddings.py
"""Embedding Generation for Visual Open Brain.

Supports local embedding generation via sentence-transformers
or remote via LM Studio OpenAI-compatible endpoint.
"""
import numpy as np
from typing import List, Optional
import os


class EmbeddingGenerator:
    """Generates 384-dimensional embeddings for semantic search.

    Supports multiple backends:
    - sentence-transformers (local, default)
    - LM Studio OpenAI-compatible endpoint
    - OpenAI API (fallback)
    """

    EMBEDDING_DIM = 384

    def __init__(
        self,
        model_name: str = "all-MiniLM-L6-v2",
        backend: str = "local",
        lm_studio_url: Optional[str] = None
    ):
        self.model_name = model_name
        self.backend = backend
        self.lm_studio_url = lm_studio_url or os.getenv("LM_STUDIO_URL", "http://localhost:1234")
        self._model = None

    def _load_model(self):
        """Lazy load the sentence-transformers model."""
        if self._model is None and self.backend == "local":
            from sentence_transformers import SentenceTransformer
            self._model = SentenceTransformer(self.model_name)
        return self._model

    def generate(self, text: str) -> List[float]:
        """Generate embedding for a single text.

        Args:
            text: Input text to embed

        Returns:
            384-dimensional embedding vector
        """
        if not text or not text.strip():
            return [0.0] * self.EMBEDDING_DIM

        if self.backend == "local":
            return self._generate_local(text)
        elif self.backend == "lm_studio":
            return self._generate_lm_studio(text)
        else:
            raise ValueError(f"Unknown backend: {self.backend}")

    def _generate_local(self, text: str) -> List[float]:
        """Generate embedding using local sentence-transformers."""
        model = self._load_model()
        embedding = model.encode(text, normalize_embeddings=True)
        return embedding.tolist()

    def _generate_lm_studio(self, text: str) -> List[float]:
        """Generate embedding using LM Studio OpenAI-compatible endpoint."""
        import requests

        try:
            response = requests.post(
                f"{self.lm_studio_url}/v1/embeddings",
                json={
                    "model": "text-embedding-3-small",
                    "input": text
                },
                timeout=30
            )
            response.raise_for_status()
            data = response.json()
            return data["data"][0]["embedding"]
        except Exception as e:
            # Fallback to local if LM Studio unavailable
            print(f"LM Studio unavailable, falling back to local: {e}")
            return self._generate_local(text)

    def generate_batch(self, texts: List[str]) -> List[List[float]]:
        """Generate embeddings for multiple texts.

        Args:
            texts: List of input texts

        Returns:
            List of 384-dimensional embedding vectors
        """
        if self.backend == "local":
            model = self._load_model()
            embeddings = model.encode(texts, normalize_embeddings=True)
            return [e.tolist() for e in embeddings]
        else:
            return [self.generate(t) for t in texts]

    def cosine_similarity(self, e1: List[float], e2: List[float]) -> float:
        """Calculate cosine similarity between two embeddings."""
        a = np.array(e1)
        b = np.array(e2)
        return float(np.dot(a, b) / (np.linalg.norm(a) * np.linalg.norm(b)))


# Convenience function
def embed(text: str, backend: str = "local") -> List[float]:
    """One-shot embedding generation."""
    gen = EmbeddingGenerator(backend=backend)
    return gen.generate(text)
```

**Step 4: Update requirements**

Add to `requirements-open-brain.txt`:
```
sentence-transformers>=2.2.0
requests>=2.31.0
```

**Step 5: Run test to verify it passes**

Run: `cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit && python3 -m pytest tests/test_embeddings.py -v`
Expected: PASS (4 tests)

**Step 6: Commit**

```bash
git add open_brain/embeddings.py tests/test_embeddings.py requirements-open-brain.txt
git commit -m "feat(open-brain): add embedding generation with local/LM Studio backends"
```

---

## Task 2: Add Geometric State Memory Type

**Files:**
- Create: `open_brain/geometric_state.py`
- Create: `tests/test_geometric_state.py`
- Modify: `open_brain/memory_glyph.py`

**Step 1: Write the failing test**

```python
# tests/test_geometric_state.py
import pytest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))


class TestGeometricState:
    def test_encode_spatial_position(self):
        """Geometric state includes x, y coordinates."""
        from open_brain.geometric_state import GeometricStateEncoder
        encoder = GeometricStateEncoder()

        state = encoder.encode({
            "type": "glyph",
            "x": 128,
            "y": 256,
            "opcode": 0x6A,
            "symmetry": 1
        })

        assert state["x"] == 128
        assert state["y"] == 256
        assert state["g"] == 0x6A  # Opcode in green channel

    def test_hilbert_index_calculated(self):
        """Hilbert curve index is calculated from x, y."""
        from open_brain.geometric_state import GeometricStateEncoder
        encoder = GeometricStateEncoder()

        state = encoder.encode({
            "type": "glyph",
            "x": 8,
            "y": 8,
            "opcode": 0x10
        })

        assert "hilbert_index" in state
        assert isinstance(state["hilbert_index"], int)

    def test_to_memory_entry(self):
        """Geometric state converts to memory entry format."""
        from open_brain.geometric_state import GeometricStateEncoder
        encoder = GeometricStateEncoder()

        entry = encoder.to_memory_entry({
            "type": "glyph",
            "x": 100,
            "y": 200,
            "opcode": 0x82,  # tone opcode
            "symmetry": 0,
            "content": "Sound synthesis instruction"
        })

        assert entry["type"] == "geometric_state"
        assert entry["content"]["opcode"] == 0x82
        assert entry["content"]["spatial"]["x"] == 100
        assert entry["priority"] == 0.8  # High priority for active glyphs

    def test_visual_ide_state_snapshot(self):
        """Can capture entire Visual IDE state."""
        from open_brain.geometric_state import GeometricStateEncoder
        encoder = GeometricStateEncoder()

        ide_state = {
            "glyphs": [
                {"x": 0, "y": 0, "opcode": 0x6A, "symmetry": 1},
                {"x": 1, "y": 0, "opcode": 0x6B, "symmetry": 1},
                {"x": 2, "y": 0, "opcode": 0x6C, "symmetry": 1},
            ],
            "view_offset": {"x": 0, "y": 0},
            "zoom": 1.0
        }

        snapshot = encoder.capture_ide_snapshot(ide_state)
        assert snapshot["type"] == "ide_snapshot"
        assert len(snapshot["content"]["glyphs"]) == 3
```

**Step 2: Run test to verify it fails**

Run: `cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit && python3 -m pytest tests/test_geometric_state.py -v`
Expected: FAIL

**Step 3: Implement geometric state module**

```python
# open_brain/geometric_state.py
"""Geometric State Memory for Visual Open Brain.

Encodes spatial positions, opcodes, and symmetry states
so the LLM can remember WHERE things are on the infinite map.
"""
import sys
from pathlib import Path
from typing import Dict, Any, List
import json

sys.path.insert(0, str(Path(__file__).parent.parent))
from core.hilbert_util import HilbertCurve


class GeometricStateEncoder:
    """Encodes geometric state for LLM memory.

    Memory types:
    - geometric_state: Single glyph position + opcode
    - ide_snapshot: Full Visual IDE state capture
    - layout_pattern: Reusable spatial arrangement
    """

    # Priority mapping for different glyph types
    OPCODE_PRIORITY = {
        0x6A: 0.9,  # ADD - high priority
        0x6B: 0.9,  # SUB
        0x6C: 0.9,  # MUL
        0x82: 0.95, # tone - sound is important
        0x80: 0.7,  # rect
        0x81: 0.7,  # clr
    }

    def __init__(self, hilbert_order: int = 8):
        self.hilbert = HilbertCurve(order=hilbert_order)

    def encode(self, state: Dict[str, Any]) -> Dict[str, Any]:
        """Encode geometric state with RGB channels.

        Args:
            state: Dict with x, y, opcode, symmetry

        Returns:
            Encoded state with RGB and metadata
        """
        x = state.get("x", 0)
        y = state.get("y", 0)
        opcode = state.get("opcode", 0x10)
        symmetry = state.get("symmetry", 0)

        # Calculate Hilbert index for spatial locality
        hilbert_index = self.hilbert.xy2d(x % self.hilbert.size, y % self.hilbert.size)

        # Priority based on opcode type
        priority = self.OPCODE_PRIORITY.get(opcode, 0.5)

        return {
            "x": x,
            "y": y,
            "r": 255,  # Standard visual
            "g": opcode,  # Opcode in green channel
            "b": symmetry,  # Symmetry type in blue
            "hilbert_index": hilbert_index,
            "priority": priority,
            "symmetry": symmetry
        }

    def to_memory_entry(self, state: Dict[str, Any]) -> Dict[str, Any]:
        """Convert geometric state to memory entry format.

        Args:
            state: Geometric state dict

        Returns:
            Memory entry ready for database storage
        """
        encoded = self.encode(state)

        return {
            "type": "geometric_state",
            "content": {
                "spatial": {
                    "x": encoded["x"],
                    "y": encoded["y"],
                    "hilbert_index": encoded["hilbert_index"]
                },
                "visual": {
                    "opcode": encoded["g"],
                    "symmetry": encoded["b"],
                    "rgb": [encoded["r"], encoded["g"], encoded["b"]]
                },
                "description": state.get("content", "")
            },
            "priority": encoded["priority"],
            "tags": ["geometric", "spatial", f"opcode-{encoded['g']:02x}"],
            "metadata": {
                "hilbert_index": encoded["hilbert_index"]
            }
        }

    def capture_ide_snapshot(self, ide_state: Dict[str, Any]) -> Dict[str, Any]:
        """Capture full Visual IDE state as memory entry.

        Args:
            ide_state: Dict with glyphs, view_offset, zoom

        Returns:
            Memory entry representing IDE snapshot
        """
        glyphs = ide_state.get("glyphs", [])

        # Calculate bounding box
        if glyphs:
            xs = [g["x"] for g in glyphs]
            ys = [g["y"] for g in glyphs]
            bounds = {
                "min_x": min(xs),
                "max_x": max(xs),
                "min_y": min(ys),
                "max_y": max(ys)
            }
        else:
            bounds = {"min_x": 0, "max_x": 0, "min_y": 0, "max_y": 0}

        # Encode each glyph
        encoded_glyphs = [self.encode(g) for g in glyphs]

        # Calculate Hilbert path for the layout
        hilbert_path = sorted(encoded_glyphs, key=lambda g: g["hilbert_index"])

        return {
            "type": "ide_snapshot",
            "content": {
                "glyphs": glyphs,
                "encoded_glyphs": encoded_glyphs,
                "bounds": bounds,
                "view_offset": ide_state.get("view_offset", {"x": 0, "y": 0}),
                "zoom": ide_state.get("zoom", 1.0),
                "glyph_count": len(glyphs),
                "hilbert_path": [(g["x"], g["y"]) for g in hilbert_path]
            },
            "priority": 0.95,  # Snapshots are high priority
            "tags": ["ide_snapshot", "layout", "visual_state"],
            "metadata": {
                "bounds": bounds,
                "glyph_count": len(glyphs)
            }
        }

    def encode_layout_pattern(
        self,
        name: str,
        glyphs: List[Dict[str, Any]],
        description: str = ""
    ) -> Dict[str, Any]:
        """Encode a reusable layout pattern.

        Args:
            name: Pattern name (e.g., "adder_circuit")
            glyphs: List of relative-position glyphs
            description: What this pattern does

        Returns:
            Memory entry for the pattern
        """
        # Normalize to relative positions from origin
        if glyphs:
            min_x = min(g["x"] for g in glyphs)
            min_y = min(g["y"] for g in glyphs)
            normalized = [
                {"x": g["x"] - min_x, "y": g["y"] - min_y, "opcode": g.get("opcode", 0x10)}
                for g in glyphs
            ]
        else:
            normalized = []

        return {
            "type": "layout_pattern",
            "content": {
                "name": name,
                "description": description,
                "glyphs": normalized,
                "dimensions": {
                    "width": max((g["x"] for g in normalized), default=0) + 1,
                    "height": max((g["y"] for g in normalized), default=0) + 1
                }
            },
            "priority": 0.7,
            "tags": ["pattern", name, "reusable"],
            "metadata": {"pattern_name": name}
        }
```

**Step 4: Run test to verify it passes**

Run: `cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit && python3 -m pytest tests/test_geometric_state.py -v`
Expected: PASS (4 tests)

**Step 5: Commit**

```bash
git add open_brain/geometric_state.py tests/test_geometric_state.py
git commit -m "feat(open-brain): add geometric state memory type with spatial encoding"
```

---

## Task 3: Update Visual MCP Server with Embeddings

**Files:**
- Modify: `open_brain/visual_mcp.py`
- Modify: `tests/test_visual_mcp.py`

**Step 1: Add test for semantic search**

```python
# Add to tests/test_visual_mcp.py

class TestVisualMCPWithEmbeddings:
    @pytest.mark.asyncio
    async def test_store_with_embedding(self):
        """Store memory generates embedding automatically."""
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        server.db = MagicMock()
        server.db.store_memory = AsyncMock(return_value=42)
        server._connected = True

        result = await server.call_tool("store_memory", {
            "type": "note",
            "content": "The API uses PostgreSQL with pgvector",
            "priority": 0.8
        })

        assert result["id"] == 42
        # Verify embedding was generated
        server.db.store_memory.assert_called_once()
        call_args = server.db.store_memory.call_args
        assert call_args[0][1] is not None  # embedding was passed

    @pytest.mark.asyncio
    async def test_semantic_search(self):
        """Search finds semantically similar memories."""
        from open_brain.visual_mcp import VisualMCPServer
        server = VisualMCPServer("postgresql://test:test@localhost/openbrain_test")

        server.db = MagicMock()
        server.db.search_by_embedding = AsyncMock(return_value=[
            {"id": 1, "content": "PostgreSQL database configuration", "similarity": 0.85}
        ])
        server._connected = True

        result = await server.call_tool("search_memory", {
            "query": "database setup",
            "limit": 5
        })

        assert result["format"] == "search_results"
        assert len(result["results"]) == 1
```

**Step 2: Run test to verify it fails**

Run: `cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit && python3 -m pytest tests/test_visual_mcp.py::TestVisualMCPWithEmbeddings -v`
Expected: FAIL

**Step 3: Update visual_mcp.py**

Add to `open_brain/visual_mcp.py`:

```python
# Add import at top
from .embeddings import EmbeddingGenerator

# Add to __init__
def __init__(self, connection_string: str, embedding_backend: str = "local"):
    self.db = Database(connection_string)
    self.encoder = MemoryGlyphEncoder()
    self.embedding_gen = EmbeddingGenerator(backend=embedding_backend)
    self._connected = False

# Add to list_tools
{
    "name": "search_memory",
    "description": "Semantic search over memories using embeddings",
    "inputSchema": {
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query"},
            "limit": {"type": "integer", "default": 10}
        },
        "required": ["query"]
    }
}

# Add to call_tool
elif name == "search_memory":
    return await self._search_memory(arguments)

# Add new method
async def _search_memory(self, args: Dict[str, Any]) -> Dict[str, Any]:
    """Semantic search over memories."""
    query = args.get("query", "")
    limit = args.get("limit", 10)

    # Generate query embedding
    query_embedding = self.embedding_gen.generate(query)

    # Search database
    results = await self.db.search_by_embedding(query_embedding, limit=limit)

    return {
        "format": "search_results",
        "query": query,
        "results": results,
        "count": len(results)
    }

# Update _store_memory to generate embeddings
async def _store_memory(self, args: Dict[str, Any]) -> Dict[str, Any]:
    entry = {
        "type": args.get("type", "note"),
        "content": args.get("content", ""),
        "priority": args.get("priority", 0.5),
        "tags": args.get("tags", []),
        "metadata": {}
    }

    # Generate embedding from content
    content_text = str(entry["content"])
    embedding = self.embedding_gen.generate(content_text)

    memory_id = await self.db.store_memory(entry, embedding=embedding)

    return {
        "id": memory_id,
        "status": "stored",
        "type": entry["type"],
        "embedding_dim": len(embedding)
    }
```

**Step 4: Run test to verify it passes**

Run: `cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit && python3 -m pytest tests/test_visual_mcp.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add open_brain/visual_mcp.py tests/test_visual_mcp.py
git commit -m "feat(open-brain): add semantic search with embedding generation"
```

---

## Task 4: Create Bootstrap Script

**Files:**
- Create: `scripts/bootstrap_knowledge.py`
- Create: `scripts/start_mcp_server.py`

**Step 1: Create bootstrap script**

```python
# scripts/bootstrap_knowledge.py
#!/usr/bin/env python3
"""Bootstrap Geometry OS knowledge into Visual Open Brain.

Encodes documentation, source code patterns, and existing
knowledge into the memory database for LLM context.
"""
import asyncio
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from open_brain.db import Database
from open_brain.embeddings import EmbeddingGenerator
from open_brain.geometric_state import GeometricStateEncoder


async def bootstrap_docs(db: Database, embedder: EmbeddingGenerator):
    """Import documentation files."""
    docs_dir = Path(__file__).parent.parent / "docs"

    for md_file in docs_dir.glob("**/*.md"):
        content = md_file.read_text()

        # Split into sections
        sections = content.split("\n## ")

        for i, section in enumerate(sections):
            if len(section.strip()) < 50:
                continue

            title = section.split("\n")[0].strip()
            embedding = embedder.generate(section[:500])

            await db.store_memory({
                "type": "reference",
                "content": {
                    "title": title,
                    "source": str(md_file.relative_to(docs_dir)),
                    "section_index": i,
                    "text": section[:1000]
                },
                "priority": 0.7,
                "tags": ["docs", md_file.stem],
            }, embedding=embedding)

            print(f"  Imported: {md_file.name} - {title[:40]}")


async def bootstrap_code_patterns(db: Database, embedder: EmbeddingGenerator):
    """Import code patterns from source files."""
    src_dir = Path(__file__).parent.parent / "core"

    for py_file in src_dir.glob("**/*.py"):
        content = py_file.read_text()

        # Extract docstrings and function signatures
        import ast
        tree = ast.parse(content)

        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)):
                docstring = ast.get_docstring(node) or ""
                if len(docstring) < 30:
                    continue

                embedding = embedder.generate(f"{node.name}: {docstring}")

                await db.store_memory({
                    "type": "code",
                    "content": {
                        "name": node.name,
                        "docstring": docstring,
                        "source_file": str(py_file.relative_to(src_dir)),
                        "line_number": node.lineno
                    },
                    "priority": 0.6,
                    "tags": ["code", py_file.stem, node.__class__.__name__],
                }, embedding=embedding)


async def bootstrap_geometric_patterns(db: Database, encoder: GeometricStateEncoder):
    """Import known geometric patterns."""
    patterns = [
        {
            "name": "adder_circuit",
            "description": "Two-input addition using + glyph",
            "glyphs": [
                {"x": 0, "y": 0, "opcode": 0x00},  # Input A
                {"x": 1, "y": 0, "opcode": 0x00},  # Input B
                {"x": 2, "y": 0, "opcode": 0x6A},  # ADD
            ]
        },
        {
            "name": "sound_tone",
            "description": "Play a tone with frequency, duration, volume",
            "glyphs": [
                {"x": 0, "y": 0, "opcode": 0x00},  # frequency (440)
                {"x": 1, "y": 0, "opcode": 0x00},  # duration (0.5)
                {"x": 2, "y": 0, "opcode": 0x00},  # volume (1.0)
                {"x": 3, "y": 0, "opcode": 0x82},  # tone
            ]
        }
    ]

    for pattern in patterns:
        entry = encoder.encode_layout_pattern(
            name=pattern["name"],
            glyphs=pattern["glyphs"],
            description=pattern["description"]
        )
        await db.store_memory(entry)
        print(f"  Imported pattern: {pattern['name']}")


async def main():
    print("Bootstrapping Geometry OS knowledge...")
    print()

    db = Database("postgresql://localhost/openbrain")
    await db.connect()

    embedder = EmbeddingGenerator(backend="local")
    geo_encoder = GeometricStateEncoder()

    print("1. Importing documentation...")
    await bootstrap_docs(db, embedder)
    print()

    print("2. Importing code patterns...")
    await bootstrap_code_patterns(db, embedder)
    print()

    print("3. Importing geometric patterns...")
    await bootstrap_geometric_patterns(db, geo_encoder)
    print()

    await db.disconnect()
    print("Bootstrap complete!")


if __name__ == "__main__":
    asyncio.run(main())
```

**Step 2: Create MCP server startup script**

```python
# scripts/start_mcp_server.py
#!/usr/bin/env python3
"""Start the Visual Open Brain MCP server.

This server exposes memory tools to LM Studio or other
MCP-compatible LLM clients.
"""
import asyncio
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from open_brain.visual_mcp import VisualMCPServer


async def main():
    # Get connection string from environment
    db_url = os.getenv(
        "DATABASE_URL",
        "postgresql://localhost/openbrain"
    )

    # Get embedding backend preference
    embedding_backend = os.getenv("EMBEDDING_BACKEND", "local")

    print(f"Starting Visual Open Brain MCP Server...")
    print(f"  Database: {db_url}")
    print(f"  Embedding backend: {embedding_backend}")
    print()

    server = VisualMCPServer(
        connection_string=db_url,
        embedding_backend=embedding_backend
    )

    await server.connect()

    # List available tools
    tools = await server.list_tools()
    print("Available tools:")
    for tool in tools:
        print(f"  - {tool['name']}: {tool['description'][:60]}...")
    print()

    # Keep server running
    print("Server ready. Press Ctrl+C to stop.")
    try:
        while True:
            await asyncio.sleep(1)
    except KeyboardInterrupt:
        print("\nShutting down...")

    await server.disconnect()


if __name__ == "__main__":
    asyncio.run(main())
```

**Step 3: Make scripts executable**

```bash
chmod +x scripts/bootstrap_knowledge.py
chmod +x scripts/start_mcp_server.py
```

**Step 4: Commit**

```bash
git add scripts/
git commit -m "feat(open-brain): add bootstrap and MCP server startup scripts"
```

---

## Task 5: Add LM Studio Integration Guide

**Files:**
- Create: `docs/LM_STUDIO_INTEGRATION.md`

**Step 1: Create integration guide**

```markdown
# docs/LM_STUDIO_INTEGRATION.md
# LM Studio Integration Guide

## Overview

This guide shows how to connect LM Studio (local LLM) to Visual Open Brain,
enabling the AI to "see" and remember geometric state on the infinite map.

## Prerequisites

1. LM Studio installed and running
2. PostgreSQL with pgvector extension
3. Python 3.10+ with dependencies

## Setup

### 1. Configure LM Studio

1. Open LM Studio
2. Go to Settings → Developer
3. Enable "Enable API Server"
4. Set port to 1234 (default)
5. Load a model (e.g., Qwen2.5-7B-Instruct)

### 2. Set Environment Variables

```bash
export DATABASE_URL="postgresql://user:pass@localhost/openbrain"
export LM_STUDIO_URL="http://localhost:1234"
export EMBEDDING_BACKEND="lm_studio"  # or "local"
```

### 3. Bootstrap Knowledge

```bash
python scripts/bootstrap_knowledge.py
```

### 4. Start MCP Server

```bash
python scripts/start_mcp_server.py
```

## Using with LM Studio

### Direct Tool Calls

In LM Studio chat, you can reference the tools:

```
Please use the store_memory tool to save this decision:
We will use PostgreSQL with pgvector for semantic search.
```

### Memory Queries

```
Use query_memory to find all decisions about the API.
```

### Visual Memory

```
Use query_visual_memory to show me the current glyph layout.
```

## Token Efficiency

| Format | 100 Memories | Savings |
|--------|--------------|---------|
| JSON | ~15,000 tokens | - |
| TSV (query_memory) | ~9,000 tokens | 40% |
| Glyph Atlas (query_visual_memory) | ~5,000 tokens | 67% |

## Troubleshooting

### LM Studio Not Responding

```bash
curl http://localhost:1234/v1/models
```

### Database Connection Failed

```bash
psql -d openbrain -c "SELECT * FROM memory_entries LIMIT 1;"
```

### Embedding Generation Failed

Ensure sentence-transformers is installed:
```bash
pip install sentence-transformers
```
```

**Step 2: Commit**

```bash
git add docs/LM_STUDIO_INTEGRATION.md
git commit -m "docs(open-brain): add LM Studio integration guide"
```

---

## Verification Checklist

- [ ] EmbeddingGenerator generates 384-dim vectors
- [ ] GeometricStateEncoder captures spatial positions
- [ ] VisualMCPServer supports semantic search
- [ ] Bootstrap script imports docs/code
- [ ] MCP server startup script works
- [ ] All tests pass

---

## Summary

| Component | File | Purpose |
|-----------|------|---------|
| Embeddings | `open_brain/embeddings.py` | Local/LM Studio embedding generation |
| Geometric State | `open_brain/geometric_state.py` | Spatial memory encoding |
| MCP Tools | `open_brain/visual_mcp.py` | store_memory, search_memory, query_visual_memory |
| Bootstrap | `scripts/bootstrap_knowledge.py` | Import existing knowledge |
| Server | `scripts/start_mcp_server.py` | Standalone MCP server |
| Guide | `docs/LM_STUDIO_INTEGRATION.md` | Setup instructions |
