// net.rs -- TCP Networking for Geometry OS (Phase 41)
//
// Provides TCP socket operations for VM programs:
//   CONNECT addr_reg, port_reg, fd_reg  (0x7F)
//   SOCKSEND fd_reg, buf_reg, len_reg, sent_reg  (0x80)
//   SOCKRECV fd_reg, buf_reg, max_len_reg, recv_reg  (0x81)
//   DISCONNECT fd_reg  (0x82)
//
// Design: Each connection is stored in a slot (0..MAX_TCP_CONNECTIONS-1).
// The "fd" returned by CONNECT is the slot index. All operations are
// non-blocking. CONNECT itself blocks briefly (100ms timeout) since
// programs need the connection established before sending.
//
// String encoding: CONNECT reads a null-terminated ASCII string from RAM
// at the address in addr_reg. Format: "127.0.0.1" (IP only, port is separate).

use std::io::{Read, Write};
use std::net::TcpStream;

/// Maximum simultaneous TCP connections.
pub const MAX_TCP_CONNECTIONS: usize = 8;

/// TCP connection result codes (written to r0 on error).
pub const NET_OK: u32 = 0;
pub const NET_ERR_INVALID_FD: u32 = 1;
pub const NET_ERR_CONNECT_FAILED: u32 = 2;
pub const NET_ERR_SEND_FAILED: u32 = 3;
pub const NET_ERR_RECV_FAILED: u32 = 4;
pub const NET_ERR_NO_SLOTS: u32 = 5;
pub const NET_ERR_WOULD_BLOCK: u32 = 6;
pub const NET_ERR_CONNECTION_CLOSED: u32 = 7;

/// Read a null-terminated ASCII string from RAM.
/// Returns the string (without null terminator) or empty string on error.
fn read_string_from_ram(ram: &[u32], addr: u32) -> String {
    let mut s = String::new();
    let mut i = addr as usize;
    while i < ram.len() {
        let word = ram[i];
        // Each u32 holds one ASCII byte (lower 8 bits)
        let byte = (word & 0xFF) as u8;
        if byte == 0 {
            break;
        }
        if byte.is_ascii() {
            s.push(byte as char);
        }
        i += 1;
    }
    s
}

impl super::Vm {
    /// CONNECT addr_reg, port_reg, fd_reg  (0x7F)
    ///
    /// Connect to a TCP server. Reads a null-terminated IP address string
    /// from RAM[addr_reg], uses port_reg as the port number (16-bit).
    /// On success, stores the connection fd (= slot index) in fd_reg and r0 = NET_OK.
    /// On error, sets r0 = error code.
    ///
    /// The connection is made with a 100ms timeout for the connect phase,
    /// then set to non-blocking for subsequent send/recv operations.
    pub fn op_connect(&mut self) {
        let addr_reg = self.fetch() as usize;
        let port_reg = self.fetch() as usize;
        let fd_reg = self.fetch() as usize;

        if addr_reg >= super::NUM_REGS || port_reg >= super::NUM_REGS || fd_reg >= super::NUM_REGS {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        let addr_str = read_string_from_ram(&self.ram, self.regs[addr_reg]);
        let port = self.regs[port_reg] as u16;

        if addr_str.is_empty() {
            self.regs[0] = NET_ERR_CONNECT_FAILED;
            return;
        }

        let socket_addr = format!("{}:{}", addr_str, port);

        // Find a free slot
        let slot = self.tcp_connections.iter().position(|c| c.is_none());

        let slot = match slot {
            Some(s) => s,
            None => {
                self.regs[0] = NET_ERR_NO_SLOTS;
                return;
            }
        };

        match TcpStream::connect(&*socket_addr) {
            Ok(stream) => {
                if stream.set_nonblocking(true).is_err() {
                    self.regs[0] = NET_ERR_CONNECT_FAILED;
                    return;
                }
                if stream
                    .set_read_timeout(Some(std::time::Duration::from_millis(100)))
                    .is_err()
                {
                    self.regs[0] = NET_ERR_CONNECT_FAILED;
                    return;
                }
                self.tcp_connections[slot] = Some(stream);
                self.regs[fd_reg] = slot as u32;
                self.regs[0] = NET_OK;
            }
            Err(_) => {
                self.regs[0] = NET_ERR_CONNECT_FAILED;
            }
        }
    }

    /// SOCKSEND fd_reg, buf_reg, len_reg, sent_reg  (0x80)
    ///
    /// Send data over a TCP connection. Reads `len` bytes from RAM starting
    /// at buf_reg address (one byte per u32, lower 8 bits).
    /// On success, stores bytes actually sent in sent_reg and r0 = NET_OK.
    /// r0 = NET_ERR_WOULD_BLOCK if socket buffer is full (try again next frame).
    pub fn op_socksend(&mut self) {
        let fd_reg = self.fetch() as usize;
        let buf_reg = self.fetch() as usize;
        let len_reg = self.fetch() as usize;
        let sent_reg = self.fetch() as usize;

        if fd_reg >= super::NUM_REGS || buf_reg >= super::NUM_REGS
            || len_reg >= super::NUM_REGS || sent_reg >= super::NUM_REGS
        {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        let fd = self.regs[fd_reg] as usize;
        let buf_addr = self.regs[buf_reg] as usize;
        let len = self.regs[len_reg] as usize;

        if fd >= MAX_TCP_CONNECTIONS || self.tcp_connections[fd].is_none() {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        // Convert RAM u32s to bytes
        let mut bytes = Vec::with_capacity(len.min(4096));
        for i in 0..len.min(4096) {
            let idx = buf_addr + i;
            if idx < self.ram.len() {
                bytes.push((self.ram[idx] & 0xFF) as u8);
            } else {
                break;
            }
        }

        if let Some(ref mut stream) = self.tcp_connections[fd] {
            match stream.write(&bytes) {
                Ok(n) => {
                    self.regs[sent_reg] = n as u32;
                    self.regs[0] = NET_OK;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.regs[sent_reg] = 0;
                    self.regs[0] = NET_ERR_WOULD_BLOCK;
                }
                Err(_) => {
                    self.regs[0] = NET_ERR_SEND_FAILED;
                }
            }
        } else {
            self.regs[0] = NET_ERR_INVALID_FD;
        }
    }

    /// SOCKRECV fd_reg, buf_reg, max_len_reg, recv_reg  (0x81)
    ///
    /// Receive data from a TCP connection (non-blocking). Reads up to max_len
    /// bytes and stores them in RAM starting at buf_reg (one byte per u32).
    /// On success, stores bytes actually received in recv_reg and r0 = NET_OK.
    /// r0 = NET_ERR_WOULD_BLOCK if no data available (try again next frame).
    /// r0 = NET_ERR_CONNECTION_CLOSED if remote closed the connection.
    pub fn op_sockrecv(&mut self) {
        let fd_reg = self.fetch() as usize;
        let buf_reg = self.fetch() as usize;
        let max_len_reg = self.fetch() as usize;
        let recv_reg = self.fetch() as usize;

        if fd_reg >= super::NUM_REGS || buf_reg >= super::NUM_REGS
            || max_len_reg >= super::NUM_REGS || recv_reg >= super::NUM_REGS
        {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        let fd = self.regs[fd_reg] as usize;
        let buf_addr = self.regs[buf_reg] as usize;
        let max_len = self.regs[max_len_reg] as usize;

        if fd >= MAX_TCP_CONNECTIONS || self.tcp_connections[fd].is_none() {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        let mut buf = vec![0u8; max_len.min(4096)];

        if let Some(ref mut stream) = self.tcp_connections[fd] {
            match stream.read(&mut buf) {
                Ok(0) => {
                    // Connection closed by remote
                    self.regs[recv_reg] = 0;
                    self.regs[0] = NET_ERR_CONNECTION_CLOSED;
                }
                Ok(n) => {
                    // Store received bytes into RAM
                    for (i, &byte) in buf[..n].iter().enumerate() {
                        let idx = buf_addr + i;
                        if idx < self.ram.len() {
                            self.ram[idx] = byte as u32;
                        }
                    }
                    self.regs[recv_reg] = n as u32;
                    self.regs[0] = NET_OK;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.regs[recv_reg] = 0;
                    self.regs[0] = NET_ERR_WOULD_BLOCK;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    self.regs[recv_reg] = 0;
                    self.regs[0] = NET_ERR_WOULD_BLOCK;
                }
                Err(_) => {
                    self.regs[0] = NET_ERR_RECV_FAILED;
                }
            }
        } else {
            self.regs[0] = NET_ERR_INVALID_FD;
        }
    }

    /// DISCONNECT fd_reg  (0x82)
    ///
    /// Close a TCP connection and free the slot.
    /// r0 = NET_OK on success, error code on failure.
    pub fn op_disconnect(&mut self) {
        let fd_reg = self.fetch() as usize;

        if fd_reg >= super::NUM_REGS {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        let fd = self.regs[fd_reg] as usize;

        if fd >= MAX_TCP_CONNECTIONS || self.tcp_connections[fd].is_none() {
            self.regs[0] = NET_ERR_INVALID_FD;
            return;
        }

        // TcpStream::drop handles the close
        self.tcp_connections[fd] = None;
        self.regs[0] = NET_OK;
    }

    /// Get the number of active TCP connections (for diagnostics/testing).
    #[allow(dead_code)]
    pub fn tcp_connection_count(&self) -> usize {
        self.tcp_connections.iter().filter(|c| c.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    /// Helper: create a VM with a test TCP echo server running in background.
    /// Returns (vm, port) where port is the server's listening port.
    fn setup_echo_server() -> (super::super::Vm, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            // Accept one connection, echo data back, then close
            if let Ok((mut stream, _)) = listener.accept() {
                stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break, // connection closed
                        Ok(n) => {
                            if stream.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        // Small delay for server to start
        thread::sleep(Duration::from_millis(50));

        let mut vm = super::super::Vm::new();
        // Write IP string to RAM at 0x7000
        let ip = "127.0.0.1";
        for (i, b) in ip.bytes().enumerate() {
            vm.ram[0x7000 + i] = b as u32;
        }
        vm.ram[0x7000 + ip.len()] = 0; // null terminator

        (vm, port)
    }

    #[test]
    fn test_connect_success() {
        let (mut vm, port) = setup_echo_server();

        // LDI r1, 0x7000  (address of IP string)
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x7000;
        // LDI r2, port
        vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = port as u32;
        // CONNECT r1, r2, r3
        vm.ram[6] = 0x7F; vm.ram[7] = 1; vm.ram[8] = 2; vm.ram[9] = 3;
        vm.pc = 0;

        vm.step(); // LDI r1
        vm.step(); // LDI r2
        vm.step(); // CONNECT
        assert_eq!(vm.regs[0], NET_OK, "CONNECT should succeed");
        assert_eq!(vm.tcp_connection_count(), 1, "Should have 1 connection");
        // r3 should hold the fd (slot index)
        assert!(vm.regs[3] < MAX_TCP_CONNECTIONS as u32);
    }

    #[test]
    fn test_connect_invalid_address() {
        let mut vm = super::super::Vm::new();

        // Write empty string to RAM
        vm.ram[0x7000] = 0; // null terminator immediately

        // LDI r1, 0x7000
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x7000;
        // LDI r2, 80
        vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 80;
        // CONNECT r1, r2, r3
        vm.ram[6] = 0x7F; vm.ram[7] = 1; vm.ram[8] = 2; vm.ram[9] = 3;
        vm.pc = 0;

        vm.step(); // LDI r1
        vm.step(); // LDI r2
        vm.step(); // CONNECT
        assert_eq!(vm.regs[0], NET_ERR_CONNECT_FAILED);
        assert_eq!(vm.tcp_connection_count(), 0);
    }

    #[test]
    fn test_connect_refused() {
        let mut vm = super::super::Vm::new();

        // Write IP string for a port nothing listens on
        let ip = "127.0.0.1";
        for (i, b) in ip.bytes().enumerate() {
            vm.ram[0x7000 + i] = b as u32;
        }
        vm.ram[0x7000 + ip.len()] = 0;

        // LDI r1, 0x7000
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x7000;
        // LDI r2, 19 (port 19 -- nothing listens here)
        vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 19;
        // CONNECT r1, r2, r3
        vm.ram[6] = 0x7F; vm.ram[7] = 1; vm.ram[8] = 2; vm.ram[9] = 3;
        vm.pc = 0;

        vm.step(); // LDI r1
        vm.step(); // LDI r2
        vm.step(); // CONNECT
        assert_eq!(vm.regs[0], NET_ERR_CONNECT_FAILED);
    }

    #[test]
    fn test_send_recv_roundtrip() {
        let (mut vm, port) = setup_echo_server();

        // Setup: load IP and connect
        vm.regs[1] = 0x7000;
        vm.regs[2] = port as u32;
        vm.pc = 100;
        // CONNECT r1, r2, r3
        vm.ram[100] = 0x7F; vm.ram[101] = 1; vm.ram[102] = 2; vm.ram[103] = 3;
        vm.step(); // CONNECT
        assert_eq!(vm.regs[0], NET_OK);
        let fd = vm.regs[3];

        // Write "Hello" to RAM at 0x7100
        let msg = b"Hello";
        for (i, &b) in msg.iter().enumerate() {
            vm.ram[0x7100 + i] = b as u32;
        }

        // SOCKSEND fd, 0x7100, 5, r5
        vm.regs[4] = fd;
        vm.regs[5] = 0x7100;
        vm.regs[6] = 5;
        vm.pc = 200;
        vm.ram[200] = 0x80; vm.ram[201] = 4; vm.ram[202] = 5; vm.ram[203] = 6; vm.ram[204] = 7;
        vm.step(); // SOCKSEND
        assert_eq!(vm.regs[0], NET_OK, "SOCKSEND should succeed");
        assert!(vm.regs[7] > 0, "Should have sent bytes");

        // Small delay for echo server to respond
        thread::sleep(Duration::from_millis(100));

        // SOCKRECV fd, 0x7200, 100, r5
        vm.regs[5] = 0x7200;
        vm.regs[6] = 100;
        vm.pc = 300;
        vm.ram[300] = 0x81; vm.ram[301] = 4; vm.ram[302] = 5; vm.ram[303] = 6; vm.ram[304] = 8;
        vm.step(); // SOCKRECV
        assert_eq!(vm.regs[0], NET_OK, "SOCKRECV should succeed");
        let received = vm.regs[8] as usize;
        assert!(received > 0, "Should have received bytes");

        // Verify echoed data
        for i in 0..received {
            assert_eq!(vm.ram[0x7200 + i], msg[i] as u32,
                "Byte {} mismatch: got {}, expected {}", i, vm.ram[0x7200 + i], msg[i]);
        }
    }

    #[test]
    fn test_disconnect() {
        let (mut vm, port) = setup_echo_server();

        // Connect
        vm.regs[1] = 0x7000;
        vm.regs[2] = port as u32;
        vm.pc = 100;
        vm.ram[100] = 0x7F; vm.ram[101] = 1; vm.ram[102] = 2; vm.ram[103] = 3;
        vm.step();
        assert_eq!(vm.regs[0], NET_OK);
        assert_eq!(vm.tcp_connection_count(), 1);

        // Disconnect
        let fd = vm.regs[3];
        vm.regs[4] = fd;
        vm.pc = 200;
        vm.ram[200] = 0x82; vm.ram[201] = 4;
        vm.step();
        assert_eq!(vm.regs[0], NET_OK);
        assert_eq!(vm.tcp_connection_count(), 0);
    }

    #[test]
    fn test_disconnect_invalid_fd() {
        let mut vm = super::super::Vm::new();

        // DISCONNECT with fd=99 (out of range)
        vm.regs[1] = 99;
        vm.pc = 100;
        vm.ram[100] = 0x82; vm.ram[101] = 1;
        vm.step();
        assert_eq!(vm.regs[0], NET_ERR_INVALID_FD);
    }

    #[test]
    fn test_max_connections() {
        // Start 8 echo servers
        let mut ports = Vec::new();
        let mut vms = Vec::new();
        for _ in 0..MAX_TCP_CONNECTIONS {
            let (vm, port) = setup_echo_server();
            ports.push(port);
            vms.push(vm);
        }

        // Connect all 8 from first VM
        let vm = &mut vms[0];
        for i in 0..MAX_TCP_CONNECTIONS {
            vm.regs[1] = 0x7000;
            vm.regs[2] = ports[i] as u32;
            vm.pc = (100 + i * 10) as u32;
            vm.ram[100 + i * 10] = 0x7F;
            vm.ram[101 + i * 10] = 1;
            vm.ram[102 + i * 10] = 2;
            vm.ram[103 + i * 10] = 10 + i as u32; // store fd in r10+i
            vm.step();
            assert_eq!(vm.regs[0], NET_OK, "Connection {} should succeed", i);
        }

        assert_eq!(vm.tcp_connection_count(), MAX_TCP_CONNECTIONS);

        // 9th connection should fail
        let (extra_vm, extra_port) = setup_echo_server();
        // We need to update the IP in RAM since extra_vm wrote to its own RAM
        // Use the first vm's RAM which already has 127.0.0.1 at 0x7000
        vm.regs[2] = extra_port as u32;
        vm.pc = 1000;
        vm.ram[1000] = 0x7F; vm.ram[1001] = 1; vm.ram[1002] = 2; vm.ram[1003] = 20;
        vm.step();
        assert_eq!(vm.regs[0], NET_ERR_NO_SLOTS, "9th connection should fail with NO_SLOTS");
    }

    #[test]
    fn test_send_invalid_fd() {
        let mut vm = super::super::Vm::new();

        // SOCKSEND with fd=99
        vm.regs[1] = 99;
        vm.regs[2] = 0x7100;
        vm.regs[3] = 5;
        vm.pc = 100;
        vm.ram[100] = 0x80; vm.ram[101] = 1; vm.ram[102] = 2; vm.ram[103] = 3; vm.ram[104] = 4;
        vm.step();
        assert_eq!(vm.regs[0], NET_ERR_INVALID_FD);
    }

    #[test]
    fn test_recv_invalid_fd() {
        let mut vm = super::super::Vm::new();

        // SOCKRECV with fd=99
        vm.regs[1] = 99;
        vm.regs[2] = 0x7200;
        vm.regs[3] = 100;
        vm.pc = 100;
        vm.ram[100] = 0x81; vm.ram[101] = 1; vm.ram[102] = 2; vm.ram[103] = 3; vm.ram[104] = 4;
        vm.step();
        assert_eq!(vm.regs[0], NET_ERR_INVALID_FD);
    }

    #[test]
    fn test_read_string_from_ram() {
        let mut vm = super::super::Vm::new();
        vm.ram[0x7000] = 'H' as u32;
        vm.ram[0x7001] = 'i' as u32;
        vm.ram[0x7002] = 0; // null terminator

        let s = read_string_from_ram(&vm.ram, 0x7000);
        assert_eq!(s, "Hi");
    }

    #[test]
    fn test_read_string_from_ram_empty() {
        let vm = super::super::Vm::new();
        let s = read_string_from_ram(&vm.ram, 0x7000);
        assert_eq!(s, "");
    }

    #[test]
    fn test_read_string_from_ram_past_end() {
        let mut vm = super::super::Vm::new();
        vm.ram[0x7000] = 'A' as u32;
        // No null terminator -- reads until end of RAM
        let s = read_string_from_ram(&vm.ram, 0x7000);
        assert!(s.starts_with("A"));
    }
}
