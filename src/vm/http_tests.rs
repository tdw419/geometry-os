// http_tests.rs -- HTTP Client Tests (Phase 81)
//
// Tests for the HTTP client library (programs/http_get.asm) and text browser
// (programs/browser.asm). Uses mock TCP servers for round-trip testing.

use super::*;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

/// Helper: read a null-terminated string from RAM at address.
fn http_read_string(vm: &Vm, addr: usize) -> String {
    let mut result = String::new();
    let mut a = addr;
    while a < vm.ram.len() {
        let ch = vm.ram[a] & 0xFF;
        if ch == 0 {
            break;
        }
        result.push(ch as u8 as char);
        a += 1;
    }
    result
}

/// Helper: write a string to RAM at address (one byte per u32 word, null-terminated).
fn http_write_string(vm: &mut Vm, addr: usize, s: &str) {
    for (i, byte) in s.bytes().enumerate() {
        if addr + i < vm.ram.len() {
            vm.ram[addr + i] = byte as u32;
        }
    }
    let null_pos = addr + s.len();
    if null_pos < vm.ram.len() {
        vm.ram[null_pos] = 0;
    }
}

/// Helper: load the http_get.asm program into a VM and return it.
fn load_http_program() -> Vm {
    let source = std::fs::read_to_string("programs/http_get.asm").expect("http_get.asm not found");
    let asm = crate::assembler::assemble(&source, 0).expect("http_get.asm failed to assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    vm
}

/// Helper: load http_get.asm, find a label, set up caller, run, return vm.
fn run_http_subroutine(label: &str, mut vm: Vm, url: Option<&str>) -> Vm {
    let source = std::fs::read_to_string("programs/http_get.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let label_addr = *asm
        .labels
        .get(label)
        .unwrap_or_else(|| panic!("label '{}' not found", label));

    if let Some(u) = url {
        http_write_string(&mut vm, 0x6000, u);
    }

    // Write caller at high address to avoid bytecode overlap
    let halt_addr = 0x5000;
    vm.ram[halt_addr] = 0x33; // CALL
    vm.ram[halt_addr + 1] = label_addr as u32;
    vm.ram[halt_addr + 2] = 0x00; // HALT
    vm.pc = halt_addr as u32;
    vm.halted = false;
    vm.regs[30] = 0xFF00; // SP

    for _ in 0..50000 {
        if !vm.step() {
            break;
        }
    }

    vm
}

// ═══════════════════════════════════════════════════════
// Assembly tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_http_get_assembles() {
    let source = std::fs::read_to_string("programs/http_get.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(result.is_ok(), "http_get.asm should assemble: {:?}", result);
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "should have substantial bytecode, got {}",
        asm.pixels.len()
    );
}

#[test]
fn test_browser_assembles() {
    let source = std::fs::read_to_string("programs/browser.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(result.is_ok(), "browser.asm should assemble: {:?}", result);
    let asm = result.unwrap();
    assert!(asm.pixels.len() > 100, "browser should have bytecode");
}

// ═══════════════════════════════════════════════════════
// URL Parsing tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_url_parse_host_only() {
    let vm = run_http_subroutine("url_parse", load_http_program(), Some("example.com"));
    assert_eq!(http_read_string(&vm, 0x6100), "example.com");
    assert_eq!(vm.ram[0x6200], 80, "port should default to 80");
    assert_eq!(http_read_string(&vm, 0x6300), "/");
}

#[test]
fn test_url_parse_host_port_path() {
    let vm = run_http_subroutine(
        "url_parse",
        load_http_program(),
        Some("example.com:8080/index.html"),
    );
    assert_eq!(http_read_string(&vm, 0x6100), "example.com");
    assert_eq!(vm.ram[0x6200], 8080);
    assert_eq!(http_read_string(&vm, 0x6300), "/index.html");
}

#[test]
fn test_url_parse_host_path_no_port() {
    let vm = run_http_subroutine("url_parse", load_http_program(), Some("example.com/page"));
    assert_eq!(http_read_string(&vm, 0x6100), "example.com");
    assert_eq!(vm.ram[0x6200], 80);
    assert_eq!(http_read_string(&vm, 0x6300), "/page");
}

#[test]
fn test_url_parse_ip_port() {
    let vm = run_http_subroutine(
        "url_parse",
        load_http_program(),
        Some("127.0.0.1:8000/test"),
    );
    assert_eq!(http_read_string(&vm, 0x6100), "127.0.0.1");
    assert_eq!(vm.ram[0x6200], 8000);
    assert_eq!(http_read_string(&vm, 0x6300), "/test");
}

#[test]
fn test_url_parse_host_only_slash() {
    let vm = run_http_subroutine("url_parse", load_http_program(), Some("example.com/"));
    assert_eq!(http_read_string(&vm, 0x6100), "example.com");
    assert_eq!(vm.ram[0x6200], 80);
    assert_eq!(http_read_string(&vm, 0x6300), "/");
}

#[test]
fn test_url_parse_port_only_no_path() {
    let vm = run_http_subroutine("url_parse", load_http_program(), Some("myhost:9090"));
    assert_eq!(http_read_string(&vm, 0x6100), "myhost");
    assert_eq!(vm.ram[0x6200], 9090);
    assert_eq!(http_read_string(&vm, 0x6300), "/");
}

// ═══════════════════════════════════════════════════════
// HTTP Request formatting tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_http_request_format() {
    let source = std::fs::read_to_string("programs/http_get.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let url_parse_addr = *asm.labels.get("url_parse").expect("url_parse");
    let send_get_addr = *asm.labels.get("http_send_get").expect("http_send_get");

    let mut vm = load_http_program();
    http_write_string(&mut vm, 0x6000, "example.com/index.html");

    // CALL url_parse; CALL http_send_get; HALT
    // Use high address to avoid conflicting with loaded bytecode
    let halt_addr = 0x5000;
    vm.ram[halt_addr] = 0x33;
    vm.ram[halt_addr + 1] = url_parse_addr as u32;
    vm.ram[halt_addr + 2] = 0x33;
    vm.ram[halt_addr + 3] = send_get_addr as u32;
    vm.ram[halt_addr + 4] = 0x00;
    vm.pc = halt_addr as u32;
    vm.halted = false;
    vm.regs[30] = 0xFF00;

    for _ in 0..50000 {
        if !vm.step() {
            break;
        }
    }

    // Read request buffer (built before SOCKSEND which fails due to no connection)
    let request = http_read_string(&vm, 0x6400);
    assert!(
        request.starts_with("GET /index.html HTTP/1.0"),
        "request should start with GET line, got: {:?}",
        &request[..request.len().min(60)]
    );
    assert!(
        request.contains("Host: example.com"),
        "request should contain Host header"
    );
    assert!(
        request.contains("\r\n\r\n"),
        "request should end with double CRLF"
    );
}

// ═══════════════════════════════════════════════════════
// Response parsing tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_http_strip_headers_200() {
    let mut vm = load_http_program();
    let response =
        "HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 12\r\n\r\nHello World!";
    http_write_string(&mut vm, 0x6800, response);
    vm.ram[0x9804] = response.len() as u32;

    let vm = run_http_subroutine("http_strip_headers", vm, None);

    assert_eq!(vm.ram[0x9808], 200);
    assert_eq!(http_read_string(&vm, 0x9000), "Hello World!");
    assert_eq!(vm.ram[0x9804], 12);
}

#[test]
fn test_http_strip_headers_404() {
    let mut vm = load_http_program();
    let response =
        "HTTP/1.0 404 Not Found\r\nContent-Type: text/html\r\n\r\n<html>Not Found</html>";
    http_write_string(&mut vm, 0x6800, response);
    vm.ram[0x9804] = response.len() as u32;

    let vm = run_http_subroutine("http_strip_headers", vm, None);

    assert_eq!(vm.ram[0x9808], 404);
    assert!(http_read_string(&vm, 0x9000).contains("Not Found"));
}

#[test]
fn test_http_strip_headers_empty_body() {
    let mut vm = load_http_program();
    let response = "HTTP/1.0 204 No Content\r\n\r\n";
    http_write_string(&mut vm, 0x6800, response);
    vm.ram[0x9804] = response.len() as u32;

    let vm = run_http_subroutine("http_strip_headers", vm, None);

    assert_eq!(vm.ram[0x9808], 204);
    assert_eq!(vm.ram[0x9000], 0);
    assert_eq!(vm.ram[0x9804], 0);
}

#[test]
fn test_http_status_code_variants() {
    let test_cases = vec![
        ("HTTP/1.0 301 Moved\r\n\r\nbody301", 301u32),
        ("HTTP/1.0 403 Forbidden\r\n\r\ndenied", 403),
        ("HTTP/1.0 500 Internal Server Error\r\n\r\nerror", 500),
    ];

    for (response, expected_status) in test_cases {
        let mut vm = load_http_program();
        http_write_string(&mut vm, 0x6800, response);
        vm.ram[0x9804] = response.len() as u32;

        let vm = run_http_subroutine("http_strip_headers", vm, None);

        assert_eq!(
            vm.ram[0x9808],
            expected_status,
            "status should be {} for {:?}",
            expected_status,
            &response[..20.min(response.len())]
        );
    }
}

// ═══════════════════════════════════════════════════════
// Full round-trip tests with mock HTTP server
// ═══════════════════════════════════════════════════════

#[test]
fn test_http_get_full_roundtrip() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();

    let body_text = "Hello from mock server!";
    let response = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        body_text.len(),
        body_text
    );
    let response_bytes = response.into_bytes();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut buf = [0u8; 4096];
            if let Ok(n) = stream.read(&mut buf) {
                let request = String::from_utf8_lossy(&buf[..n]);
                assert!(request.starts_with("GET "), "should be GET request");
                let _ = stream.write_all(&response_bytes);
            }
        }
    });

    thread::sleep(Duration::from_millis(100));

    let source = std::fs::read_to_string("programs/http_get.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let http_get_addr = *asm.labels.get("http_get").expect("http_get label");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    let url = format!("127.0.0.1:{}/", port);
    http_write_string(&mut vm, 0x6000, &url);

    let halt_addr = 0x5000;
    vm.ram[halt_addr] = 0x33;
    vm.ram[halt_addr + 1] = http_get_addr as u32;
    vm.ram[halt_addr + 2] = 0x00;
    vm.pc = halt_addr as u32;
    vm.halted = false;
    vm.regs[30] = 0xFF00;

    for _ in 0..100000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.ram[0x9808], 200, "HTTP status should be 200");
    let body = http_read_string(&vm, 0x9000);
    assert_eq!(body, body_text, "body should match server response");
}

#[test]
fn test_http_get_large_response() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();

    let body_text: String = "ABCDEFGHIJ".repeat(50); // 500 chars
    let response = format!(
        "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{}",
        body_text.len(),
        body_text
    );
    let response_bytes = response.into_bytes();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut buf = [0u8; 4096];
            if stream.read(&mut buf).is_ok() {
                let _ = stream.write_all(&response_bytes);
            }
        }
    });

    thread::sleep(Duration::from_millis(100));

    let source = std::fs::read_to_string("programs/http_get.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let http_get_addr = *asm.labels.get("http_get").unwrap();

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    let url = format!("127.0.0.1:{}/test", port);
    http_write_string(&mut vm, 0x6000, &url);

    let halt_addr = 0x5000;
    vm.ram[halt_addr] = 0x33;
    vm.ram[halt_addr + 1] = http_get_addr as u32;
    vm.ram[halt_addr + 2] = 0x00;
    vm.pc = halt_addr as u32;
    vm.halted = false;
    vm.regs[30] = 0xFF00;

    for _ in 0..200000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.ram[0x9808], 200);
    let body = http_read_string(&vm, 0x9000);
    assert!(body.len() > 0, "body should have content");
    assert!(
        body.starts_with("ABCDEFGHIJ"),
        "body content should match, got: {:?}",
        &body[..20.min(body.len())]
    );
}

#[test]
fn test_http_get_connection_refused() {
    let source = std::fs::read_to_string("programs/http_get.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let http_get_addr = *asm.labels.get("http_get").unwrap();

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    http_write_string(&mut vm, 0x6000, "127.0.0.1:19999/");

    let halt_addr = 0x5000;
    vm.ram[halt_addr] = 0x33;
    vm.ram[halt_addr + 1] = http_get_addr as u32;
    vm.ram[halt_addr + 2] = 0x00;
    vm.pc = halt_addr as u32;
    vm.halted = false;
    vm.regs[30] = 0xFF00;

    for _ in 0..50000 {
        if !vm.step() {
            break;
        }
    }

    assert_ne!(vm.regs[0], 0, "should fail with connection error");
}
