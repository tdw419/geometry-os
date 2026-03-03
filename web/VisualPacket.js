/**
 * GOS-RP Visual Packet Encoder
 * 
 * Encodes routing control plane data into visual glyph grids.
 */

import { HilbertCurve } from './VisualCompiler.js';

export class VisualPacket {
    constructor(font) {
        this.font = font;
        this.gridSize = 8; // Small 8x8 grid for packets
        this.curve = new HilbertCurve(3); // 2^3 = 8
    }

    /**
     * Encode a Route Advertisement into a glyph grid
     * 
     * Grid Layout (Hilbert Ordered):
     * 0: Packet Type (0xA0 = Route Advertisement)
     * 1-4: Target Address (4 bytes)
     * 5: Distance (1 byte)
     * 6-9: Sender Address (4 bytes)
     * 10-63: Visual padding/signature
     */
    encodeRouteAd(targetAddr, distance, senderAddr) {
        const grid = new Array(this.gridSize * this.gridSize).fill(null);
        
        // Type
        this._setGlyph(grid, 0, '?', { g: 0xA0, b: 0 }); 
        
        // Target Address
        this._encodeUint32(grid, 1, targetAddr);
        
        // Distance
        this._setGlyph(grid, 5, distance.toString(), { g: 0, b: distance });
        
        // Sender Address
        this._encodeUint32(grid, 6, senderAddr);
        
        return grid;
    }

    /**
     * Encode a Ping packet
     */
    encodePing(senderAddr) {
        const grid = new Array(this.gridSize * this.gridSize).fill(null);
        this._setGlyph(grid, 0, 'P', { g: 0xB5, b: 0 }); // PING
        this._encodeUint32(grid, 1, senderAddr);
        return grid;
    }

    _encodeUint32(grid, startIdx, value) {
        for (let i = 0; i < 4; i++) {
            const byte = (value >> (i * 8)) & 0xFF;
            this._setGlyph(grid, startIdx + i, byte.toString(), { g: 0, b: byte });
        }
    }

    _setGlyph(grid, hilbertIdx, char, info) {
        const { x, y } = this.curve.d2xy(hilbertIdx);
        grid[y * this.gridSize + x] = { char, info };
    }

    /**
     * Render the visual packet to a canvas
     */
    render(grid, canvas) {
        const ctx = canvas.getContext('2d');
        const cellSize = canvas.width / this.gridSize;
        
        ctx.fillStyle = '#000';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        for (let i = 0; i < grid.length; i++) {
            const glyph = grid[i];
            if (glyph) {
                const x = (i % this.gridSize) * cellSize;
                const y = Math.floor(i / this.gridSize) * cellSize;
                
                // Draw semantic background
                ctx.fillStyle = `rgb(${glyph.info.r || 0}, ${glyph.info.g || 0}, ${glyph.info.b || 0})`;
                ctx.fillRect(x, y, cellSize, cellSize);
                
                // Draw glyph shape if it exists in font
                if (glyph.char) {
                    this.font.drawChar(ctx, glyph.char, x, y, { scale: cellSize / 16 });
                }
            }
        }
    }
}
