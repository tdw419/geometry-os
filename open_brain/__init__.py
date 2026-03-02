"""Visual Open Brain - Geometry OS + Persistent Memory Integration."""
__version__ = "0.1.0"

from .memory_glyph import MemoryGlyphEncoder
from .db import Database
from .visual_mcp import VisualMCPServer, create_server
from .embeddings import EmbeddingGenerator, EMBEDDING_DIM

__all__ = [
    "MemoryGlyphEncoder",
    "Database",
    "VisualMCPServer",
    "create_server",
    "EmbeddingGenerator",
    "EMBEDDING_DIM",
]
