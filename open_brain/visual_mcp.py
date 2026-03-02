"""Visual MCP Server for Open Brain."""
import json
import base64
from typing import Dict, Any, List, Optional
from io import BytesIO

from .db import Database
from .memory_glyph import MemoryGlyphEncoder
from .embeddings import EmbeddingGenerator


class VisualMCPServer:
    """MCP server exposing visual memory tools.

    Tools provided:
    - query_memory: Retrieve memories as TSV (token-efficient)
    - query_visual_memory: Retrieve memories as glyph atlas
    - store_memory: Store new memory entry
    - search_memory: Semantic search using embeddings
    """

    def __init__(
        self,
        connection_string: str,
        embedding_backend: str = "local",
        lm_studio_url: Optional[str] = None
    ):
        self.db = Database(connection_string)
        self.encoder = MemoryGlyphEncoder()
        self.embedding_gen = EmbeddingGenerator(
            backend=embedding_backend,
            lm_studio_url=lm_studio_url
        )
        self._connected = False

    async def connect(self):
        """Connect to database."""
        if not self._connected:
            await self.db.connect()
            self._connected = True

    async def disconnect(self):
        """Disconnect from database."""
        await self.db.disconnect()
        self._connected = False

    async def list_tools(self) -> List[Dict[str, Any]]:
        """Return list of available MCP tools."""
        return [
            {
                "name": "query_memory",
                "description": "Query memories from Open Brain database. Returns TSV format for token efficiency.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum memories to return",
                            "default": 100
                        }
                    }
                }
            },
            {
                "name": "query_visual_memory",
                "description": "Query memories as visual glyph atlas. Vision-capable models can 'see' memory patterns.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum memories to encode",
                            "default": 256
                        },
                        "atlas_size": {
                            "type": "integer",
                            "description": "Output atlas size in pixels",
                            "default": 512
                        }
                    }
                }
            },
            {
                "name": "store_memory",
                "description": "Store a new memory entry in Open Brain.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "description": "Memory type: note, task, decision, idea, reference, code, meeting, project",
                            "default": "note"
                        },
                        "content": {
                            "type": "string",
                            "description": "Memory content"
                        },
                        "priority": {
                            "type": "number",
                            "description": "Priority from 0.0 to 1.0",
                            "default": 0.5
                        },
                        "tags": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional tags"
                        }
                    },
                    "required": ["content"]
                }
            },
            {
                "name": "search_memory",
                "description": "Semantic search over memories using embeddings. Finds similar content even without exact keyword matches.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query text"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results to return",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }
            }
        ]

    async def call_tool(self, name: str, arguments: Dict[str, Any]) -> Dict[str, Any]:
        """Execute a tool call."""
        await self.connect()

        if name == "query_memory":
            return await self._query_memory(arguments)
        elif name == "query_visual_memory":
            return await self._query_visual_memory(arguments)
        elif name == "store_memory":
            return await self._store_memory(arguments)
        elif name == "search_memory":
            return await self._search_memory(arguments)
        else:
            raise ValueError(f"Unknown tool: {name}")

    async def _query_memory(self, args: Dict[str, Any]) -> Dict[str, Any]:
        """Query memories as TSV."""
        limit = args.get("limit", 100)
        tsv_content = await self.db.get_tsv_export(limit=limit)
        return {
            "format": "tsv",
            "content": tsv_content,
            "row_count": len(tsv_content.split("\n")) - 1 if tsv_content else 0
        }

    async def _query_visual_memory(self, args: Dict[str, Any]) -> Dict[str, Any]:
        """Query memories as visual glyph atlas."""
        limit = args.get("limit", 256)
        atlas_size = args.get("atlas_size", 512)

        # Get memories
        memories = await self.db.get_visual_memories(limit=limit)

        if not memories:
            return {
                "format": "glyph_atlas",
                "image_base64": "",
                "message": "No memories found"
            }

        # Generate atlas
        atlas = self.encoder.entries_to_atlas(memories, atlas_size=atlas_size)

        # Convert to base64 PNG
        from PIL import Image
        img = Image.fromarray(atlas, mode="RGBA")
        buffer = BytesIO()
        img.save(buffer, format="PNG")
        image_base64 = base64.b64encode(buffer.getvalue()).decode("utf-8")

        # Generate legend
        legend = []
        for i, m in enumerate(memories):
            glyph = self.encoder.encode(m)
            legend.append({
                "index": i,
                "id": m.get("id"),
                "char": glyph["char"],
                "type": m.get("type"),
                "rgb": {"r": glyph["r"], "g": glyph["g"], "b": glyph["b"]},
                "content_preview": str(m.get("content", ""))[:50]
            })

        return {
            "format": "glyph_atlas",
            "image_base64": image_base64,
            "legend": legend[:20],  # First 20 entries
            "atlas_size": atlas_size,
            "memory_count": len(memories)
        }

    async def _store_memory(self, args: Dict[str, Any]) -> Dict[str, Any]:
        """Store a new memory entry with automatic embedding generation."""
        content = args.get("content", "")
        entry = {
            "type": args.get("type", "note"),
            "content": content,
            "priority": args.get("priority", 0.5),
            "tags": args.get("tags", []),
            "metadata": {}
        }

        # Generate embedding from content
        embedding = self.embedding_gen.generate(content)
        embedding_list = embedding.tolist()

        memory_id = await self.db.store_memory(entry, embedding=embedding_list)

        return {
            "id": memory_id,
            "status": "stored",
            "type": entry["type"],
            "embedding_generated": True
        }

    async def _search_memory(self, args: Dict[str, Any]) -> Dict[str, Any]:
        """Semantic search over memories using embeddings."""
        query = args.get("query", "")
        limit = args.get("limit", 10)

        if not query:
            return {
                "format": "search_results",
                "results": [],
                "query": query,
                "message": "Empty query"
            }

        # Generate embedding for query
        query_embedding = self.embedding_gen.generate(query)
        embedding_list = query_embedding.tolist()

        # Search by similarity
        results = await self.db.search_by_embedding(embedding_list, limit=limit)

        # Format results
        formatted_results = []
        for r in results:
            formatted_results.append({
                "id": r.get("id"),
                "type": r.get("type"),
                "content": r.get("content"),
                "priority": r.get("priority"),
                "similarity": r.get("similarity"),
                "tags": r.get("tags", [])
            })

        return {
            "format": "search_results",
            "query": query,
            "results": formatted_results,
            "count": len(formatted_results)
        }


async def create_server(connection_string: str) -> VisualMCPServer:
    """Create and connect MCP server."""
    server = VisualMCPServer(connection_string)
    await server.connect()
    return server
