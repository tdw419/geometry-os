// episode_log.rs -- Episodic Memory for Geometry OS
//
// Persists diagnostic context across Hermes sessions so the LLM agent can
// recall past program runs, black-screen incidents, and what fixed them.
//
// Storage: JSONL (one JSON object per line) in the project's episodic_memory/ dir.
// No external dependencies -- hand-rolled JSON to match the project's zero-dep philosophy.
//
// Memory taxonomy (from AI-Native OS research):
//   Working   = context window (already done)
//   Episodic  = THIS FILE (run logs with outcomes)
//   Semantic  = RAG knowledge base (already done)
//   Procedural = skills system (already done)

use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// A single diagnostic episode from a program run.
#[derive(Debug, Clone)]
pub struct Episode {
    /// ISO 8601 timestamp of when the run happened
    pub timestamp: String,
    /// Program filename (e.g. "mandelbrot.asm")
    pub program: String,
    /// Total instructions executed
    pub total_ops: u64,
    /// Top opcodes as (hex_opcode, mnemonic, count, percentage)
    pub top_opcodes: Vec<(u8, String, u64, f64)>,
    /// Screen state after execution
    pub screen_state: ScreenState,
    /// PC value when execution ended
    pub final_pc: u32,
    /// Whether the VM halted normally
    pub halted: bool,
    /// Human-readable fix or note (set after diagnosis)
    pub fix: Option<String>,
    /// Outcome classification
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Program ran and produced visible output
    Success,
    /// Program ran but screen was all black
    BlackScreen,
    /// Program failed to assemble
    AssembleError,
    /// Program hit max steps without halting
    Timeout,
    /// Unknown / not yet classified
    Unknown,
}

impl Outcome {
    fn as_str(&self) -> &'static str {
        match self {
            Outcome::Success => "success",
            Outcome::BlackScreen => "black_screen",
            Outcome::AssembleError => "assemble_error",
            Outcome::Timeout => "timeout",
            Outcome::Unknown => "unknown",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "success" => Outcome::Success,
            "black_screen" => Outcome::BlackScreen,
            "assemble_error" => Outcome::AssembleError,
            "timeout" => Outcome::Timeout,
            _ => Outcome::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScreenState {
    /// Non-zero pixels count
    pub non_black_pixels: usize,
    /// Total pixels in framebuffer
    pub total_pixels: usize,
    /// Percentage of screen drawn (0.0 - 100.0)
    pub drawn_pct: f64,
}

impl ScreenState {
    pub fn is_black(&self) -> bool {
        self.non_black_pixels == 0
    }
}

/// Get the episode log directory, creating it if needed.
fn episode_dir() -> PathBuf {
    let root = get_project_root();
    let dir = root.join("episodic_memory");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn get_project_root() -> PathBuf {
    // Walk up from CWD to find Cargo.toml
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("Cargo.toml").exists() {
            return dir;
        }
        if !dir.pop() {
            return PathBuf::from(".");
        }
    }
}

/// Get the JSONL file path for a given program name.
fn episode_file(program: &str) -> PathBuf {
    // Sanitize filename: keep only alphanumeric, dash, underscore, dot
    let safe_name: String = program
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    let stem = safe_name.trim_end_matches(".asm");
    episode_dir().join(format!("{}.jsonl", stem))
}

/// Append an episode to the log. Creates the file if it doesn't exist.
pub fn log_episode(episode: &Episode) {
    let path = episode_file(&episode.program);
    let json = episode_to_json(episode);
    match fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(e) = writeln!(file, "{}", json) {
                eprintln!("[episode_log] write error: {}", e);
            }
        }
        Err(e) => eprintln!("[episode_log] open error for {}: {}", path.display(), e),
    }
}

/// Query past episodes for a given program. Returns most recent first, limited to `limit`.
pub fn query_episodes(program: &str, limit: usize) -> Vec<Episode> {
    let path = episode_file(program);
    if !path.exists() {
        return Vec::new();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut episodes: Vec<Episode> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(json_to_episode)
        .collect();
    // Most recent last in file, reverse for most recent first
    episodes.reverse();
    episodes.truncate(limit);
    episodes
}

/// Format past episodes as a context string for the Hermes prompt.
/// Shows the last N episodes for the current program, highlighting patterns.
pub fn format_episode_context(program: &str, limit: usize) -> String {
    let episodes = query_episodes(program, limit);
    if episodes.is_empty() {
        return String::new();
    }

    let mut ctx = String::new();
    ctx.push_str(&format!(
        "\n## Episodic Memory: {} ({} past runs)\n",
        program,
        episodes.len()
    ));

    // Count outcomes
    let black_screens = episodes
        .iter()
        .filter(|e| e.outcome == Outcome::BlackScreen)
        .count();
    let _successes = episodes
        .iter()
        .filter(|e| e.outcome == Outcome::Success)
        .count();

    if black_screens > 0 {
        ctx.push_str(&format!(
            "  WARNING: {} of {} past runs resulted in BLACK SCREEN.\n",
            black_screens,
            episodes.len()
        ));
    }

    // Show recent episodes (oldest first for readability)
    let display: Vec<&Episode> = episodes.iter().rev().take(5).collect();
    for ep in &display {
        ctx.push_str(&format!(
            "  [{}] {} ops, PC={:04X}, screen {:.1}% drawn ({}){}\n",
            ep.timestamp,
            ep.total_ops,
            ep.final_pc,
            ep.screen_state.drawn_pct,
            ep.outcome.as_str(),
            ep.fix
                .as_ref()
                .map(|f| format!(" -- fix: {}", f))
                .unwrap_or_default(),
        ));
    }

    // Pattern detection: if recent runs all had same issue, flag it
    if display.len() >= 3 {
        let recent_black = display
            .iter()
            .rev()
            .take(3)
            .filter(|e| e.outcome == Outcome::BlackScreen)
            .count();
        if recent_black >= 3 {
            ctx.push_str(
                "  PATTERN: Last 3 runs all black-screened. This is a persistent issue.\n",
            );
        }
    }

    // Show top fix if any
    let fixes: Vec<&str> = display.iter().filter_map(|e| e.fix.as_deref()).collect();
    if !fixes.is_empty() {
        ctx.push_str("  Past fixes that worked:\n");
        for fix in fixes.iter().rev() {
            ctx.push_str(&format!("    - {}\n", fix));
        }
    }

    ctx
}

// -- JSON serialization (hand-rolled, no serde) --

fn episode_to_json(ep: &Episode) -> String {
    let mut json = String::with_capacity(512);
    json.push('{');
    json.push_str(&format!("\"ts\":\"{}\"", escape_json(&ep.timestamp)));
    json.push_str(&format!(",\"prog\":\"{}\"", escape_json(&ep.program)));
    json.push_str(&format!(",\"ops\":{}", ep.total_ops));
    json.push_str(&format!(",\"pc\":{}", ep.final_pc));
    json.push_str(&format!(",\"halted\":{}", ep.halted));
    json.push_str(&format!(
        ",\"screen\":{{\"non_zero\":{},\"total\":{},\"pct\":{:.1}}}",
        ep.screen_state.non_black_pixels, ep.screen_state.total_pixels, ep.screen_state.drawn_pct
    ));
    // Top opcodes as compact array: [[opcode, count, pct], ...]
    json.push_str(",\"top_ops\":[");
    for (i, (op, _name, count, pct)) in ep.top_opcodes.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("[{},\"{}\",{},{:.1}]", op, _name, count, pct));
    }
    json.push(']');
    if let Some(ref fix) = ep.fix {
        json.push_str(&format!(",\"fix\":\"{}\"", escape_json(fix)));
    }
    json.push_str(&format!(",\"outcome\":\"{}\"", ep.outcome.as_str()));
    json.push('}');
    json
}

/// Parse the top_ops JSON array: `"top_ops":[[opcode,"MNEMONIC",count,pct],...]`
/// Returns empty vec if parsing fails (non-fatal -- episodes are still usable without opcodes).
fn parse_top_ops_array(json: &str) -> Vec<(u8, String, u64, f64)> {
    // Find the "top_ops" key, then extract the array content between [ and ]
    let marker = "\"top_ops\":[";
    let start = match json.find(marker) {
        Some(s) => s + marker.len() - 1, // points at the opening [
        None => return Vec::new(),
    };
    // Find matching ] -- scan for bracket balance
    let bytes = json.as_bytes();
    let mut depth = 0i32;
    let mut end = start;
    for (i, &byte) in bytes[start..].iter().enumerate() {
        match byte {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    if end <= start + 1 {
        return Vec::new(); // empty array
    }
    let array_content = &json[start + 1..end]; // inside outer brackets

    // Split on ],[ to get individual entries like 0,"HALT",1,0.02
    let mut result = Vec::new();
    for entry in array_content.split("],[") {
        // Strip leading [ and trailing ] from first/last entries
        let entry = entry.trim_start_matches('[').trim_end_matches(']');
        // Parse: opcode,"NAME",count,pct
        let parts: Vec<&str> = entry.split(',').collect();
        if parts.len() < 4 {
            continue;
        }
        let opcode = match parts[0].parse::<u8>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Name is quoted: "HALT" -- strip quotes
        let name = parts[1].trim_matches('"').to_string();
        let count = match parts[2].parse::<u64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let pct = parts[3].parse::<f64>().unwrap_or(0.0);
        result.push((opcode, name, count, pct));
    }
    result
}

fn json_to_episode(line: &str) -> Option<Episode> {
    // Minimal JSON parser for our known format
    let ts = extract_json_string_val(line, "\"ts\"")?;
    let prog = extract_json_string_val(line, "\"prog\"")?;
    let total_ops = extract_json_u64(line, "\"ops\"")?;
    let final_pc = extract_json_u64(line, "\"pc\"")? as u32;
    let halted = extract_json_bool(line, "\"halted\"");
    let fix = extract_json_string_val(line, "\"fix\"");
    let outcome_str = extract_json_string_val(line, "\"outcome\"")?;
    let outcome = Outcome::from_str(&outcome_str);

    // Parse screen object
    let non_zero = extract_json_u64(line, "\"non_zero\"")? as usize;
    let total = extract_json_u64(line, "\"total\"")? as usize;
    let pct = extract_json_f64(line, "\"pct\"").unwrap_or(0.0);

    // Parse top_ops array: [[opcode,"MNEMONIC",count,pct], ...]
    let top_opcodes = parse_top_ops_array(line);

    Some(Episode {
        timestamp: ts,
        program: prog,
        total_ops,
        top_opcodes,
        final_pc,
        halted,
        screen_state: ScreenState {
            non_black_pixels: non_zero,
            total_pixels: total,
            drawn_pct: pct,
        },
        fix,
        outcome,
    })
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn extract_json_string_val(json: &str, key: &str) -> Option<String> {
    let start = json.find(key)?;
    let after_key = &json[start + key.len()..];
    let colon = after_key.find(':')?;
    let after_colon = &after_key[colon + 1..];
    let trimmed = after_colon.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let value_start = 1;
    let mut i = value_start;
    let bytes = trimmed.as_bytes();
    let mut result = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 < bytes.len() {
                    match bytes[i + 1] {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        _ => result.push(bytes[i + 1] as char),
                    }
                    i += 2;
                } else {
                    break;
                }
            }
            b'"' => break,
            _ => {
                result.push(bytes[i] as char);
                i += 1;
            }
        }
    }
    Some(result)
}

fn extract_json_u64(json: &str, key: &str) -> Option<u64> {
    let start = json.find(key)?;
    let after_key = &json[start + key.len()..];
    let colon = after_key.find(':')?;
    let after_colon = &after_key[colon + 1..];
    let trimmed = after_colon.trim_start();
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    trimmed[..end].parse().ok()
}

fn extract_json_f64(json: &str, key: &str) -> Option<f64> {
    let start = json.find(key)?;
    let after_key = &json[start + key.len()..];
    let colon = after_key.find(':')?;
    let after_colon = &after_key[colon + 1..];
    let trimmed = after_colon.trim_start();
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != 'e' && c != 'E')
        .unwrap_or(trimmed.len());
    trimmed[..end].parse().ok()
}

fn extract_json_bool(json: &str, key: &str) -> bool {
    if let Some(start) = json.find(key) {
        let after_key = &json[start + key.len()..];
        if let Some(colon) = after_key.find(':') {
            let after_colon = &after_key[colon + 1..];
            let trimmed = after_colon.trim_start();
            return trimmed.starts_with("true");
        }
    }
    false
}

/// Helper to build an episode from raw VM state after a run.
/// Takes pre-formatted top opcodes from the caller (which has access to opcode_name).
/// Uses raw values to avoid cross-crate type mismatches (lib vs bin Vm types).
#[allow(clippy::too_many_arguments)]
pub fn build_episode_raw(
    program: &str,
    total_ops: u64,
    top_opcodes: Vec<(u8, String, u64, f64)>,
    screen_non_black: usize,
    screen_total: usize,
    final_pc: u32,
    halted: bool,
    fix: Option<String>,
) -> Episode {
    let drawn_pct = if screen_total > 0 {
        screen_non_black as f64 / screen_total as f64 * 100.0
    } else {
        0.0
    };

    let outcome = if screen_non_black == 0 && halted {
        Outcome::BlackScreen
    } else if !halted {
        Outcome::Timeout
    } else {
        Outcome::Success
    };

    let timestamp = format_timestamp();

    Episode {
        timestamp,
        program: program.to_string(),
        total_ops,
        top_opcodes,
        screen_state: ScreenState {
            non_black_pixels: screen_non_black,
            total_pixels: screen_total,
            drawn_pct,
        },
        final_pc,
        halted,
        fix,
        outcome,
    }
}

fn format_timestamp() -> String {
    // Simple timestamp without chrono dependency
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // ISO-ish format: YYYYMMDD-HHMMSS (approximate, no calendar math)
    // Just use epoch seconds for now -- Hermes can interpret it
    format!("{}-{:03}", secs / 86400 + 20000, secs % 86400)
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_json_roundtrip() {
        let episode = Episode {
            timestamp: "2026-04-19T16:04:00".to_string(),
            program: "mandelbrot.asm".to_string(),
            total_ops: 4820,
            top_opcodes: vec![
                (0x42, "FILL".to_string(), 2000, 41.5),
                (0x00, "HALT".to_string(), 1, 0.02),
            ],
            screen_state: ScreenState {
                non_black_pixels: 5000,
                total_pixels: 65536,
                drawn_pct: 7.6,
            },
            final_pc: 0x0042,
            halted: true,
            fix: Some("replaced wrong opcode 0x42 with FILL".to_string()),
            outcome: Outcome::Success,
        };

        let json = episode_to_json(&episode);
        assert!(json.contains("\"prog\":\"mandelbrot.asm\""));
        assert!(json.contains("\"ops\":4820"));
        assert!(json.contains("\"outcome\":\"success\""));
        assert!(json.contains("\"fix\":\"replaced wrong opcode"));

        let parsed = json_to_episode(&json).expect("parse failed");
        assert_eq!(parsed.program, "mandelbrot.asm");
        assert_eq!(parsed.total_ops, 4820);
        assert_eq!(parsed.final_pc, 0x0042);
        assert!(parsed.halted);
        assert_eq!(parsed.outcome, Outcome::Success);
        assert_eq!(
            parsed.fix,
            Some("replaced wrong opcode 0x42 with FILL".to_string())
        );
        assert_eq!(parsed.screen_state.non_black_pixels, 5000);
        assert_eq!(parsed.screen_state.total_pixels, 65536);
    }

    #[test]
    fn test_episode_json_without_fix() {
        let episode = Episode {
            timestamp: "2026-04-19T16:10:00".to_string(),
            program: "blank.asm".to_string(),
            total_ops: 100,
            top_opcodes: vec![],
            screen_state: ScreenState {
                non_black_pixels: 0,
                total_pixels: 65536,
                drawn_pct: 0.0,
            },
            final_pc: 0x0010,
            halted: true,
            fix: None,
            outcome: Outcome::BlackScreen,
        };

        let json = episode_to_json(&episode);
        assert!(json.contains("\"outcome\":\"black_screen\""));
        assert!(!json.contains("\"fix\":"));

        let parsed = json_to_episode(&json).expect("parse failed");
        assert_eq!(parsed.outcome, Outcome::BlackScreen);
        assert!(parsed.screen_state.is_black());
        assert!(parsed.fix.is_none());
    }

    #[test]
    fn test_json_string_escaping() {
        let val = escape_json("hello \"world\"\nline2\\path");
        assert_eq!(val, "hello \\\"world\\\"\\nline2\\\\path");
    }

    #[test]
    fn test_extract_json_u64() {
        assert_eq!(extract_json_u64("{\"ops\":4820}", "\"ops\""), Some(4820));
        assert_eq!(extract_json_u64("{\"pc\":256}", "\"pc\""), Some(256));
        assert_eq!(extract_json_u64("{\"ops\":0}", "\"ops\""), Some(0));
    }

    #[test]
    fn test_extract_json_bool() {
        assert!(extract_json_bool("{\"halted\":true}", "\"halted\""));
        assert!(!extract_json_bool("{\"halted\":false}", "\"halted\""));
    }

    #[test]
    fn test_episode_file_sanitization() {
        let path = episode_file("../../etc/passwd.asm");
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(!name.contains('/'));
        assert!(name.ends_with(".jsonl"));
        // Dots in the original name (passwd.asm) become part of "etcpasswd.asm"
        // then .asm is stripped, leaving "etcpasswd.jsonl"
    }

    #[test]
    fn test_format_episode_context_empty() {
        let ctx = format_episode_context("nonexistent.asm", 5);
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_outcome_roundtrip() {
        for outcome in [
            Outcome::Success,
            Outcome::BlackScreen,
            Outcome::AssembleError,
            Outcome::Timeout,
            Outcome::Unknown,
        ] {
            assert_eq!(Outcome::from_str(outcome.as_str()), outcome);
        }
    }

    #[test]
    fn test_parse_top_ops_array() {
        let json = r#"{"ts":"2026-04-19","prog":"test.asm","ops":100,"pc":16,"halted":true,"screen":{"non_zero":50,"total":65536,"pct":0.1},"top_ops":[[66,"FILL",80,80.0],[0,"HALT",1,1.0]],"outcome":"success"}"#;
        let ops = parse_top_ops_array(json);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0], (66, "FILL".to_string(), 80, 80.0));
        assert_eq!(ops[1], (0, "HALT".to_string(), 1, 1.0));
    }

    #[test]
    fn test_parse_top_ops_array_empty() {
        let json = r#"{"ts":"2026-04-19","prog":"test.asm","ops":0,"pc":0,"halted":true,"screen":{"non_zero":0,"total":65536,"pct":0.0},"top_ops":[],"outcome":"success"}"#;
        let ops = parse_top_ops_array(json);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_parse_top_ops_array_missing() {
        let json = r#"{"ts":"2026-04-19","prog":"test.asm","ops":0,"pc":0}"#;
        let ops = parse_top_ops_array(json);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_episode_roundtrip_preserves_top_ops() {
        let episode = Episode {
            timestamp: "2026-04-19".to_string(),
            program: "test.asm".to_string(),
            total_ops: 4820,
            top_opcodes: vec![
                (0x42, "FILL".to_string(), 2000, 41.5),
                (0x00, "HALT".to_string(), 1, 0.02),
                (0x30, "JMP".to_string(), 500, 10.4),
            ],
            screen_state: ScreenState {
                non_black_pixels: 5000,
                total_pixels: 65536,
                drawn_pct: 7.6,
            },
            final_pc: 0x0042,
            halted: true,
            fix: None,
            outcome: Outcome::Success,
        };

        let json = episode_to_json(&episode);
        let parsed = json_to_episode(&json).expect("parse failed");
        assert_eq!(parsed.top_opcodes.len(), 3);
        assert_eq!(
            parsed.top_opcodes[0],
            (0x42, "FILL".to_string(), 2000, 41.5)
        );
        assert_eq!(parsed.top_opcodes[1], (0x00, "HALT".to_string(), 1, 0.0));
        assert_eq!(parsed.top_opcodes[2], (0x30, "JMP".to_string(), 500, 10.4));
    }
}
