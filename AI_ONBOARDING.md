# AI Agent Instruction: Geometry OS Font Toolkit

**Role:** You are an autonomous Geometry OS Engineer.
**Objective:** Set up the Geometry OS font system, generate assets, and verify rendering.

---

## Phase 1: Installation

*Execute when user asks to "set up" or "install" the font system.*

### Step 1: Check Dependencies

Ensure these Python packages are installed:
- `numpy` - Numerical processing
- `Pillow` - Image processing
- `fonttools` - TTF generation

```bash
pip install numpy Pillow fonttools
```

### Step 2: Run Installer

```bash
python3 install.py
```

This generates:
- `web/assets/universal_font.rts.png` - Font atlas
- `web/assets/glyph_info.json` - Glyph metadata
- `GeometryOS-Regular.ttf` - System font file

### Step 3: Verify

Confirm these files exist:
```
web/assets/universal_font.rts.png
web/assets/glyph_info.json
GeometryOS-Regular.ttf
```

---

## Phase 2: Hello World

*Prove the system works.*

### CLI Preview

```bash
python3 examples/cli_preview.py
```

Outputs: `gos_preview.png` showing "GOS7" with circuit traces.

### Web Demo

```bash
cd web && python3 -m http.server 8770
```

Open: http://localhost:8770/demo.html

---

## Phase 3: Development Modules

### Module A: Adding Custom Glyphs

1. Edit `core/atlas_gen.py`
2. Find `register_glyphs()` function
3. Add: `add_glyph('X', 210, r=255, g=128, is_instruction=True, sym_type=SYM_ROT_90)`
4. Run: `python3 core/atlas_gen.py`

### Module B: Web Integration

```javascript
import { GeometryFont } from './GeometryFont.js';

const font = new GeometryFont();
await font.load();

// Draw text
const ctx = canvas.getContext('2d');
font.drawText(ctx, "SYSTEM ONLINE", 10, 10, {
    scale: 2,
    tint: '#00ffcc'
});
```

### Module C: Hilbert Encoding

```python
from core.hilbert_util import HilbertCurve, glyph_to_hilbert

curve = HilbertCurve(order=4)  # 16x16

# 2D -> 1D
distance = curve.xy2d(x=8, y=8)

# 1D -> 2D
x, y = curve.d2xy(distance)

# Convert glyph bitmap to Hilbert sequence
hilbert_seq = glyph_to_hilbert(pixels_2d)
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Missing glyphs in atlas | Regenerate: `python3 core/atlas_gen.py` |
| TTF font blank | Check fonttools installed: `pip install fonttools` |
| Web demo won't load | Run from `web/` directory with http.server |
| Black text rendering | Verify `glyph_info.json` matches atlas dimensions |

---

## Key Files

| File | Purpose |
|------|---------|
| `install.py` | Main installer script |
| `core/atlas_gen.py` | Font atlas generator |
| `core/ttf_export.py` | TTF font generator |
| `core/hilbert_util.py` | Hilbert curve utilities |
| `web/GeometryFont.js` | Browser font renderer |
| `web/demo.html` | Interactive demo |

---

**Status:** Ready for autonomous execution.
