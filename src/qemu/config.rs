// qemu/config.rs -- QEMU configuration parsing and command building
//
// Parses a config string like "arch=riscv64 kernel=linux.img ram=256M"
// and builds the corresponding QEMU Command.

use std::process::{Command, Stdio};

// ── Architecture mapping ─────────────────────────────────────────

/// Maps a config arch string to (qemu_binary, machine_flag).
/// Returns None for unknown architectures.
pub fn arch_to_qemu(arch: &str) -> Option<(&'static str, Option<&'static str>)> {
    match arch {
        "riscv64" => Some(("qemu-system-riscv64", Some("-machine virt"))),
        "riscv32" => Some(("qemu-system-riscv32", Some("-machine virt"))),
        "x86_64" => Some(("qemu-system-x86_64", None)),
        "aarch64" => Some(("qemu-system-aarch64", Some("-machine virt"))),
        "arm" => Some(("qemu-system-arm", Some("-machine virt"))),
        "mipsel" => Some(("qemu-system-mipsel", Some("-machine malta"))),
        "mips" => Some(("qemu-system-mips", Some("-machine malta"))),
        "ppc" => Some(("qemu-system-ppc", None)),
        "ppc64" => Some(("qemu-system-ppc64", None)),
        "s390x" => Some(("qemu-system-s390x", None)),
        _ => None,
    }
}

// ── QEMU Config ──────────────────────────────────────────────────

/// Parsed QEMU configuration from a config string.
/// Format: "arch=riscv64 kernel=linux.img ram=256M disk=rootfs.ext4"
#[derive(Debug, Clone, Default)]
pub struct QemuConfig {
    pub arch: String,
    pub kernel: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
    pub bios: Option<String>,
    pub initrd: Option<String>,
    pub append: Option<String>,
    pub net: Option<String>,
    pub extra_args: Vec<String>,
}

impl QemuConfig {
    /// Parse a config string into a QemuConfig.
    /// Format: "key=value key=value ..."
    /// Unknown keys are stored in extra_args.
    pub fn parse(config_str: &str) -> Result<QemuConfig, String> {
        let mut cfg = QemuConfig::default();
        for token in config_str.split_whitespace() {
            let parts: Vec<&str> = token.splitn(2, '=').collect();
            if parts.len() != 2 || parts[1].is_empty() {
                return Err(format!(
                    "invalid config token: '{}' (expected key=value)",
                    token
                ));
            }
            let key = parts[0].to_lowercase();
            let val = parts[1].to_string();
            match key.as_str() {
                "arch" => cfg.arch = val,
                "kernel" => cfg.kernel = Some(val),
                "ram" | "memory" | "m" => cfg.ram = Some(val),
                "disk" | "drive" | "hda" => cfg.disk = Some(val),
                "bios" => cfg.bios = Some(val),
                "initrd" => cfg.initrd = Some(val),
                "append" | "cmdline" => cfg.append = Some(val),
                "net" | "nic" => cfg.net = Some(val),
                _ => cfg.extra_args.push(token.to_string()),
            }
        }
        if cfg.arch.is_empty() {
            return Err("config must specify arch=<architecture>".into());
        }
        Ok(cfg)
    }

    /// Build the QEMU command from this config.
    pub fn build_command(&self) -> Result<Command, String> {
        let (binary, machine) = arch_to_qemu(&self.arch)
            .ok_or_else(|| format!("unknown architecture: '{}'", self.arch))?;

        let mut cmd = Command::new(binary);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Always use nographic serial mode
        cmd.arg("-nographic");
        cmd.arg("-serial").arg("mon:stdio");

        // Machine type
        if let Some(m) = machine {
            let parts: Vec<&str> = m.split_whitespace().collect();
            for p in parts {
                cmd.arg(p);
            }
        }

        // RAM
        if let Some(ref ram) = self.ram {
            cmd.arg("-m").arg(ram);
        }

        // Kernel
        if let Some(ref kernel) = self.kernel {
            cmd.arg("-kernel").arg(kernel);
        }

        // BIOS
        if let Some(ref bios) = self.bios {
            cmd.arg("-bios").arg(bios);
        }

        // Initrd
        if let Some(ref initrd) = self.initrd {
            cmd.arg("-initrd").arg(initrd);
        }

        // Kernel command line
        if let Some(ref append) = self.append {
            cmd.arg("-append").arg(append);
        }

        // Disk
        if let Some(ref disk) = self.disk {
            cmd.args([
                "-drive",
                &format!("file={},format=raw,if=virtio", disk),
            ]);
        }

        // Network
        if let Some(ref net) = self.net {
            if net == "none" {
                cmd.arg("-net").arg("none");
            } else {
                cmd.args(["-netdev", &format!("user,id=net0,{}", net)]);
                cmd.args(["-device", "virtio-net-device,netdev=net0"]);
            }
        }

        // Extra args
        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parse_minimal() {
        let cfg = QemuConfig::parse("arch=riscv64").expect("config parse should succeed");
        assert_eq!(cfg.arch, "riscv64");
        assert!(cfg.kernel.is_none());
        assert!(cfg.ram.is_none());
    }

    #[test]
    fn test_config_parse_full() {
        let cfg = QemuConfig::parse(
            "arch=x86_64 kernel=bzImage ram=512M disk=rootfs.ext4",
        )
        .expect("operation should succeed");
        assert_eq!(cfg.arch, "x86_64");
        assert_eq!(cfg.kernel.as_deref(), Some("bzImage"));
        assert_eq!(cfg.ram.as_deref(), Some("512M"));
        assert_eq!(cfg.disk.as_deref(), Some("rootfs.ext4"));
    }

    #[test]
    fn test_config_parse_memory_alias() {
        let cfg = QemuConfig::parse("arch=aarch64 memory=1G").expect("config parse should succeed");
        assert_eq!(cfg.ram.as_deref(), Some("1G"));
    }

    #[test]
    fn test_config_parse_initrd_append() {
        let cfg = QemuConfig::parse(
            "arch=riscv64 kernel=Image initrd=initramfs.cpio.gz append=root=/dev/vda",
        )
        .expect("operation should succeed");
        assert_eq!(cfg.initrd.as_deref(), Some("initramfs.cpio.gz"));
        assert_eq!(cfg.append.as_deref(), Some("root=/dev/vda"));
    }

    #[test]
    fn test_config_parse_no_arch() {
        let result = QemuConfig::parse("kernel=linux.img");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("arch"));
    }

    #[test]
    fn test_config_parse_invalid_token() {
        let result = QemuConfig::parse("arch=riscv64 nogoodvalue");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_parse_extra_args() {
        let cfg = QemuConfig::parse("arch=riscv64 custom=foo").expect("config parse should succeed");
        assert_eq!(cfg.extra_args, vec!["custom=foo"]);
    }

    #[test]
    fn test_arch_mapping_riscv64() {
        let (bin, machine) = arch_to_qemu("riscv64").expect("arch should be supported");
        assert_eq!(bin, "qemu-system-riscv64");
        assert_eq!(machine, Some("-machine virt"));
    }

    #[test]
    fn test_arch_mapping_x86_64() {
        let (bin, machine) = arch_to_qemu("x86_64").expect("arch should be supported");
        assert_eq!(bin, "qemu-system-x86_64");
        assert!(machine.is_none());
    }

    #[test]
    fn test_arch_mapping_aarch64() {
        let (bin, machine) = arch_to_qemu("aarch64").expect("arch should be supported");
        assert_eq!(bin, "qemu-system-aarch64");
        assert_eq!(machine, Some("-machine virt"));
    }

    #[test]
    fn test_arch_mapping_mipsel() {
        let (bin, machine) = arch_to_qemu("mipsel").expect("arch should be supported");
        assert_eq!(bin, "qemu-system-mipsel");
        assert_eq!(machine, Some("-machine malta"));
    }

    #[test]
    fn test_arch_mapping_unknown() {
        assert!(arch_to_qemu("nonexistent").is_none());
    }

    #[test]
    fn test_build_command_riscv64() {
        let cfg =
            QemuConfig::parse("arch=riscv64 kernel=Image ram=256M").expect("config parse should succeed");
        let cmd = cfg.build_command().expect("command build should succeed");
        let args: Vec<String> =
            cmd.get_args().map(|s| s.to_string_lossy().into()).collect();
        assert!(args.contains(&"-nographic".to_string()));
        assert!(args.contains(&"mon:stdio".to_string()));
        assert!(args.contains(&"-machine".to_string()));
        assert!(args.contains(&"virt".to_string()));
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"256M".to_string()));
        assert!(args.contains(&"-kernel".to_string()));
        assert!(args.contains(&"Image".to_string()));
    }

    #[test]
    fn test_build_command_with_disk() {
        let cfg = QemuConfig::parse("arch=riscv64 disk=rootfs.ext4").expect("config parse should succeed");
        let cmd = cfg.build_command().expect("command build should succeed");
        let args: Vec<String> =
            cmd.get_args().map(|s| s.to_string_lossy().into()).collect();
        let drive_arg = args.iter().find(|a| a.contains("rootfs.ext4")).expect("expected element should exist");
        assert!(drive_arg.contains("format=raw"));
        assert!(drive_arg.contains("if=virtio"));
    }

    #[test]
    fn test_build_command_with_net_none() {
        let cfg = QemuConfig::parse("arch=riscv64 net=none").expect("config parse should succeed");
        let cmd = cfg.build_command().expect("command build should succeed");
        let args: Vec<String> =
            cmd.get_args().map(|s| s.to_string_lossy().into()).collect();
        assert!(args.contains(&"none".to_string()));
    }

    #[test]
    fn test_build_command_unknown_arch() {
        let cfg = QemuConfig::parse("arch=invalid_cpu").expect("config parse should succeed");
        assert!(cfg.build_command().is_err());
    }
}
