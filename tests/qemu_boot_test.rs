// Phase 33: Integration test -- Boot RISC-V Linux via QEMU, verify console output
//
// This test requires:
//   - qemu-system-riscv64 installed (sudo apt install qemu-system-misc)
//   - Kernel images downloaded (./scripts/download_riscv_linux.sh)
//
// Marked #[ignore] so CI without QEMU doesn't fail.
// Run with: cargo test --test qemu_boot_test -- --ignored

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Returns the project root directory.
fn project_root() -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    PathBuf::from(dir)
}

/// Returns the path to the RISC-V kernel image.
fn kernel_path() -> PathBuf {
    project_root().join(".geometry_os/fs/linux/Image")
}

/// Returns the path to the RISC-V initrd.
fn initrd_path() -> PathBuf {
    project_root().join(".geometry_os/fs/linux/initrd.gz")
}

/// Check if qemu-system-riscv64 is available on PATH.
fn qemu_available() -> bool {
    Command::new("qemu-system-riscv64")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Read all output from a stdout pipe into a shared buffer.
/// Runs until EOF (pipe closed) or the child is killed.
fn drain_stdout_to_buf(stdout: std::process::ChildStdout, buf: Arc<Mutex<String>>) {
    let mut read_buf = [0u8; 4096];
    let mut stdout = stdout;
    loop {
        match stdout.read(&mut read_buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                let text = String::from_utf8_lossy(&read_buf[..n]);
                if let Ok(mut guard) = buf.lock() {
                    guard.push_str(&text);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                continue;
            }
            Err(_) => break,
        }
    }
}

/// Kill a child process and wait for it to clean up.
fn cleanup_child(child: &mut Child) {
    let _ = child.kill();
    // Wait briefly for the process to exit; don't hang if it's stubborn
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            // Still running, send SIGKILL already done, just wait
            let _ = child.wait();
        }
        Err(_) => {}
    }
}

#[test]
#[ignore] // Requires qemu-system-riscv64 and kernel images
fn test_boot_riscv_linux_verify_version() -> Result<(), Box<dyn std::error::Error>> {
    // Pre-flight checks
    assert!(qemu_available(), "qemu-system-riscv64 not found on PATH");
    let kernel = kernel_path();
    let initrd = initrd_path();
    assert!(
        kernel.exists(),
        "Kernel not found at {:?}. Run: ./scripts/download_riscv_linux.sh",
        kernel
    );
    assert!(initrd.exists(), "Initrd not found at {:?}", initrd);

    // Spawn QEMU with the RISC-V kernel
    let mut child = Command::new("qemu-system-riscv64")
        .args([
            "-machine",
            "virt",
            "-nographic",
            "-m",
            "256M",
            "-kernel",
            kernel.to_str().ok_or("kernel path is not valid UTF-8")?,
            "-initrd",
            initrd.to_str().ok_or("initrd path is not valid UTF-8")?,
            "-append",
            "console=ttyS0 panic=1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().ok_or("failed to capture QEMU stdout")?;

    // Read QEMU output in a background thread (stdout.read() is blocking)
    let output: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let writer_buf = Arc::clone(&output);
    let reader = std::thread::spawn(move || {
        drain_stdout_to_buf(stdout, writer_buf);
    });

    // Wait up to 30 seconds, checking every 500ms for "Linux version"
    let timeout = Duration::from_secs(30);
    let check_interval = Duration::from_millis(500);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        std::thread::sleep(check_interval);
        if let Ok(guard) = output.lock() {
            if guard.contains("Linux version") {
                break;
            }
        }
    }

    // Cleanup: kill QEMU (this will cause the reader thread's read() to return with EOF)
    cleanup_child(&mut child);

    // Wait for the reader thread to finish (should be quick now that QEMU is dead)
    let _ = reader.join();

    // Get the final output
    let output_text = output.lock().expect("output mutex poisoned").clone();

    // Verify we got output at all
    assert!(
        !output_text.is_empty(),
        "QEMU produced no output after {}s. Is qemu-system-riscv64 working?",
        timeout.as_secs()
    );

    // Verify "Linux version" appears in the output
    assert!(
        output_text.contains("Linux version"),
        "'Linux version' not found in QEMU output.\nFirst 2000 chars:\n{}",
        &output_text[..output_text.len().min(2000)]
    );

    eprintln!("[PASS] Linux version detected in QEMU output");
    eprintln!(
        "First 500 chars of output:\n{}",
        &output_text[..output_text.len().min(500)]
    );
    Ok(())
}
