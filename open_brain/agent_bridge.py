"""
Agent Bridge for Visual Open Brain

Provides a synchronous interface for .loop and .bridge systems to store
agent memories in the PostgreSQL+pgvector database.

Memory Type Mapping:
- Planner (Gemini) → decision (0x30/Purple): Plans, analysis, decisions
- Worker (Claude) → code (0x60/Green): Code changes, results, iterations
- System → note (0x10/Cyan): State transitions, errors, metadata
"""

import os
import json
import asyncio
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional, Dict, Any, List

# Import existing components
from .db import Database
from .embeddings import EmbeddingGenerator, EMBEDDING_DIM
from .spirv_encoder import MemorySpirvEncoder, MEMORY_STRIDE

# Memory type to opcode mapping - centralized in memory_glyph.py
from .memory_glyph import CATEGORY_OPCODES


class AgentBridge:
    """Synchronous bridge for agent systems to store memories.

    Handles the async/sync boundary by running async operations
    in a dedicated event loop thread.

    Uses CATEGORY_OPCODES from memory_glyph.py for type-to-opcode mapping.
    """

    def __init__(self, database_url: Optional[str] = None):
        self.database_url = database_url or os.environ.get(
            "DATABASE_URL",
            "postgresql://postgres:openbrain@localhost:5432/open_brain"
        )
        self._db: Optional[Database] = None
        self._embedding_gen: Optional[EmbeddingGenerator] = None
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._thread: Optional[threading.Thread] = None
        self._ready = threading.Event()

    def start(self):
        """Start the background async event loop."""
        if self._thread is not None:
            return

        def run_loop():
            self._loop = asyncio.new_event_loop()
            asyncio.set_event_loop(self._loop)
            self._loop.run_until_complete(self._async_init())
            self._ready.set()
            # Keep loop alive
            self._loop.run_forever()

        self._thread = threading.Thread(target=run_loop, daemon=True)
        self._thread.start()
        self._ready.wait(timeout=10)

    async def _async_init(self):
        """Initialize async components."""
        self._db = Database(self.database_url)
        await self._db.connect()
        self._embedding_gen = EmbeddingGenerator(backend="local")

    def _run_async(self, coro):
        """Run an async coroutine in the background loop."""
        if self._loop is None:
            self.start()
        future = asyncio.run_coroutine_threadsafe(coro, self._loop)
        return future.result(timeout=30)

    def store_memory(
        self,
        content: str,
        memory_type: str = "note",
        priority: float = 0.5,
        tags: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None
    ) -> int:
        """Store a memory entry synchronously.

        Args:
            content: The memory content text
            memory_type: Type of memory (decision, code, note, error, plan, result)
            priority: Importance 0.0-1.0 (affects Z-height in 3D browser)
            tags: List of tags for categorization
            metadata: Additional metadata dict

        Returns:
            Memory ID
        """
        async def _store():
            # Generate embedding
            embedding = await self._embedding_gen.generate_async(content)

            # Build entry
            entry = {
                "type": memory_type,
                "content": content,
                "priority": priority,
                "tags": tags or [],
                "metadata": metadata or {}
            }

            # Store in database
            memory_id = await self._db.store_memory(entry, embedding)
            return memory_id

        return self._run_async(_store())

    def store_loop_iteration(
        self,
        iteration: int,
        action: str,
        target: str,
        context: str,
        result: Optional[str] = None
    ) -> int:
        """Store a .loop iteration as a memory.

        Args:
            iteration: Iteration number
            action: Action taken (READ, WRITE, EDIT, RUN, ANALYZE, DONE)
            target: Target file or command
            context: Context from STATE.md
            result: Optional result summary

        Returns:
            Memory ID
        """
        content = f"[Iteration {iteration}] {action}: {target}"
        if result:
            content += f"\nResult: {result}"
        content += f"\nContext: {context[:500]}"

        # Map action to priority
        priority_map = {
            "DONE": 0.95,
            "WRITE": 0.8,
            "EDIT": 0.75,
            "RUN": 0.7,
            "ANALYZE": 0.6,
            "READ": 0.5,
        }
        priority = priority_map.get(action, 0.5)

        return self.store_memory(
            content=content,
            memory_type="code",
            priority=priority,
            tags=["loop", f"iteration-{iteration}", action.lower()],
            metadata={
                "iteration": iteration,
                "action": action,
                "target": target,
                "source": ".loop"
            }
        )

    def store_bridge_fragment(
        self,
        fragment_type: str,
        agent: str,
        content: str,
        state_transition: Optional[str] = None
    ) -> int:
        """Store a .bridge fragment as a memory.

        Args:
            fragment_type: Type of fragment (plan, results, system, task)
            agent: Agent name (Gemini, Claude, LM_Studio)
            content: Fragment content
            state_transition: Optional state change (e.g., "WAITING_FOR_PLAN -> WAITING_FOR_EXECUTION")

        Returns:
            Memory ID
        """
        # Map fragment type to memory type
        type_map = {
            "plan": "plan",
            "results": "result",
            "system": "note",
            "task": "decision",
            "question": "decision",
            "answer": "decision",
        }
        memory_type = type_map.get(fragment_type, "note")

        # Priority based on agent and fragment
        priority = 0.7
        if agent == "Gemini":
            priority = 0.8  # Plans are important
        elif fragment_type == "results":
            priority = 0.85  # Results are most important

        full_content = f"[{agent}] {fragment_type.upper()}"
        if state_transition:
            full_content += f" | {state_transition}"
        full_content += f"\n\n{content[:2000]}"  # Truncate long content

        return self.store_memory(
            content=full_content,
            memory_type=memory_type,
            priority=priority,
            tags=["bridge", fragment_type, agent.lower()],
            metadata={
                "fragment_type": fragment_type,
                "agent": agent,
                "state_transition": state_transition,
                "source": ".bridge"
            }
        )

    def refresh_substrate(self, output_path: Optional[str] = None) -> str:
        """Export current memories to SPIR-V substrate.

        Args:
            output_path: Optional path for .spv file

        Returns:
            Path to generated .spv file
        """
        async def _refresh():
            memories = await self._db.get_visual_memories(limit=1000)

            if not memories:
                return None

            encoder = MemorySpirvEncoder()
            spv_data = encoder.encode_memories(memories)

            if output_path is None:
                project_root = Path(__file__).parent.parent
                output_path = project_root / "web" / "assets" / "memory_substrate.spv"
            else:
                output_path = Path(output_path)

            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_bytes(spv_data)

            return str(output_path)

        result = self._run_async(_refresh())
        return result

    def close(self):
        """Shutdown the bridge."""
        if self._loop and self._loop.is_running():
            self._loop.call_soon_threadsafe(self._loop.stop)
        if self._thread:
            self._thread.join(timeout=5)


# Singleton instance for easy import
_bridge_instance: Optional[AgentBridge] = None


def get_bridge() -> AgentBridge:
    """Get or create the singleton AgentBridge instance."""
    global _bridge_instance
    if _bridge_instance is None:
        _bridge_instance = AgentBridge()
    return _bridge_instance


# Convenience functions for direct import
def store_memory(content: str, memory_type: str = "note", **kwargs) -> int:
    """Store a memory using the singleton bridge."""
    return get_bridge().store_memory(content, memory_type=memory_type, **kwargs)


def store_loop_iteration(iteration: int, action: str, target: str, context: str, **kwargs) -> int:
    """Store a loop iteration using the singleton bridge."""
    return get_bridge().store_loop_iteration(iteration, action, target, context, **kwargs)


def store_bridge_fragment(fragment_type: str, agent: str, content: str, **kwargs) -> int:
    """Store a bridge fragment using the singleton bridge."""
    return get_bridge().store_bridge_fragment(fragment_type, agent, content, **kwargs)


def refresh_substrate(output_path: Optional[str] = None) -> str:
    """Refresh the SPIR-V substrate using the singleton bridge."""
    return get_bridge().refresh_substrate(output_path)
