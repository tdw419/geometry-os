"""
Hilbert Curve Utility for Geometry OS Font Toolkit

Maps 2D coordinates to 1D space-filling curve positions.
Preserves spatial locality - nearby points in 2D stay nearby in 1D.
"""

class HilbertCurve:
    """2D Hilbert curve coordinate mapper."""

    def __init__(self, order: int):
        """
        Initialize Hilbert curve.

        Args:
            order: Grid is 2^order x 2^order (e.g., order=4 = 16x16 grid)
        """
        self.order = order
        self.size = 2 ** order

    def xy2d(self, x: int, y: int) -> int:
        """Convert (x, y) coordinates to distance along curve."""
        d = 0
        s = self.size // 2
        while s > 0:
            rx = 1 if (x & s) > 0 else 0
            ry = 1 if (y & s) > 0 else 0
            d += s * s * ((3 * rx) ^ ry)
            x, y = self._rot(s, x, y, rx, ry)
            s //= 2
        return d

    def d2xy(self, d: int) -> tuple:
        """Convert distance along curve to (x, y) coordinates."""
        x = y = 0
        s = 1
        t = d
        while s < self.size:
            rx = 1 if (t & 2) != 0 else 0
            ry = 1 if (t & 1) != 0 else 0
            x, y = self._rot(s, x, y, rx, ry)
            x += s * rx
            y += s * ry
            t //= 4
            s *= 2
        return x, y

    def _rot(self, s, x, y, rx, ry):
        """Rotate/flip quadrant."""
        if ry == 0:
            if rx == 1:
                x = s - 1 - x
                y = s - 1 - y
            x, y = y, x
        return x, y


def glyph_to_hilbert(pixels_2d, order: int = 4) -> list:
    """
    Convert a 2D glyph bitmap to Hilbert-ordered 1D sequence.

    Args:
        pixels_2d: 2D array (size x size) of pixel values
        order: Hilbert curve order (default 4 = 16x16)

    Returns:
        List of pixel values in Hilbert order
    """
    curve = HilbertCurve(order)
    size = 2 ** order
    result = []
    for d in range(size * size):
        x, y = curve.d2xy(d)
        result.append(pixels_2d[y][x] if y < len(pixels_2d) and x < len(pixels_2d[0]) else 0)
    return result


def hilbert_to_glyph(hilbert_seq, order: int = 4) -> list:
    """
    Convert Hilbert-ordered 1D sequence back to 2D glyph bitmap.

    Args:
        hilbert_seq: 1D sequence of pixel values
        order: Hilbert curve order (default 4 = 16x16)

    Returns:
        2D list (size x size) of pixel values
    """
    curve = HilbertCurve(order)
    size = 2 ** order
    result = [[0] * size for _ in range(size)]
    for d, val in enumerate(hilbert_seq):
        if d < size * size:
            x, y = curve.d2xy(d)
            result[y][x] = val
    return result


if __name__ == "__main__":
    # Demo: show Hilbert mapping for 4x4 grid
    curve = HilbertCurve(order=2)  # 4x4
    print("Hilbert Curve Order 2 (4x4 grid):")
    for y in range(4):
        row = []
        for x in range(4):
            row.append(f"{curve.xy2d(x, y):2d}")
        print(" ".join(row))
