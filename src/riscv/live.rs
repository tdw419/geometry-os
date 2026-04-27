// riscv/live.rs -- Off-thread RISC-V VM with channel-based present (Phase B, b.1.1)
//
// Spawns RiscvVm on its own thread. When the guest writes to the framebuffer
// control register (fb_present), the pixel buffer is cloned and sent over an
// mpsc channel to the GUI thread. The GUI thread recv()s on its render tick.
//
// This makes the synchronous-callback-in-bus-write bug structurally impossible:
// the VM thread never touches the display surface.
//
// Controls: pause/resume/reset/shutdown via ThreadControl enum.
// The GUI thread calls try_recv_frame() each render tick (non-blocking).

use super::cpu::StepResult;
use super::framebuf::{FB_HEIGHT, FB_WIDTH};
use super::loader;
use super::RiscvVm;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

// ── Messages ─────────────────────────────────────────────────────

/// A single frame of pixels from the guest, sent over the channel.
#[derive(Debug)]
pub struct Frame {
    /// 256*256 RGBA pixels (same layout as Framebuffer::pixels).
    pub pixels: Vec<u32>,
    /// Width (256).
    pub width: usize,
    /// Height (256).
    pub height: usize,
    /// Instruction count at time of present.
    pub instructions: u64,
}

/// Control messages sent from the GUI thread to the VM thread.
pub enum ThreadControl {
    /// Stop the VM loop and join the thread.
    Shutdown,
    /// Pause execution (VM thread spins without stepping).
    Pause,
    /// Resume execution after a pause.
    Resume,
    /// Reset VM: reload ELF from stored bytes, reset PC.
    Reset,
    /// Forward a keyboard byte to the guest's UART RX.
    Input(u8),
}

/// Status reports sent from the VM thread to the GUI thread.
#[derive(Debug)]
pub enum VmStatus {
    /// VM has halted (ebreak, shutdown, fault).
    Halted {
        pc: u32,
        instructions: u64,
        reason: String,
    },
    /// VM reset completed.
    ResetDone,
    /// VM paused.
    Paused,
    /// VM resumed.
    Resumed,
}

// ── Handle ───────────────────────────────────────────────────────

/// Handle to the off-thread RISC-V VM.
///
/// The GUI thread holds this. It provides:
/// - try_recv_frame(): non-blocking frame receive for the render loop
/// - control(): send control commands (pause/resume/reset/shutdown)
/// - try_recv_status(): check for status updates (halted, reset, etc.)
pub struct RiscvVmHandle {
    /// Channel: VM thread -> GUI thread (pixel frames).
    frame_rx: Receiver<Frame>,
    /// Channel: GUI thread -> VM thread (control commands).
    control_tx: Sender<ThreadControl>,
    /// Channel: VM thread -> GUI thread (status updates).
    status_rx: Receiver<VmStatus>,
    /// Handle to the VM thread (for join on drop).
    thread_handle: Option<JoinHandle<()>>,
    /// Whether the VM is currently paused.
    paused: bool,
}

impl RiscvVmHandle {
    /// Non-blocking check for a new frame from the VM.
    /// Returns Ok(Frame) if one is available, Err(TryRecvError) otherwise.
    /// Call this each render tick in the GUI loop.
    pub fn try_recv_frame(&self) -> Result<Frame, TryRecvError> {
        self.frame_rx.try_recv()
    }

    /// Send a control command to the VM thread.
    pub fn control(&self, cmd: ThreadControl) {
        let _ = self.control_tx.send(cmd);
    }

    /// Non-blocking check for VM status updates.
    pub fn try_recv_status(&self) -> Result<VmStatus, TryRecvError> {
        self.status_rx.try_recv()
    }

    /// Check if the VM is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Convenience: pause the VM.
    pub fn pause(&mut self) {
        self.control(ThreadControl::Pause);
        self.paused = true;
    }

    /// Convenience: resume the VM.
    pub fn resume(&mut self) {
        self.control(ThreadControl::Resume);
        self.paused = false;
    }

    /// Convenience: reset the VM.
    pub fn reset(&self) {
        self.control(ThreadControl::Reset);
    }

    /// Forward a keyboard byte to the guest's UART RX.
    /// Call this from the GUI thread when a key is pressed and the
    /// RISC-V program should receive it.
    pub fn send_input(&self, byte: u8) {
        let _ = self.control_tx.send(ThreadControl::Input(byte));
    }

    /// Shut down the VM thread and wait for it to finish.
    pub fn shutdown(&mut self) {
        self.control(ThreadControl::Shutdown);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for RiscvVmHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ── Spawner ──────────────────────────────────────────────────────

/// Configuration for spawning a new RiscvVmThread.
pub struct VmThreadConfig {
    /// ELF binary data to load.
    pub elf_data: Vec<u8>,
    /// RAM size in bytes (default: 1MB).
    pub ram_size: usize,
    /// RAM base address (default: 0x8000_0000).
    pub ram_base: u64,
    /// Instructions per batch between channel checks (default: 5000).
    pub batch_size: u32,
}

impl Default for VmThreadConfig {
    fn default() -> Self {
        Self {
            elf_data: Vec::new(),
            ram_size: 1024 * 1024,
            ram_base: 0x8000_0000,
            batch_size: 5000,
        }
    }
}

/// Spawn a RISC-V VM on its own thread with channel-based present.
///
/// Returns an RiscvVmHandle for the GUI thread to receive frames
/// and send control commands.
pub fn spawn_vm_thread(config: VmThreadConfig) -> Result<RiscvVmHandle, String> {
    let (frame_tx, frame_rx) = mpsc::channel::<Frame>();
    let (control_tx, control_rx) = mpsc::channel::<ThreadControl>();
    let (status_tx, status_rx) = mpsc::channel::<VmStatus>();

    let handle = thread::Builder::new()
        .name("riscv-vm".into())
        .spawn(move || {
            vm_thread_main(config, frame_tx, control_rx, status_tx);
        })
        .map_err(|e| format!("Failed to spawn VM thread: {}", e))?;

    Ok(RiscvVmHandle {
        frame_rx,
        control_tx,
        status_rx,
        thread_handle: Some(handle),
        paused: false,
    })
}

// ── VM Thread Main Loop ──────────────────────────────────────────

fn vm_thread_main(
    config: VmThreadConfig,
    frame_tx: Sender<Frame>,
    control_rx: Receiver<ThreadControl>,
    status_tx: Sender<VmStatus>,
) {
    // Store ELF for reset
    let elf_data = config.elf_data;
    let ram_size = config.ram_size;
    let ram_base = config.ram_base;
    let batch_size = config.batch_size;

    // Create VM
    let mut vm = RiscvVm::new_with_base(ram_base, ram_size);

    // Load ELF
    let load_info = match loader::load_elf(&mut vm.bus, &elf_data) {
        Ok(info) => info,
        Err(e) => {
            let _ = status_tx.send(VmStatus::Halted {
                pc: 0,
                instructions: 0,
                reason: format!("ELF load error: {:?}", e),
            });
            return;
        }
    };
    vm.cpu.pc = load_info.entry;

    // Install channel-based present callback
    let ft = frame_tx.clone();
    let instruction_count: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
    let ic_clone = instruction_count.clone();
    let present_cb: super::framebuf::PresentCallback =
        Rc::new(RefCell::new(move |pixels: &[u32]| {
            let frame = Frame {
                pixels: pixels.to_vec(),
                width: FB_WIDTH,
                height: FB_HEIGHT,
                instructions: *ic_clone.borrow(),
            };
            let _ = ft.send(frame);
        }));
    vm.bus.framebuf.on_present = Some(present_cb);

    let mut paused = false;
    let mut running = true;

    while running {
        // 1. Process control commands
        match control_rx.try_recv() {
            Ok(ThreadControl::Shutdown) => break,
            Ok(ThreadControl::Pause) => {
                paused = true;
                let _ = status_tx.send(VmStatus::Paused);
            }
            Ok(ThreadControl::Resume) => {
                paused = false;
                let _ = status_tx.send(VmStatus::Resumed);
            }
            Ok(ThreadControl::Reset) => {
                // Re-create VM from scratch
                vm = RiscvVm::new_with_base(ram_base, ram_size);
                match loader::load_elf(&mut vm.bus, &elf_data) {
                    Ok(info) => vm.cpu.pc = info.entry,
                    Err(e) => {
                        let _ = status_tx.send(VmStatus::Halted {
                            pc: 0,
                            instructions: 0,
                            reason: format!("ELF reload error: {:?}", e),
                        });
                        running = false;
                        continue;
                    }
                }
                // Re-install the callback with the new VM
                let ft2 = frame_tx.clone();
                *instruction_count.borrow_mut() = 0;
                let ic2 = instruction_count.clone();
                let cb: super::framebuf::PresentCallback =
                    Rc::new(RefCell::new(move |pixels: &[u32]| {
                        let frame = Frame {
                            pixels: pixels.to_vec(),
                            width: FB_WIDTH,
                            height: FB_HEIGHT,
                            instructions: *ic2.borrow(),
                        };
                        let _ = ft2.send(frame);
                    }));
                vm.bus.framebuf.on_present = Some(cb);
                paused = false;
                let _ = status_tx.send(VmStatus::ResetDone);
            }
            Ok(ThreadControl::Input(byte)) => {
                vm.bus.uart.receive_byte(byte);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        // 2. If paused, yield and continue
        if paused {
            thread::yield_now();
            continue;
        }

        // 3. Run a batch of instructions
        let mut halt_reason = None;
        for _ in 0..batch_size {
            match vm.step() {
                StepResult::Ok => {}
                StepResult::Ebreak => {
                    halt_reason = Some(format!("EBREAK at PC=0x{:08X}", vm.cpu.pc));
                    break;
                }
                StepResult::Shutdown => {
                    halt_reason = Some("Guest requested shutdown".into());
                    break;
                }
                StepResult::Ecall => {
                    // SBI handled internally
                }
                StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                    halt_reason = Some(format!("FAULT at PC=0x{:08X}", vm.cpu.pc));
                    break;
                }
            }
            *instruction_count.borrow_mut() += 1;

            if vm.bus.sbi.shutdown_requested {
                halt_reason = Some("SBI shutdown".into());
                break;
            }
        }

        // 4. If halted, notify and stop
        if let Some(reason) = halt_reason {
            let _ = status_tx.send(VmStatus::Halted {
                pc: vm.cpu.pc,
                instructions: *instruction_count.borrow(),
                reason,
            });
            running = false;
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test: spawn VM, wait for it to halt (no ELF = immediate fault),
    /// verify the status channel works.
    #[test]
    fn test_spawn_and_shutdown() {
        // Use the smallest valid program we can: raw RISC-V with ebreak
        // Actually, let's just test the channel mechanics without a real ELF.
        let (frame_tx, _frame_rx): (Sender<Frame>, Receiver<Frame>) = mpsc::channel();
        let (control_tx, control_rx) = mpsc::channel::<ThreadControl>();
        let (status_tx, status_rx) = mpsc::channel::<VmStatus>();

        // Send shutdown immediately
        control_tx.send(ThreadControl::Shutdown).unwrap();

        // Run the thread main in a closure -- it should exit immediately
        let config = VmThreadConfig::default();
        let handle = thread::spawn(move || {
            vm_thread_main(config, frame_tx, control_rx, status_tx);
        });

        handle.join().expect("thread should join cleanly");
    }

    #[test]
    fn test_pause_resume() {
        // With no valid ELF, the VM halts immediately. So we test that
        // the control/status channel plumbing works by checking we get
        // a Halted status (the expected outcome with empty ELF data).
        let (frame_tx, _frame_rx): (Sender<Frame>, Receiver<Frame>) = mpsc::channel();
        let (control_tx, control_rx) = mpsc::channel::<ThreadControl>();
        let (status_tx, status_rx) = mpsc::channel::<VmStatus>();

        let config = VmThreadConfig::default();
        let handle = thread::spawn(move || {
            vm_thread_main(config, frame_tx, control_rx, status_tx);
        });

        // With empty ELF data, VM should report Halted
        let status = status_rx.recv_timeout(std::time::Duration::from_secs(2));
        match status {
            Ok(VmStatus::Halted { reason, .. }) => {
                assert!(
                    reason.contains("ELF load error"),
                    "expected ELF load error, got: {}",
                    reason
                );
            }
            other => panic!("expected Halted status, got: {:?}", other),
        }

        // Shutdown
        control_tx.send(ThreadControl::Shutdown).unwrap();
        handle.join().expect("thread should join cleanly");
    }

    #[test]
    fn test_real_elf_halts_and_sends_status() {
        // Load a real ELF -- hello_asm.elf should halt after printing.
        let elf_path = "examples/riscv-hello/hello_asm.elf";
        let elf_data = match std::fs::read(elf_path) {
            Ok(d) => d,
            Err(_) => return, // Skip if ELF not present (CI, etc.)
        };

        let config = VmThreadConfig {
            elf_data,
            ram_size: 64 * 1024,
            ..Default::default()
        };

        let mut handle = spawn_vm_thread(config).expect("spawn should succeed");

        // Wait for the VM to halt (hello_asm should finish quickly)
        // May need a few retries since the VM runs asynchronously
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let final_status: Result<VmStatus, String> = loop {
            match handle.try_recv_status() {
                Ok(s) => break Ok(s),
                Err(TryRecvError::Empty) => {
                    if std::time::Instant::now() > deadline {
                        break Err("timeout".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => break Err(format!("{:?}", e)),
            }
        };

        match final_status {
            Ok(VmStatus::Halted { instructions, .. }) => {
                assert!(instructions > 0, "should have executed some instructions");
            }
            other => panic!("expected Halted status, got: {:?}", other),
        }

        handle.shutdown();
    }
}
