// lib.rs -- Geometry OS WASM bindings
//
// Exposes the VM and assembler to JavaScript via wasm-bindgen.
// The VM runs entirely in the browser -- no server needed.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

use geometry_os::assembler;
use geometry_os::vm::Vm;

// Screen dimensions (must match VM)
const SCREEN_W: u32 = 256;
const SCREEN_H: u32 = 256;

/// The main WASM interface for Geometry OS.
///
/// Usage from JavaScript:
/// ```js
/// const geo = new GeometryOS(canvas);
/// geo.load("hello.asm");
/// geo.run();
/// ```
#[wasm_bindgen]
pub struct GeometryOS {
    vm: Vm,
    #[allow(dead_code)]
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}

#[wasm_bindgen]
impl GeometryOS {
    /// Create a new Geometry OS instance bound to an HTML canvas.
    /// The canvas will be scaled up for visibility.
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<GeometryOS, JsValue> {
        // Set up panic hook for better error messages in browser console
        console_error_panic_hook::set_once();

        let document = web_sys::window()
            .ok_or("no window")?
            .document()
            .ok_or("no document")?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| format!("canvas '{}' not found", canvas_id))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "element is not a canvas")?;

        canvas.set_width(SCREEN_W);
        canvas.set_height(SCREEN_H);

        let ctx = canvas
            .get_context("2d")
            .map_err(|_| "failed to get 2d context")?
            .ok_or("no 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "context is not 2d")?;

        Ok(GeometryOS {
            vm: Vm::new(),
            canvas,
            ctx,
        })
    }

    /// Assemble and load a program from source text.
    /// Returns Ok(bytecode_length) on success, Err(error_message) on failure.
    pub fn load(&mut self, source: &str) -> Result<usize, JsValue> {
        let result = assembler::assemble(source, 0x1000).map_err(|e| e.to_string())?;
        let bytecode = &result.pixels;
        let len = bytecode.len();

        // Copy bytecode into VM RAM starting at 0x1000
        for (i, &word) in bytecode.iter().enumerate() {
            let addr = 0x1000 + i;
            if addr < self.vm.ram.len() {
                self.vm.ram[addr] = word;
            }
        }

        // Set PC to start of program
        self.vm.pc = 0x1000;
        self.vm.halted = false;
        self.vm.frame_ready = false;

        Ok(len)
    }

    /// Run one VM step. Returns false when halted.
    pub fn step(&mut self) -> bool {
        if self.vm.halted {
            return false;
        }
        let running = self.vm.step();
        if !running {
            self.render();
        }
        running
    }

    /// Run the VM until it halts or FRAME fires.
    /// Call this in a requestAnimationFrame loop.
    /// Returns true if the VM is still running.
    pub fn tick(&mut self) -> bool {
        if self.vm.halted {
            return false;
        }

        // Execute instructions until FRAME or HALT
        let max_steps = 100_000; // Safety limit per frame
        for _ in 0..max_steps {
            if self.vm.halted {
                self.render();
                return false;
            }
            let running = self.vm.step();
            if self.vm.frame_ready {
                self.vm.frame_ready = false;
                self.render();
                return true;
            }
            if !running {
                self.render();
                return false;
            }
        }

        // Hit safety limit -- render and continue next frame
        self.render();
        true
    }

    /// Reset the VM to a clean state.
    pub fn reset(&mut self) {
        self.vm = Vm::new();
        // Clear canvas
        self.ctx.set_fill_style_str("#000000");
        self.ctx.fill_rect(0.0, 0.0, SCREEN_W as f64, SCREEN_H as f64);
    }

    /// Get the current value of a register (0-31).
    pub fn get_reg(&self, reg: usize) -> u32 {
        if reg < 32 {
            self.vm.regs[reg]
        } else {
            0
        }
    }

    /// Get the current PC.
    pub fn get_pc(&self) -> u32 {
        self.vm.pc
    }

    /// Check if the VM is halted.
    pub fn is_halted(&self) -> bool {
        self.vm.halted
    }

    /// Get the frame count.
    pub fn get_frame_count(&self) -> u32 {
        self.vm.frame_count
    }

    /// Read a word from RAM.
    pub fn peek(&self, addr: u32) -> u32 {
        if (addr as usize) < self.vm.ram.len() {
            self.vm.ram[addr as usize]
        } else {
            0
        }
    }

    /// Write a word to RAM.
    pub fn poke(&mut self, addr: u32, value: u32) {
        if (addr as usize) < self.vm.ram.len() {
            self.vm.ram[addr as usize] = value;
        }
    }

    /// Set a key press (for IKEY opcode). The VM reads RAM[0xFFF].
    /// key_code should be the ASCII/scan code value.
    pub fn key_press(&mut self, key_code: u32) {
        // Set the keyboard port
        self.vm.ram[0xFFF] = key_code;
    }

    /// Clear the keyboard port (simulates IKEY clearing it).
    pub fn key_clear(&mut self) {
        self.vm.ram[0xFFF] = 0;
    }

    /// Get the BEEP frequency/duration if pending, returns null if no beep.
    /// Returns [freq, duration] as a JsValue array, or null.
    pub fn get_beep(&mut self) -> JsValue {
        match self.vm.beep.take() {
            Some((freq, dur)) => {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from(freq));
                arr.push(&JsValue::from(dur));
                arr.into()
            }
            None => JsValue::NULL,
        }
    }

    /// Disassemble instruction at an address. Returns (mnemonic, byte_length).
    pub fn disassemble_at(&self, addr: u32) -> Vec<JsValue> {
        let (mnemonic, len) = self.vm.disassemble_at(addr);
        vec![JsValue::from(mnemonic), JsValue::from(len as u32)]
    }

    // ── Private ──────────────────────────────────────────────────────

    /// Render the VM's 256x256 screen buffer to the canvas.
    fn render(&self) {
        let screen = &self.vm.screen;

        // Create RGBA pixel data from the VM's 0xRRGGBB screen buffer
        let mut rgba = Vec::with_capacity((SCREEN_W * SCREEN_H * 4) as usize);
        for &pixel in screen {
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            rgba.push(r as u8);
            rgba.push(g as u8);
            rgba.push(b as u8);
            rgba.push(0xFF); // alpha
        }

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&rgba),
            SCREEN_W,
            SCREEN_H,
        )
        .expect("failed to create ImageData");

        self.ctx
            .put_image_data(&image_data, 0.0, 0.0)
            .expect("failed to put image data");
    }
}

/// Assemble source code and return the bytecode length (or error).
/// Convenience function for one-shot assembly without creating a VM.
#[wasm_bindgen]
pub fn assemble(source: &str) -> Result<usize, JsValue> {
    let result = assembler::assemble(source, 0x1000).map_err(|e| e.to_string())?;
    Ok(result.pixels.len())
}
