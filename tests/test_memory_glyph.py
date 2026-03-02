import pytest
import numpy as np
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

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
        assert glyph["b"] == 255

    def test_encode_to_pixels(self):
        from open_brain.memory_glyph import MemoryGlyphEncoder
        encoder = MemoryGlyphEncoder()
        entry = {"type": "note", "content": "Test", "priority": 0.5}
        pixels = encoder.encode_to_pixels(entry)
        assert pixels.shape == (16, 16, 4)
        assert pixels.dtype == np.uint8
        assert pixels[0, 0, 0] == 255  # R
        assert pixels[0, 0, 1] == 0x10  # G (note opcode)
        assert pixels[0, 0, 2] == 127  # B (0.5 * 255)

    def test_entries_to_atlas(self):
        from open_brain.memory_glyph import MemoryGlyphEncoder
        encoder = MemoryGlyphEncoder()
        entries = [
            {"type": "note", "content": "A", "priority": 0.5},
            {"type": "task", "content": "B", "priority": 0.8}
        ]
        atlas = encoder.entries_to_atlas(entries, atlas_size=32)
        assert atlas.shape == (32, 32, 4)
        assert atlas.dtype == np.uint8

    def test_unknown_type_defaults_to_note(self):
        from open_brain.memory_glyph import MemoryGlyphEncoder
        encoder = MemoryGlyphEncoder()
        entry = {"type": "unknown", "content": "Test", "priority": 0.5}
        glyph = encoder.encode(entry)
        assert glyph["char"] == "N"
        assert glyph["g"] == 0x10
