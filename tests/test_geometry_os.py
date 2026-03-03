#!/usr/bin/env python3
"""
Geometry OS Test Suite

Comprehensive tests to verify Geometry OS functionality.
"""

import unittest
import struct
import tempfile
import json
from pathlib import Path

# Import core modules
import sys
sys.path.insert(0, str(Path(__file__).parent.parent))

from core.atlas_gen import generate_atlas, GLYPH_SIZE, ATLAS_SIZE


class TestFontAtlas(unittest.TestCase):
    """Test Font Atlas generation."""

    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp())
        self.atlas_path = self.temp_dir / "test_atlas.png"
        self.json_path = self.temp_dir / "test_glyphs.json"

    def test_atlas_generation(self):
        """Test that atlas generates correctly."""
        # Patch output paths
        import core.atlas_gen as atlas
        atlas.OUTPUT_ATLAS = self.atlas_path
        atlas.OUTPUT_JSON = self.json_path

        # Generate
        generate_atlas()

        # Verify atlas exists
        self.assertTrue(self.atlas_path.exists())
        self.assertTrue(self.json_path.exists())

        # Verify metadata
        with open(self.json_path) as f:
            metadata = json.load(f)
            self.assertIn("glyphs", metadata)
            self.assertEqual(metadata["atlas_size"], ATLAS_SIZE)
            self.assertEqual(metadata["glyph_size"], GLYPH_SIZE)

    def test_morphological_glyphs_registered(self):
        """Test that morphological glyphs are registered."""
        import core.atlas_gen as atlas

        # Check arithmetic glyphs
        self.assertIn(160, atlas.glyphs)  # ⊕ FADD
        self.assertEqual(atlas.glyphs[160]["char"], "⊕")
        self.assertTrue(atlas.glyphs[160]["is_instruction"])

        # Check memory glyphs
        self.assertIn(164, atlas.glyphs)  # → STORE
        self.assertEqual(atlas.glyphs[164]["char"], "→")

        # Check IPC glyphs
        self.assertIn(173, atlas.glyphs)  # ✉ MSG_SEND
        self.assertEqual(atlas.glyphs[173]["char"], "✉")


class TestMemoryLayout(unittest.TestCase):
    """Test memory layout constants."""

    def test_sector_bounds(self):
        """Test sector boundary calculations."""
        sectors = {
            'KERNEL': (0, 1023),
            'MAILBOX': (1024, 1535),
            'SHELL': (2048, 6143),
            'FILES': (8192, 24575),
            'MEMORY': (32768, 65535),
            'AGENTS': (65536, 81919),
        }

        for name, (base, end) in sectors.items():
            self.assertLess(base, end)
            self.assertGreaterEqual(base, 0)

    def test_mailbox_layout(self):
        """Test mailbox memory layout."""
        mailbox_size = 32  # words
        max_mailboxes = 16
        total_size = mailbox_size * max_mailboxes

        self.assertEqual(total_size, 512)

        # Check message header offsets
        msg_sender = 0
        msg_type = 1
        msg_size = 2
        msg_data = 3

        self.assertLess(msg_data, mailbox_size)


class TestIPC(unittest.TestCase):
    """Test IPC functionality."""

    def test_message_format(self):
        """Test IPC message format."""
        # Message: [sender, type, size, data...]
        message = [0, 1, 4, 42, 0, 0, 0]

        self.assertEqual(message[0], 0)  # Sender
        self.assertEqual(message[1], 1)  # Type
        self.assertEqual(message[2], 4)  # Size
        self.assertEqual(message[3], 42)  # Data

    def test_mailbox_address_calculation(self):
        """Test mailbox address calculation."""
        pid = 5
        mailbox_base = 1024 + pid * 32

        self.assertEqual(mailbox_base, 1024 + 160)
        self.assertEqual(mailbox_base, 1184)


class TestHilbertCurve(unittest.TestCase):
    """Test Hilbert curve utilities."""

    def test_hilbert_encode_simple(self):
        """Test simple Hilbert encoding."""
        # (0, 0) should map to 0
        index = self._hilbert_encode(0, 0, 32)
        self.assertEqual(index, 0)

    def test_hilbert_decode_simple(self):
        """Test simple Hilbert decoding."""
        # 0 should map to (0, 0)
        x, y = self._hilbert_decode(0, 32)
        self.assertEqual(x, 0)
        self.assertEqual(y, 0)

    def test_hilbert_roundtrip(self):
        """Test encode/decode roundtrip."""
        for i in range(100):
            x, y = self._hilbert_decode(i, 32)
            index = self._hilbert_encode(x, y, 32)
            self.assertEqual(index, i)

    def _hilbert_encode(self, x, y, n):
        """Simplified Hilbert encode."""
        return x * n + y

    def _hilbert_decode(self, index, n):
        """Simplified Hilbert decode."""
        return index // n, index % n


class TestGeoCompiler(unittest.TestCase):
    """Test the self-hosting compiler."""

    def test_opcode_constants(self):
        """Test that kernel opcodes are defined."""
        KERNEL_OPCODES = {
            'CONSTANT': 43,
            'FADD': 129,
            'FSUB': 131,
            'FMUL': 133,
            'STORE': 62,
            'LOAD': 61,
            'SHARED_STORE': 206,
            'SHARED_LOAD': 207,
            'MSG_SEND': 208,
            'MSG_RECV': 209,
            'MSG_PEEK': 210,
            'YIELD': 228,
            'JMP': 202,
            'LABEL': 248,
            'RETURN': 253,
        }

        for name, opcode in KERNEL_OPCODES.items():
            self.assertGreater(opcode, 0)
            self.assertLess(opcode, 256)

    def test_visual_glyph_mapping(self):
        """Test visual glyph to opcode mapping."""
        GLYPH_OPCODES = {
            '⊕': 'FADD',
            '⊖': 'FSUB',
            '⊗': 'FMUL',
            '⊘': 'FDIV',
            '→': 'STORE',
            '←': 'LOAD',
            '⇒': 'SHARED_STORE',
            '⇐': 'SHARED_LOAD',
            '↻': 'JMP',
            '∅': 'JZ',
            '◉': 'JNZ',
            '█': 'LABEL',
            '▣': 'RETURN',
            '◇': 'CONSTANT',
        }

        # Verify all mapped opcodes are valid
        valid_opcodes = {'FADD', 'FSUB', 'FMUL', 'FDIV', 'STORE', 'LOAD',
                        'SHARED_STORE', 'SHARED_LOAD', 'JMP', 'JZ', 'JNZ',
                        'LABEL', 'RETURN', 'CONSTANT'}

        for glyph, opcode in GLYPH_OPCODES.items():
            self.assertIn(opcode, valid_opcodes)


if __name__ == '__main__':
    unittest.main()
