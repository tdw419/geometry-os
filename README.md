# Geometry OS Font Toolkit

A standalone toolkit for the Geometry OS morphological font system.

## What Is This?

Geometry OS uses **morphological fonts** - text rendered as visual geometry rather than symbolic characters. Each glyph:

- Exists on a 16x16 pixel grid
- Can carry semantic RGB data (instruction type, operand, metadata)
- Follows symmetry rules (rotational, asymmetric, grid, point-inversion)
- Connects via "circuit traces" and "terminal ports"

**Philosophy:** Text is a visual texture. The AI "sees" and "draws" directly.

## Quick Start

```bash
# 1. Install dependencies
pip install numpy Pillow fonttools

# 2. Run installer
python3 install.py

# 3. Start web demo
cd web && python3 -m http.server 8770

# 4. Open http://localhost:8770/demo.html
```

## Structure

```
toolkit/
├── install.py           # Main installer
├── AI_ONBOARDING.md     # AI-friendly setup guide
├── README.md            # This file
├── core/
│   ├── atlas_gen.py     # Font atlas generator
│   ├── ttf_export.py    # TTF font exporter
│   └── hilbert_util.py  # Hilbert curve utilities
├── web/
│   ├── demo.html        # Interactive web demo
│   ├── GeometryFont.js  # Browser renderer
│   └── assets/          # Generated atlas + metadata
└── examples/
    └── cli_preview.py   # CLI text renderer
```

## Using with AI

Share `AI_ONBOARDING.md` with any AI assistant:

> "Read AI_ONBOARDING.md and set up the Geometry OS font system for me."

The AI will automatically:
1. Check dependencies
2. Generate the font atlas
3. Create the TTF font file
4. Launch the demo

## Web Integration

```javascript
import { GeometryFont } from './GeometryFont.js';

const font = new GeometryFont();
await font.load();

const ctx = canvas.getContext('2d');
font.drawText(ctx, "Hello World", 10, 10, {
    scale: 2,
    tint: '#00ffcc'
});
```

## Hilbert Curve Encoding

Text can be converted to 1D Hilbert sequences for neural processing:

```python
from core.hilbert_util import HilbertCurve, glyph_to_hilbert

curve = HilbertCurve(order=4)  # 16x16 grid

# Convert glyph to 1D sequence
hilbert_seq = glyph_to_hilbert(glyph_pixels)
```

## Symmetry Types

| Type | Description |
|------|-------------|
| `SYM_NONE` | No symmetry enforcement |
| `SYM_ROT_90` | 4-way rotational symmetry |
| `SYM_ASYMMETRIC` | Directional bias (right-facing) |
| `SYM_GRID` | Mesh pattern overlay |
| `SYM_INV_POINT` | Point inversion symmetry |

## Output Files

| File | Description |
|------|-------------|
| `universal_font.rts.png` | RGBA atlas with semantic color channels |
| `glyph_info.json` | Glyph metadata (positions, symmetry, RGB values) |
| `GeometryOS-Regular.ttf` | System-installable TrueType font |

## License

Part of Geometry OS. See main repository for license details.
