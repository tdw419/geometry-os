/**
 * Geometry OS Visual IDE
 * 
 * Interactive grid-based editor for drawing visual programs.
 */

import { GeometryFont } from './GeometryFont.js';
import { SpirvRunner } from './SpirvRunner.js';
import { HilbertCurve } from './VisualCompiler.js';

export class VisualIDE {
    constructor(canvasId, options = {}) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.gridSize = options.gridSize || 16;
        this.glyphSize = 16;
        this.program = new Array(this.gridSize * this.gridSize).fill(null);
        this.font = new GeometryFont();
        this.runner = new SpirvRunner();
        
        this.selectedGlyph = null;
        this.isDrawing = false;
        this.showHilbertPath = options.showHilbertPath || false;
        this.curve = new HilbertCurve(Math.log2(this.gridSize));
        
        this._setupEvents();
    }

    async init() {
        await this.font.load();
        await this.runner.init();
        this.render();
    }

    _setupEvents() {
        this.canvas.addEventListener('mousedown', (e) => {
            this.isDrawing = true;
            this._handleMouse(e);
        });
        this.canvas.addEventListener('mousemove', (e) => {
            if (this.isDrawing) this._handleMouse(e);
        });
        window.addEventListener('mouseup', () => this.isDrawing = false);
    }

    _handleMouse(e) {
        if (!this.selectedGlyph) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        const gx = Math.floor(x / (this.glyphSize));
        const gy = Math.floor(y / (this.glyphSize));
        
        if (gx >= 0 && gx < this.gridSize && gy >= 0 && gy < this.gridSize) {
            const index = gy * this.gridSize + gx;
            if (e.buttons === 2) {
                // Right click to erase
                this.program[index] = null;
            } else {
                this.program[index] = { ...this.selectedGlyph };
            }
            this.render();
        }
    }

    selectGlyph(char, info) {
        this.selectedGlyph = { char, info };
    }

    setShowHilbertPath(show) {
        this.showHilbertPath = show;
        this.render();
    }

    render() {
        this.ctx.fillStyle = '#000';
        this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
        
        // Draw grid
        this.ctx.strokeStyle = '#222';
        for (let i = 0; i <= this.gridSize; i++) {
            this.ctx.beginPath();
            this.ctx.moveTo(i * this.glyphSize, 0);
            this.ctx.lineTo(i * this.glyphSize, this.canvas.height);
            this.ctx.stroke();
            
            this.ctx.beginPath();
            this.ctx.moveTo(0, i * this.glyphSize);
            this.ctx.lineTo(this.canvas.width, i * this.glyphSize);
            this.ctx.stroke();
        }

        // Draw Hilbert Path if enabled
        if (this.showHilbertPath) {
            this.ctx.strokeStyle = 'rgba(0, 255, 204, 0.2)';
            this.ctx.lineWidth = 2;
            this.ctx.beginPath();
            for (let d = 0; d < this.gridSize * this.gridSize; d++) {
                const { x, y } = this.curve.d2xy(d);
                const px = x * this.glyphSize + this.glyphSize / 2;
                const py = y * this.glyphSize + this.glyphSize / 2;
                if (d === 0) {
                    this.ctx.moveTo(px, py);
                } else {
                    this.ctx.lineTo(px, py);
                }
            }
            this.ctx.stroke();
            this.ctx.lineWidth = 1;
        }
        
        // Draw glyphs
        for (let i = 0; i < this.program.length; i++) {
            const glyph = this.program[i];
            if (glyph) {
                const gx = i % this.gridSize;
                const gy = Math.floor(i / this.gridSize);
                this.font.drawChar(this.ctx, glyph.char, gx * this.glyphSize, gy * this.glyphSize);
            }
        }
    }

    getProgramData() {
        return this.program;
    }

    async exportToPNG() {
        const exportCanvas = document.createElement('canvas');
        exportCanvas.width = this.gridSize * this.glyphSize;
        exportCanvas.height = this.gridSize * this.glyphSize;
        const eCtx = exportCanvas.getContext('2d');

        // Draw background
        eCtx.fillStyle = '#000';
        eCtx.fillRect(0, 0, exportCanvas.width, exportCanvas.height);

        // Draw each glyph with its semantic RGB
        for (let i = 0; i < this.program.length; i++) {
            const glyph = this.program[i];
            if (glyph) {
                const gx = i % this.gridSize;
                const gy = Math.floor(i / this.gridSize);
                this.font.drawChar(eCtx, glyph.char, gx * this.glyphSize, gy * this.glyphSize);
            }
        }

        return exportCanvas.toDataURL('image/png');
    }

    async loadFromPNG(imgElement) {
        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = imgElement.width;
        tempCanvas.height = imgElement.height;
        const tCtx = tempCanvas.getContext('2d');
        tCtx.drawImage(imgElement, 0, 0);
        
        const imageData = tCtx.getImageData(0, 0, tempCanvas.width, tempCanvas.height).data;
        const newProgram = new Array(this.gridSize * this.gridSize).fill(null);

        for (let gy = 0; gy < this.gridSize; gy++) {
            for (let gx = 0; gx < this.gridSize; gx++) {
                // Sample center pixel of each glyph cell
                const px = gx * this.glyphSize + (this.glyphSize / 2);
                const py = gy * this.glyphSize + (this.glyphSize / 2);
                const idx = (Math.floor(py) * tempCanvas.width + Math.floor(px)) * 4;
                
                const r = imageData[idx];
                const g = imageData[idx + 1];
                const b = imageData[idx + 2];
                const a = imageData[idx + 3];

                if (a > 128) {
                    // Try to find matching glyph from font metadata
                    let foundChar = null;
                    for (const [code, info] of Object.entries(this.font.metadata.glyphs)) {
                        if (info.g === g && (g === 0 ? info.b === b : true)) {
                            foundChar = info.char;
                            break;
                        }
                    }
                    newProgram[gy * this.gridSize + gx] = { 
                        char: foundChar, 
                        info: { r, g, b, a } 
                    };
                }
            }
        }
        this.program = newProgram;
        this.render();
    }
}
