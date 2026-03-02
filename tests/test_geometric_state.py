"""Tests for Geometric State Memory encoding."""

import pytest
import numpy as np
import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from open_brain.geometric_state import GeometricStateEncoder


class TestGeometricStateEncoder:
    """Test suite for GeometricStateEncoder."""

    def test_encode_spatial_position(self):
        """Test that encode() includes x, y coordinates and opcode in green channel."""
        encoder = GeometricStateEncoder()

        entry = {
            "x": 42,
            "y": 17,
            "opcode": 0x20,
            "type": "task"
        }

        result = encoder.encode(entry)

        # Verify spatial coordinates are included
        assert result["x"] == 42
        assert result["y"] == 17

        # Verify opcode in green channel
        assert result["g"] == 0x20

        # Verify RGB encoding: R=255 (visual), G=opcode, B=symmetry
        assert result["r"] == 255
        assert "b" in result  # symmetry value

    def test_hilbert_index_calculated(self):
        """Test that Hilbert curve index is calculated from x, y position."""
        encoder = GeometricStateEncoder()

        entry = {
            "x": 5,
            "y": 3,
            "type": "note"
        }

        result = encoder.encode(entry)

        # Hilbert index should be calculated
        assert "hilbert_index" in result
        assert isinstance(result["hilbert_index"], int)
        assert result["hilbert_index"] >= 0

        # Test known Hilbert value for (5, 3) with order 4 (16x16 grid)
        # Using the HilbertCurve from core.hilbert_util
        from core.hilbert_util import HilbertCurve
        curve = HilbertCurve(order=4)
        expected_index = curve.xy2d(5, 3)
        assert result["hilbert_index"] == expected_index

    def test_to_memory_entry(self):
        """Test conversion to memory entry format."""
        encoder = GeometricStateEncoder()

        state = {
            "x": 10,
            "y": 20,
            "opcode": 0x30,
            "symmetry": 4,
            "content": "Test geometric state"
        }

        memory_entry = encoder.to_memory_entry(state)

        # Verify memory entry structure
        assert "type" in memory_entry
        assert memory_entry["type"] == "geometric"
        assert "x" in memory_entry
        assert "y" in memory_entry
        assert "opcode" in memory_entry
        assert "content" in memory_entry
        assert memory_entry["content"] == "Test geometric state"
        assert memory_entry["x"] == 10
        assert memory_entry["y"] == 20

    def test_visual_ide_state_snapshot(self):
        """Test capture_ide_snapshot() captures full IDE state."""
        encoder = GeometricStateEncoder()

        # Mock IDE state with glyphs on the canvas
        ide_state = {
            "glyphs": [
                {"x": 0, "y": 0, "char": "A", "type": "code"},
                {"x": 1, "y": 0, "char": "D", "type": "decision"},
                {"x": 0, "y": 1, "char": "T", "type": "task"},
            ],
            "viewport": {"x": 0, "y": 0, "width": 100, "height": 100},
            "cursor": {"x": 1, "y": 1}
        }

        snapshot = encoder.capture_ide_snapshot(ide_state)

        # Verify snapshot structure
        assert "timestamp" in snapshot
        assert "glyphs" in snapshot
        assert "viewport" in snapshot
        assert "cursor" in snapshot
        assert "hilbert_indices" in snapshot

        # Verify all glyphs captured
        assert len(snapshot["glyphs"]) == 3

        # Verify Hilbert indices calculated for each glyph
        assert len(snapshot["hilbert_indices"]) == 3

    def test_priority_mapping(self):
        """Test priority mapping for different glyph types."""
        encoder = GeometricStateEncoder()

        # Test ADD type (should have priority 0.9)
        add_entry = {"x": 0, "y": 0, "type": "ADD"}
        add_result = encoder.encode(add_entry)
        assert add_result["priority"] == 0.9

        # Test tone type (should have priority 0.95)
        tone_entry = {"x": 0, "y": 0, "type": "tone"}
        tone_result = encoder.encode(tone_entry)
        assert tone_result["priority"] == 0.95

        # Test default priority
        default_entry = {"x": 0, "y": 0, "type": "unknown"}
        default_result = encoder.encode(default_entry)
        assert default_result["priority"] == 0.5

    def test_encode_layout_pattern(self):
        """Test encoding of layout patterns."""
        encoder = GeometricStateEncoder()

        # Create a simple layout pattern
        layout = [
            {"x": 0, "y": 0, "type": "code"},
            {"x": 1, "y": 0, "type": "code"},
            {"x": 0, "y": 1, "type": "note"},
        ]

        pattern = encoder.encode_layout_pattern(layout)

        # Verify pattern structure
        assert "glyphs" in pattern
        assert "bounds" in pattern
        assert "hilbert_path" in pattern

        # Verify bounds calculated
        assert pattern["bounds"]["min_x"] == 0
        assert pattern["bounds"]["max_x"] == 1
        assert pattern["bounds"]["min_y"] == 0
        assert pattern["bounds"]["max_y"] == 1

        # Verify Hilbert path
        assert len(pattern["hilbert_path"]) == 3

    def test_encode_to_pixels(self):
        """Test conversion of geometric state to pixel array."""
        encoder = GeometricStateEncoder()

        entry = {
            "x": 5,
            "y": 5,
            "opcode": 0x40,
            "symmetry": 2
        }

        pixels = encoder.encode_to_pixels(entry)

        # Verify pixel array shape
        assert pixels.shape == (16, 16, 4)

        # Verify RGB encoding in pixels
        assert pixels[0, 0, 0] == 255  # R = 255 (visual)
        assert pixels[0, 0, 1] == 0x40  # G = opcode
        assert pixels[0, 0, 2] >= 0  # B = symmetry
        assert pixels[0, 0, 3] == 255  # Alpha


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
