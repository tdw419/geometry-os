#!/usr/bin/env python3
"""Start the Visual MCP Server for Open Brain.

This script starts the MCP server that exposes visual memory tools
for LM Studio or other MCP-compatible AI systems.

Usage:
    python scripts/start_mcp_server.py

Environment Variables:
    DATABASE_URL: PostgreSQL connection string with pgvector support
    EMBEDDING_BACKEND: "local" for sentence-transformers, "lm_studio" for LM Studio API
    LM_STUDIO_URL: URL for LM Studio API (default: http://localhost:1234)
"""

import asyncio
import os
import sys
import logging
import signal
from pathlib import Path
from typing import Optional

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from open_brain.visual_mcp import VisualMCPServer

logger = logging.getLogger(__name__)
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s"
)


class MCPServerRunner:
    """Runner for the Visual MCP Server."""

    def __init__(self):
        self.server: Optional[VisualMCPServer] = None
        self._shutdown_event = asyncio.Event()

    async def start(self):
        """Start the MCP server."""
        # Get configuration from environment
        database_url = os.environ.get("DATABASE_URL")
        if not database_url:
            logger.error("DATABASE_URL environment variable not set")
            logger.info("Set DATABASE_URL to your PostgreSQL connection string")
            logger.info("Example: postgresql://user:password@localhost:5432/open_brain")
            sys.exit(1)

        embedding_backend = os.environ.get("EMBEDDING_BACKEND", "local")
        lm_studio_url = os.environ.get("LM_STUDIO_URL", "http://localhost:1234")

        logger.info("=" * 60)
        logger.info("Visual MCP Server for Open Brain")
        logger.info("=" * 60)
        logger.info(f"Database: {database_url.split('@')[-1] if '@' in database_url else 'local'}")
        logger.info(f"Embedding backend: {embedding_backend}")
        if embedding_backend == "lm_studio":
            logger.info(f"LM Studio URL: {lm_studio_url}")

        # Create and connect server
        self.server = VisualMCPServer(
            connection_string=database_url,
            embedding_backend=embedding_backend,
            lm_studio_url=lm_studio_url
        )

        try:
            await self.server.connect()
            logger.info("Connected to database")

            # List available tools
            tools = await self.server.list_tools()
            logger.info("")
            logger.info("Available MCP Tools:")
            logger.info("-" * 40)
            for tool in tools:
                logger.info(f"  {tool['name']}")
                logger.info(f"    {tool['description']}")
                required = tool['inputSchema'].get('required', [])
                if required:
                    logger.info(f"    Required: {', '.join(required)}")
                logger.info("")

            logger.info("=" * 60)
            logger.info("MCP Server is running. Press Ctrl+C to stop.")
            logger.info("=" * 60)
            logger.info("")
            logger.info("For LM Studio integration:")
            logger.info("1. Open LM Studio")
            logger.info("2. Go to Developer tab or Settings")
            logger.info("3. Add MCP server configuration")
            logger.info("4. Use this server's tools for visual memory access")
            logger.info("")

            # Wait for shutdown signal
            await self._shutdown_event.wait()

        except Exception as e:
            logger.error(f"Server error: {e}")
            raise
        finally:
            await self.stop()

    async def stop(self):
        """Stop the MCP server gracefully."""
        if self.server:
            logger.info("Disconnecting from database...")
            await self.server.disconnect()
            logger.info("Server stopped")

    def request_shutdown(self):
        """Request server shutdown."""
        logger.info("Shutdown requested...")
        self._shutdown_event.set()


async def main():
    """Main entry point."""
    runner = MCPServerRunner()

    # Setup signal handlers for graceful shutdown
    def signal_handler(sig, frame):
        runner.request_shutdown()

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    try:
        await runner.start()
    except KeyboardInterrupt:
        logger.info("Keyboard interrupt received")
        runner.request_shutdown()
    except Exception as e:
        logger.error(f"Fatal error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
