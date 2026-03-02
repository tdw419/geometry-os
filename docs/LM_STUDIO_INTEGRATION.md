# LM Studio Integration Guide

## Overview

This guide explains how to integrate **LM Studio** with the Geometry OS Open Brain system. This integration enables:

- **Semantic Memory Search**: Query your knowledge base using natural language
- **Visual Memory Access**: Retrieve memories as glyph atlases for vision-capable models
- **Persistent Memory**: Store and retrieve information across conversations
- **Token-Efficient Data Transfer**: Use TSV and glyph atlas formats to minimize token usage

The Open Brain system provides MCP (Model Context Protocol) tools that LM Studio can call to access a PostgreSQL-backed memory store with vector embeddings.

---

## Prerequisites

### 1. LM Studio

Download and install LM Studio from [lmstudio.ai](https://lmstudio.ai/)

Requirements:
- LM Studio version with MCP support (check developer settings)
- A loaded model (recommended: vision-capable model for glyph atlas features)

### 2. PostgreSQL with pgvector

The Open Brain requires PostgreSQL with the `pgvector` extension.

```bash
# Ubuntu/Debian
sudo apt install postgresql postgresql-contrib
sudo apt install postgresql-16-pgvector  # Adjust version as needed

# macOS with Homebrew
brew install postgresql pgvector

# Docker
docker run -d --name open-brain-db \
  -e POSTGRES_PASSWORD=openbrain \
  -e POSTGRES_DB=open_brain \
  -p 5432:5432 \
  pgvector/pgvector:pg16
```

### 3. Python Dependencies

```bash
cd geometry-os-font-toolkit
pip install -r requirements.txt

# Core dependencies:
# - asyncpg (PostgreSQL async driver)
# - sentence-transformers (local embeddings)
# - requests (LM Studio API calls)
# - numpy, pillow (glyph atlas generation)
```

---

## Setup

### Step 1: Configure PostgreSQL Database

Create the database and enable pgvector:

```sql
-- Connect as postgres user
psql -U postgres

-- Create database
CREATE DATABASE open_brain;

-- Connect to database
\c open_brain

-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;
```

Run the database setup script:

```bash
python database_setup.py
```

### Step 2: Configure LM Studio

1. Open LM Studio
2. Load your preferred model
3. Go to **Developer** tab (or **Settings** > **Developer**)
4. Enable the **Local Server** feature:
   - Port: `1234` (default)
   - Enable CORS: Yes
5. Start the local server

Verify LM Studio is running:

```bash
curl http://localhost:1234/v1/models
```

### Step 3: Set Environment Variables

Create a `.env` file or export variables:

```bash
# PostgreSQL connection string
export DATABASE_URL="postgresql://user:password@localhost:5432/open_brain"

# LM Studio API URL
export LM_STUDIO_URL="http://localhost:1234"

# Embedding backend: "local" or "lm_studio"
export EMBEDDING_BACKEND="lm_studio"
```

**Configuration Options:**

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | Required |
| `LM_STUDIO_URL` | LM Studio API endpoint | `http://localhost:1234` |
| `EMBEDDING_BACKEND` | Embedding generation backend | `local` |

**Embedding Backend Comparison:**

| Backend | Pros | Cons |
|---------|------|------|
| `local` | No API dependency, faster for batch | Uses ~500MB RAM for model |
| `lm_studio` | Consistent with LM Studio model, no extra memory | Requires LM Studio server running |

### Step 4: Bootstrap Knowledge Base

Import existing documentation, code patterns, and geometric patterns:

```bash
python scripts/bootstrap_knowledge.py
```

This imports:
- All markdown files from `docs/`
- Python code patterns from `open_brain/` and `core/`
- Predefined geometric patterns for the glyph system

Expected output:
```
Starting Geometry OS knowledge bootstrap...
Database: localhost:5432/open_brain
Embedding backend: lm_studio
Connected to database
Found 4 documentation files
Imported 4 documentation entries
Found 12 Python files in open_brain
Imported 10 code pattern entries
Imported 15 geometric pattern entries
Bootstrap complete! Imported 29 entries
```

### Step 5: Start MCP Server

Start the Visual MCP Server:

```bash
python scripts/start_mcp_server.py
```

Output:
```
============================================================
Visual MCP Server for Open Brain
============================================================
Database: localhost:5432/open_brain
Embedding backend: lm_studio
LM Studio URL: http://localhost:1234
Connected to database

Available MCP Tools:
----------------------------------------
  query_memory
    Query memories from Open Brain database. Returns TSV format.
    Required: (none)

  query_visual_memory
    Query memories as visual glyph atlas for vision-capable models.
    Required: (none)

  store_memory
    Store a new memory entry in Open Brain.
    Required: content

  search_memory
    Semantic search over memories using embeddings.
    Required: query
----------------------------------------

MCP Server is running. Press Ctrl+C to stop.
```

### Step 6: Configure LM Studio MCP Client

In LM Studio:

1. Go to **Developer** > **MCP Servers**
2. Add new server configuration:
   - Name: `Open Brain`
   - Type: `stdio` or `http` (depending on LM Studio version)
   - Command/URL: Path to MCP server script or endpoint
3. Enable the server
4. Restart LM Studio if required

---

## Using with LM Studio

### Direct Tool Calls

Once configured, you can call tools directly in LM Studio conversations:

#### Store a Memory

```
User: Store this note: "The glyph atlas uses 16x16 pixel tiles with RGB semantic encoding"

LM Studio calls: store_memory
{
  "type": "note",
  "content": "The glyph atlas uses 16x16 pixel tiles with RGB semantic encoding",
  "priority": 0.7,
  "tags": ["glyph", "atlas", "encoding"]
}

Response:
{
  "id": 42,
  "status": "stored",
  "type": "note",
  "embedding_generated": true
}
```

#### Search Memories

```
User: Find information about glyph encoding

LM Studio calls: search_memory
{
  "query": "glyph encoding RGB semantic",
  "limit": 5
}

Response:
{
  "format": "search_results",
  "query": "glyph encoding RGB semantic",
  "results": [
    {
      "id": 42,
      "type": "note",
      "content": "The glyph atlas uses 16x16 pixel tiles with RGB semantic encoding",
      "priority": 0.7,
      "similarity": 0.89
    }
  ],
  "count": 1
}
```

#### Query Memory as TSV

```
User: Show me recent memories

LM Studio calls: query_memory
{
  "limit": 10
}

Response (TSV format):
id	type	content	priority	tags
42	note	The glyph atlas uses...	0.7	glyph,atlas
41	task	Implement SPIR-V...	0.9	task,spirv
40	decision	Use PostgreSQL...	0.8	database,decision
```

### Visual Memory Queries

For vision-capable models, use the glyph atlas:

```
User: Visualize my memory patterns

LM Studio calls: query_visual_memory
{
  "limit": 256,
  "atlas_size": 512
}

Response:
{
  "format": "glyph_atlas",
  "image_base64": "iVBORw0KGgo...",
  "legend": [
    {
      "index": 0,
      "id": 42,
      "char": "N",
      "type": "note",
      "rgb": {"r": 255, "g": 16, "b": 178},
      "content_preview": "The glyph atlas uses..."
    }
  ],
  "atlas_size": 512,
  "memory_count": 45
}
```

The vision model can "see" the memory patterns in the atlas image, where:
- **Red channel**: Visual structure (always 255 for standard glyphs)
- **Green channel**: Category opcode (determines memory type)
- **Blue channel**: Priority level (0-255 mapping to 0.0-1.0)

### Memory Type Reference

| Type | Char | Opcode | Use Case |
|------|------|--------|----------|
| note | N | 0x10 | General notes and observations |
| task | T | 0x20 | Tasks and todos |
| decision | D | 0x30 | Design decisions |
| idea | I | 0x40 | Ideas and brainstorming |
| reference | R | 0x50 | Reference materials |
| code | C | 0x60 | Code patterns and snippets |
| meeting | M | 0x70 | Meeting notes |
| project | P | 0x80 | Project-level information |

---

## Token Efficiency

One of the key benefits of this integration is token-efficient data transfer.

### Format Comparison

| Format | 100 Memories | Tokens (approx.) | Use Case |
|--------|--------------|------------------|----------|
| JSON | ~15KB | ~4,000 | Structured data, all fields |
| TSV | ~5KB | ~1,500 | Tabular data, text-only |
| Glyph Atlas | ~2KB (base64) | ~600 | Visual summary, vision models |

### Token Savings Example

For 100 memory entries:

```
JSON format:
{
  "memories": [
    {"id": 1, "type": "note", "content": "...", "priority": 0.7, ...},
    {"id": 2, "type": "task", "content": "...", "priority": 0.9, ...},
    ... (100 entries)
  ]
}
~4,000 tokens

TSV format:
id	type	content	priority
1	note	...	0.7
2	task	...	0.9
... (100 rows)
~1,500 tokens (62% reduction)

Glyph Atlas:
[512x512 PNG image as base64]
+ legend (first 20 entries)
~600 tokens (85% reduction)
```

### When to Use Each Format

| Format | Best For |
|--------|----------|
| JSON (via `store_memory`) | Writing new memories |
| TSV (via `query_memory`) | Text-based analysis, non-vision models |
| Glyph Atlas (via `query_visual_memory`) | Quick overviews, vision-capable models |

---

## Troubleshooting

### Common Issues

#### "DATABASE_URL environment variable not set"

**Solution**: Export the environment variable:
```bash
export DATABASE_URL="postgresql://user:password@localhost:5432/open_brain"
```

Or add to `.env` file and load it:
```bash
echo 'DATABASE_URL="postgresql://user:password@localhost:5432/open_brain"' > .env
source .env
```

#### "Connection refused" to LM Studio

**Solution**:
1. Verify LM Studio is running
2. Check the local server is enabled on port 1234
3. Test with: `curl http://localhost:1234/v1/models`

#### "pgvector extension not found"

**Solution**:
```sql
-- Connect to database
\c open_brain

-- Create extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Verify
\dx vector
```

#### "sentence-transformers not installed"

**Solution**:
```bash
pip install sentence-transformers
```

Or use LM Studio backend:
```bash
export EMBEDDING_BACKEND="lm_studio"
```

#### "No memories found"

**Solution**: Run the bootstrap script to import initial knowledge:
```bash
python scripts/bootstrap_knowledge.py
```

#### Slow embedding generation with LM Studio backend

**Cause**: LM Studio generates embeddings via API calls, which can be slower than local models.

**Solutions**:
1. Use `EMBEDDING_BACKEND="local"` for faster batch processing
2. Reduce batch sizes in bootstrap script
3. Use a smaller, faster model in LM Studio

#### MCP tools not appearing in LM Studio

**Solutions**:
1. Verify MCP server is running (`python scripts/start_mcp_server.py`)
2. Check LM Studio MCP configuration
3. Restart LM Studio after configuration changes
4. Check LM Studio developer console for errors

### Debug Mode

Enable verbose logging:

```bash
# Set log level
export LOG_LEVEL=DEBUG

# Run with debug output
python scripts/start_mcp_server.py
```

### Database Health Check

```python
import asyncio
from open_brain.db import Database

async def check_db():
    db = Database("postgresql://user:password@localhost:5432/open_brain")
    await db.connect()

    # Check table exists
    result = await db.pool.fetch("SELECT COUNT(*) FROM memories")
    print(f"Memory count: {result[0]['count']}")

    await db.disconnect()

asyncio.run(check_db())
```

---

## Advanced Configuration

### Custom Embedding Model

For local backend, specify a different model:

```python
from open_brain.embeddings import EmbeddingGenerator

gen = EmbeddingGenerator(
    backend="local",
    model_name="all-mpnet-base-v2"  # Larger, more accurate
)
```

### Custom LM Studio Port

```bash
export LM_STUDIO_URL="http://localhost:8080"
```

### Database Connection Pooling

The system uses `asyncpg` with built-in connection pooling. Adjust pool size:

```python
# In db.py
self.pool = await asyncpg.create_pool(
    connection_string,
    min_size=5,
    max_size=20
)
```

---

## Architecture Overview

```
+-------------+     MCP Tools      +------------------+
|             | <----------------> | Visual MCP Server|
|  LM Studio  |                    +------------------+
|             |                           |
+-------------+                           v
                                    +------------+
                                    | PostgreSQL |
                                    | + pgvector |
                                    +------------+
                                           ^
                                           | embeddings
                                    +------------+
                                    | Embedding  |
                                    | Generator  |
                                    +------------+
```

---

## Related Documentation

- [GPU OS Architecture](GPU_OS_ARCHITECTURE.md)
- [State.md Integration](STATE_MD_INTEGRATION.md)
- [Morphological Computation](MORPHOLOGICAL_COMPUTATION.md)
