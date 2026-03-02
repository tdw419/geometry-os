/**
 * Geometry OS Font Renderer
 *
 * Renders morphological glyphs from the universal font atlas.
 * Each glyph carries visual data (R) and semantic data (G, B).
 */

export class GeometryFont {
    constructor() {
        this.atlasTexture = null;
        this.metadata = null;
        this.glyphCache = new Map();
        this.glyphSize = 16;
        this.isLoaded = false;
    }

    async load() {
        console.log('[GOS Font] Loading Universal Font Atlas...');

        // Load metadata
        const response = await fetch('assets/glyph_info.json');
        this.metadata = await response.json();
        this.glyphSize = this.metadata.glyph_size;

        // Load atlas texture (PNG for visual transparency + compression)
        this.atlasTexture = await this._loadImage('assets/universal_font.rts.png');

        // Pre-cache glyphs
        this._generateCache();

        this.isLoaded = true;
        console.log(`[GOS Font] Atlas loaded (${this.metadata.atlas_size}x${this.metadata.atlas_size})`);
    }

    async _loadImage(src) {
        return new Promise((resolve, reject) => {
            const img = new Image();
            img.onload = () => resolve(img);
            img.onerror = reject;
            img.src = src;
        });
    }

    _generateCache() {
        const cols = this.metadata.atlas_size / this.glyphSize;

        for (const [code, info] of Object.entries(this.metadata.glyphs)) {
            const i = parseInt(code);
            const x = (i % cols) * this.glyphSize;
            const y = Math.floor(i / cols) * this.glyphSize;

            this.glyphCache.set(info.char, { x, y, info });
            this.glyphCache.set(i, { x, y, info });
        }
    }

    /**
     * Draw a character to a canvas context
     */
    drawChar(ctx, char, x, y, options = {}) {
        const { scale = 1, tint = null } = options;
        const glyph = this.glyphCache.get(char);
        if (!glyph) return;

        const size = this.glyphSize * scale;

        if (tint) {
            // Draw with tint
            ctx.save();
            ctx.globalCompositeOperation = 'source-over';
            ctx.drawImage(
                this.atlasTexture,
                glyph.x, glyph.y, this.glyphSize, this.glyphSize,
                x, y, size, size
            );
            ctx.globalCompositeOperation = 'multiply';
            ctx.fillStyle = tint;
            ctx.fillRect(x, y, size, size);
            ctx.restore();
        } else {
            ctx.drawImage(
                this.atlasTexture,
                glyph.x, glyph.y, this.glyphSize, this.glyphSize,
                x, y, size, size
            );
        }
    }

    /**
     * Draw a string of text
     */
    drawText(ctx, text, x, y, options = {}) {
        const { spacing = 0, scale = 1, tint = '#00ffcc' } = options;
        let xOffset = x;

        for (const char of text) {
            this.drawChar(ctx, char, xOffset, y, { scale, tint });
            xOffset += (this.glyphSize + spacing) * scale;
        }

        return xOffset - x; // Return total width
    }

    /**
     * Get glyph metadata
     */
    getGlyphInfo(char) {
        return this.glyphCache.get(char)?.info || null;
    }
}

// Export for browser
if (typeof window !== 'undefined') {
    window.GeometryFont = GeometryFont;
}
