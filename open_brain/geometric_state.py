"""Geometric State Memory Encoding for Visual Open Brain.

Encodes spatial positions, opcodes, and visual state into memory entries
that can be stored and retrieved by the Visual Open Brain system.
"""

import sys
import logging
import time
from pathlib import Path
from typing import Dict, Any, List, Optional

import numpy as np

# Add parent directory to path for core imports
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.hilbert_util import HilbertCurve

logger = logging.getLogger(__name__)

# Default Hilbert curve order (16x16 grid)
DEFAULT_HILBERT_ORDER = 4

# Priority mapping for different glyph types
PRIORITY_MAP = {
    "ADD": 0.9,
    "tone": 0.95,
    "code": 0.8,
    "task": 0.85,
    "decision": 0.75,
    "note": 0.5,
    "idea": 0.7,
    "reference": 0.6,
    "meeting": 0.65,
    "project": 0.7,
}

# Category opcodes for geometric state
GEOMETRIC_OPCODES = {
    "geometric": 0x15,
    "spatial": 0x16,
    "layout": 0x17,
    "path": 0x18,
    "region": 0x19,
}


class GeometricStateEncoder:
    """Encodes geometric state (spatial positions, opcodes) to memory format.

    Uses RGB encoding:
    - R channel: Visual structure (255 for standard)
    - G channel: Opcode/category type
    - B channel: Symmetry/metadata

    Also calculates Hilbert curve indices to preserve spatial locality
    in the 1D memory index.
    """

    GLYPH_SIZE = 16

    def __init__(self, hilbert_order: int = DEFAULT_HILBERT_ORDER):
        """Initialize the geometric state encoder.

        Args:
            hilbert_order: Order for Hilbert curve (default 4 = 16x16 grid)
        """
        self.hilbert_order = hilbert_order
        self.hilbert_curve = HilbertCurve(order=hilbert_order)
        self.priority_map = PRIORITY_MAP.copy()
        self.opcodes = GEOMETRIC_OPCODES.copy()

    def encode(self, entry: Dict[str, Any]) -> Dict[str, Any]:
        """Encode a geometric state entry with spatial coordinates.

        Args:
            entry: Dict with x, y coordinates, opcode, and optional metadata

        Returns:
            Encoded entry with RGB values and Hilbert index
        """
        x = entry.get("x", 0)
        y = entry.get("y", 0)
        opcode = entry.get("opcode", self._get_opcode_for_type(entry.get("type", "geometric")))
        symmetry = entry.get("symmetry", 0)
        entry_type = entry.get("type", "geometric")

        # Calculate Hilbert index from position
        # Handle coordinates outside grid by modulo
        grid_size = 2 ** self.hilbert_order
        x_mod = x % grid_size
        y_mod = y % grid_size
        hilbert_index = self.hilbert_curve.xy2d(x_mod, y_mod)

        # Get priority for type
        priority = self.priority_map.get(entry_type, 0.5)

        return {
            "x": x,
            "y": y,
            "r": 255,  # Visual marker
            "g": opcode,
            "b": symmetry,
            "hilbert_index": hilbert_index,
            "opcode": opcode,
            "symmetry": symmetry,
            "type": entry_type,
            "priority": priority,
            "char": entry.get("char", "G"),
        }

    def _get_opcode_for_type(self, entry_type: str) -> int:
        """Get opcode for a given type."""
        # Check geometric opcodes first
        if entry_type in self.opcodes:
            return self.opcodes[entry_type]
        # Fall back to category opcodes from memory_glyph
        from open_brain.memory_glyph import CATEGORY_OPCODES
        return CATEGORY_OPCODES.get(entry_type, 0x15)

    def to_memory_entry(self, state: Dict[str, Any]) -> Dict[str, Any]:
        """Convert geometric state to memory entry format.

        Args:
            state: Geometric state with x, y, opcode, etc.

        Returns:
            Memory entry dict suitable for storage
        """
        encoded = self.encode(state)

        return {
            "type": "geometric",
            "x": state.get("x", 0),
            "y": state.get("y", 0),
            "opcode": encoded["opcode"],
            "symmetry": encoded["symmetry"],
            "hilbert_index": encoded["hilbert_index"],
            "content": state.get("content", ""),
            "priority": encoded["priority"],
            "char": encoded["char"],
            "r": encoded["r"],
            "g": encoded["g"],
            "b": encoded["b"],
            "timestamp": state.get("timestamp", time.time()),
        }

    def capture_ide_snapshot(self, ide_state: Dict[str, Any]) -> Dict[str, Any]:
        """Capture full Visual IDE state as a snapshot.

        Args:
            ide_state: Dict with glyphs, viewport, cursor, etc.

        Returns:
            Snapshot dict with all IDE state encoded
        """
        glyphs = ide_state.get("glyphs", [])
        viewport = ide_state.get("viewport", {"x": 0, "y": 0, "width": 100, "height": 100})
        cursor = ide_state.get("cursor", {"x": 0, "y": 0})

        # Encode each glyph
        encoded_glyphs = []
        hilbert_indices = []

        for glyph in glyphs:
            encoded = self.encode(glyph)
            encoded_glyphs.append(encoded)
            hilbert_indices.append(encoded["hilbert_index"])

        return {
            "timestamp": time.time(),
            "glyphs": encoded_glyphs,
            "viewport": viewport,
            "cursor": cursor,
            "hilbert_indices": hilbert_indices,
            "glyph_count": len(encoded_glyphs),
        }

    def encode_layout_pattern(self, layout: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Encode a layout pattern of multiple glyphs.

        Args:
            layout: List of glyph dicts with positions

        Returns:
            Pattern dict with bounds and Hilbert path
        """
        if not layout:
            return {
                "glyphs": [],
                "bounds": {"min_x": 0, "max_x": 0, "min_y": 0, "max_y": 0},
                "hilbert_path": [],
            }

        # Calculate bounds
        xs = [g.get("x", 0) for g in layout]
        ys = [g.get("y", 0) for g in layout]

        bounds = {
            "min_x": min(xs),
            "max_x": max(xs),
            "min_y": min(ys),
            "max_y": max(ys),
        }

        # Encode glyphs and create Hilbert path
        encoded_glyphs = []
        hilbert_path = []

        for glyph in layout:
            encoded = self.encode(glyph)
            encoded_glyphs.append(encoded)
            hilbert_path.append({
                "x": glyph.get("x", 0),
                "y": glyph.get("y", 0),
                "hilbert_index": encoded["hilbert_index"],
            })

        # Sort Hilbert path by index for spatial locality
        hilbert_path.sort(key=lambda p: p["hilbert_index"])

        return {
            "glyphs": encoded_glyphs,
            "bounds": bounds,
            "hilbert_path": hilbert_path,
            "pattern_size": len(layout),
        }

    def encode_to_pixels(self, entry: Dict[str, Any]) -> np.ndarray:
        """Convert a geometric state entry to a 16x16 RGBA pixel array.

        Args:
            entry: Geometric state entry

        Returns:
            16x16x4 numpy array (RGBA)
        """
        encoded = self.encode(entry)

        pixels = np.zeros((self.GLYPH_SIZE, self.GLYPH_SIZE, 4), dtype=np.uint8)
        pixels[:, :, 0] = encoded["r"]  # Red = visual
        pixels[:, :, 1] = encoded["g"]  # Green = opcode
        pixels[:, :, 2] = encoded["b"]  # Blue = symmetry
        pixels[:, :, 3] = 255  # Alpha

        return pixels

    def batch_encode(self, entries: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Encode multiple geometric state entries.

        Args:
            entries: List of entry dicts

        Returns:
            List of encoded entries
        """
        return [self.encode(entry) for entry in entries]

    def batch_to_memory_entries(self, states: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Convert multiple states to memory entry format.

        Args:
            states: List of geometric state dicts

        Returns:
            List of memory entries
        """
        return [self.to_memory_entry(state) for state in states]
