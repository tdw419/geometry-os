#!/usr/bin/env python3
"""Bootstrap script to import Geometry OS knowledge into Open Brain.

Imports documentation, code patterns, and geometric patterns into the
Visual Open Brain system for semantic search and retrieval.

Usage:
    python scripts/bootstrap_knowledge.py

Environment Variables:
    DATABASE_URL: PostgreSQL connection string with pgvector support
"""

import asyncio
import os
import sys
import logging
from pathlib import Path
from typing import List, Dict, Any

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from open_brain.db import Database
from open_brain.embeddings import EmbeddingGenerator
from open_brain.geometric_state import GeometricStateEncoder

logger = logging.getLogger(__name__)
logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s")


async def bootstrap_docs(db: Database, embedding_gen: EmbeddingGenerator) -> int:
    """Import documentation into Open Brain.

    Args:
        db: Database connection
        embedding_gen: Embedding generator for semantic search

    Returns:
        Number of documentation entries imported
    """
    docs_path = Path(__file__).parent.parent / "docs"
    count = 0

    if not docs_path.exists():
        logger.warning(f"Docs directory not found: {docs_path}")
        return 0

    # Find all markdown files
    md_files = list(docs_path.glob("**/*.md"))
    logger.info(f"Found {len(md_files)} documentation files")

    for md_file in md_files:
        try:
            content = md_file.read_text(encoding="utf-8")
            if not content.strip():
                continue

            # Create entry
            entry = {
                "type": "documentation",
                "content": content,
                "priority": 0.8,
                "tags": ["docs", md_file.stem, str(md_file.parent.relative_to(docs_path))],
                "metadata": {
                    "source": str(md_file.relative_to(docs_path)),
                    "file_size": len(content),
                }
            }

            # Generate embedding
            embedding = embedding_gen.generate(content)
            embedding_list = embedding.tolist()

            # Store in database
            await db.store_memory(entry, embedding=embedding_list)
            count += 1
            logger.debug(f"Imported: {md_file.name}")

        except Exception as e:
            logger.error(f"Error importing {md_file}: {e}")

    logger.info(f"Imported {count} documentation entries")
    return count


async def bootstrap_code_patterns(db: Database, embedding_gen: EmbeddingGenerator) -> int:
    """Import code patterns into Open Brain.

    Args:
        db: Database connection
        embedding_gen: Embedding generator for semantic search

    Returns:
        Number of code pattern entries imported
    """
    base_path = Path(__file__).parent.parent
    count = 0

    # Key source directories to index
    source_dirs = [
        base_path / "open_brain",
        base_path / "core",
    ]

    for source_dir in source_dirs:
        if not source_dir.exists():
            logger.warning(f"Source directory not found: {source_dir}")
            continue

        # Find all Python files
        py_files = list(source_dir.glob("**/*.py"))
        logger.info(f"Found {len(py_files)} Python files in {source_dir.name}")

        for py_file in py_files:
            # Skip test files and __pycache__
            if "test" in py_file.name.lower() or "__pycache__" in str(py_file):
                continue

            try:
                content = py_file.read_text(encoding="utf-8")
                if not content.strip() or len(content) < 100:
                    continue

                # Extract docstring if present
                docstring = ""
                if '"""' in content:
                    start = content.find('"""')
                    end = content.find('"""', start + 3)
                    if end > start:
                        docstring = content[start+3:end].strip()

                # Create entry with code summary
                summary = docstring if docstring else f"Code from {py_file.name}"
                entry = {
                    "type": "code",
                    "content": summary,
                    "priority": 0.85,
                    "tags": ["code", "python", py_file.stem, source_dir.name],
                    "metadata": {
                        "source": str(py_file.relative_to(base_path)),
                        "file_size": len(content),
                        "has_docstring": bool(docstring),
                    }
                }

                # Generate embedding from summary + code structure
                text_to_embed = f"{summary}\n\nFile: {py_file.name}"
                embedding = embedding_gen.generate(text_to_embed)
                embedding_list = embedding.tolist()

                # Store in database
                await db.store_memory(entry, embedding=embedding_list)
                count += 1

            except Exception as e:
                logger.error(f"Error importing {py_file}: {e}")

    logger.info(f"Imported {count} code pattern entries")
    return count


async def bootstrap_geometric_patterns(db: Database, embedding_gen: EmbeddingGenerator) -> int:
    """Import geometric patterns into Open Brain.

    Args:
        db: Database connection
        embedding_gen: Embedding generator for semantic search

    Returns:
        Number of geometric pattern entries imported
    """
    encoder = GeometricStateEncoder()
    count = 0

    # Define common geometric patterns for Geometry OS
    patterns = [
        # Core patterns
        {"x": 0, "y": 0, "type": "ADD", "opcode": 0x01, "symmetry": 0, "content": "Origin point - ADD operation at (0,0)"},
        {"x": 1, "y": 0, "type": "tone", "opcode": 0x02, "symmetry": 1, "content": "Tone marker at (1,0) - horizontal symmetry"},
        {"x": 0, "y": 1, "type": "code", "opcode": 0x03, "symmetry": 2, "content": "Code marker at (0,1) - vertical symmetry"},
        {"x": 1, "y": 1, "type": "task", "opcode": 0x04, "symmetry": 3, "content": "Task marker at (1,1) - diagonal symmetry"},

        # Spatial patterns
        {"x": 10, "y": 10, "type": "region", "opcode": 0x19, "symmetry": 0, "content": "Region marker at (10,10)"},
        {"x": 20, "y": 20, "type": "path", "opcode": 0x18, "symmetry": 1, "content": "Path marker at (20,20)"},
        {"x": 30, "y": 30, "type": "layout", "opcode": 0x17, "symmetry": 2, "content": "Layout marker at (30,30)"},

        # Hilbert curve reference points
        {"x": 0, "y": 0, "type": "spatial", "opcode": 0x16, "symmetry": 0, "content": "Hilbert curve start (0,0) - index 0"},
        {"x": 15, "y": 0, "type": "spatial", "opcode": 0x16, "symmetry": 0, "content": "Hilbert curve point (15,0)"},
        {"x": 15, "y": 15, "type": "spatial", "opcode": 0x16, "symmetry": 0, "content": "Hilbert curve corner (15,15)"},

        # Visual IDE patterns
        {"x": 100, "y": 100, "type": "idea", "opcode": 0x07, "symmetry": 0, "content": "Idea glyph at (100,100)"},
        {"x": 200, "y": 100, "type": "decision", "opcode": 0x05, "symmetry": 1, "content": "Decision glyph at (200,100)"},
        {"x": 300, "y": 100, "type": "note", "opcode": 0x08, "symmetry": 0, "content": "Note glyph at (300,100)"},

        # Project structure patterns
        {"x": 500, "y": 500, "type": "project", "opcode": 0x0A, "symmetry": 3, "content": "Project origin at (500,500)"},
        {"x": 500, "y": 501, "type": "meeting", "opcode": 0x09, "symmetry": 0, "content": "Meeting marker at (500,501)"},
    ]

    logger.info(f"Importing {len(patterns)} geometric patterns")

    for pattern in patterns:
        try:
            # Encode geometric state
            memory_entry = encoder.to_memory_entry(pattern)

            # Create entry
            entry = {
                "type": "geometric",
                "content": pattern["content"],
                "priority": memory_entry["priority"],
                "tags": ["geometric", pattern["type"], f"x={pattern['x']}", f"y={pattern['y']}"],
                "metadata": {
                    "x": pattern["x"],
                    "y": pattern["y"],
                    "opcode": pattern["opcode"],
                    "symmetry": pattern["symmetry"],
                    "hilbert_index": memory_entry["hilbert_index"],
                }
            }

            # Generate embedding
            embedding = embedding_gen.generate(pattern["content"])
            embedding_list = embedding.tolist()

            # Store in database
            await db.store_memory(entry, embedding=embedding_list)
            count += 1

        except Exception as e:
            logger.error(f"Error importing pattern {pattern}: {e}")

    logger.info(f"Imported {count} geometric pattern entries")
    return count


async def main():
    """Run all bootstrap functions."""
    # Get database URL from environment
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        logger.error("DATABASE_URL environment variable not set")
        logger.info("Set DATABASE_URL to your PostgreSQL connection string")
        logger.info("Example: postgresql://user:password@localhost:5432/open_brain")
        sys.exit(1)

    # Get embedding backend from environment
    embedding_backend = os.environ.get("EMBEDDING_BACKEND", "local")

    logger.info("Starting Geometry OS knowledge bootstrap...")
    logger.info(f"Database: {database_url.split('@')[-1] if '@' in database_url else 'local'}")
    logger.info(f"Embedding backend: {embedding_backend}")

    # Initialize components
    db = Database(database_url)
    embedding_gen = EmbeddingGenerator(backend=embedding_backend)

    try:
        # Connect to database
        await db.connect()
        logger.info("Connected to database")

        # Run bootstrap functions
        docs_count = await bootstrap_docs(db, embedding_gen)
        code_count = await bootstrap_code_patterns(db, embedding_gen)
        geo_count = await bootstrap_geometric_patterns(db, embedding_gen)

        total = docs_count + code_count + geo_count
        logger.info(f"Bootstrap complete! Imported {total} entries:")
        logger.info(f"  - Documentation: {docs_count}")
        logger.info(f"  - Code patterns: {code_count}")
        logger.info(f"  - Geometric patterns: {geo_count}")

    except Exception as e:
        logger.error(f"Bootstrap failed: {e}")
        raise
    finally:
        # Final Step: Export to SPIR-V substrate
        try:
            from open_brain.spirv_encoder import MemorySpirvEncoder
            memories = await db.get_visual_memories(limit=1000)
            if memories:
                encoder = MemorySpirvEncoder()
                spv_data = encoder.encode_memories(memories)
                output_path = Path(__file__).parent.parent / "web" / "assets" / "memory_substrate.spv"
                output_path.parent.mkdir(parents=True, exist_ok=True)
                output_path.write_bytes(spv_data)
                logger.info(f"Exported memory substrate to SPIR-V: {output_path} ({len(spv_data)} bytes)")
        except Exception as se:
            logger.error(f"Failed to export SPIR-V substrate: {se}")

        await db.disconnect()
        logger.info("Disconnected from database")


if __name__ == "__main__":
    asyncio.run(main())
