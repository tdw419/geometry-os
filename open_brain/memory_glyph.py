"""Memory to Glyph Encoding for Visual Open Brain."""
import numpy as np
from typing import Dict, Any, List

CATEGORY_OPCODES = {
    "note": 0x10, "task": 0x20, "decision": 0x30, "idea": 0x40,
    "reference": 0x50, "code": 0x60, "meeting": 0x70, "project": 0x80,
}

TYPE_CHARS = {
    "note": "N", "task": "T", "decision": "D", "idea": "I",
    "reference": "R", "code": "C", "meeting": "M", "project": "P",
}


class MemoryGlyphEncoder:
    """Encodes memory entries to Geometry OS glyph format.

    Each glyph uses semantic RGB encoding:
    - R channel: Visual structure (255 for standard)
    - G channel: Category/type opcode
    - B channel: Priority/metadata (0-255)
    """
    GLYPH_SIZE = 16

    def __init__(self):
        self.category_opcodes = CATEGORY_OPCODES.copy()
        self.type_chars = TYPE_CHARS.copy()

    def encode(self, entry: Dict[str, Any]) -> Dict[str, Any]:
        """Convert a memory entry to glyph metadata."""
        entry_type = entry.get("type", "note").lower()
        priority = entry.get("priority", 0.5)

        char = self.type_chars.get(entry_type, "N")
        g = self.category_opcodes.get(entry_type, 0x10)
        b = int(min(max(priority, 0.0), 1.0) * 255)

        return {"char": char, "r": 255, "g": g, "b": b, "symmetry": 0}

    def encode_to_pixels(self, entry: Dict[str, Any]) -> np.ndarray:
        """Convert a memory entry to a 16x16 RGBA pixel array."""
        glyph_meta = self.encode(entry)

        pixels = np.zeros((self.GLYPH_SIZE, self.GLYPH_SIZE, 4), dtype=np.uint8)
        pixels[:, :, 0] = glyph_meta["r"]
        pixels[:, :, 1] = glyph_meta["g"]
        pixels[:, :, 2] = glyph_meta["b"]
        pixels[:, :, 3] = 255  # Alpha

        return pixels

    def entries_to_atlas(self, entries: List[Dict], atlas_size: int = 512) -> np.ndarray:
        """Convert batch of entries to atlas tile array."""
        glyphs_per_row = atlas_size // self.GLYPH_SIZE
        max_glyphs = glyphs_per_row * glyphs_per_row

        atlas = np.zeros((atlas_size, atlas_size, 4), dtype=np.uint8)

        for i, entry in enumerate(entries[:max_glyphs]):
            col = i % glyphs_per_row
            row = i // glyphs_per_row

            x = col * self.GLYPH_SIZE
            y = row * self.GLYPH_SIZE

            pixels = self.encode_to_pixels(entry)
            atlas[y:y+self.GLYPH_SIZE, x:x+self.GLYPH_SIZE] = pixels

        return atlas
