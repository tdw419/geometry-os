// assembler/includes.rs -- .include and .lib directive resolution

use super::AsmError;

/// Maximum include depth to prevent recursive includes.
const MAX_INCLUDE_DEPTH: usize = 8;

/// Resolve .include directives by recursively inlining file contents.
pub(super) fn resolve_includes(
    source: &str,
    lib_dir: Option<&str>,
    depth: usize,
) -> Result<String, AsmError> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(AsmError {
            line: 0,
            message: "include depth exceeded (possible circular include)".into(),
        });
    }

    let mut output = String::new();
    for (line_num, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        // .lib name -- shorthand for .include "lib/name.asm"
        if trimmed.to_lowercase().starts_with(".lib") {
            let rest = trimmed[4..].trim();
            if rest.is_empty() {
                return Err(AsmError {
                    line: line_num + 1,
                    message: ".lib requires a library name".into(),
                });
            }
            let name = if (rest.starts_with('"') && rest.ends_with('"'))
                || (rest.starts_with('\'') && rest.ends_with('\''))
            {
                &rest[1..rest.len() - 1]
            } else {
                rest
            };
            let filename = format!("lib/{}.asm", name);
            // Search for the file (same logic as .include)
            let filepath = if let Some(dir) = lib_dir {
                let p = std::path::Path::new(dir).join(&filename);
                if p.exists() {
                    Some(p)
                } else {
                    std::path::Path::new(&filename)
                        .exists()
                        .then(|| std::path::PathBuf::from(&filename))
                }
            } else {
                std::path::Path::new(&filename)
                    .exists()
                    .then(|| std::path::PathBuf::from(&filename))
            };
            match filepath {
                Some(path) => {
                    let included = std::fs::read_to_string(&path).map_err(|e| AsmError {
                        line: line_num + 1,
                        message: format!("cannot read lib file '{}': {}", filename, e),
                    })?;
                    let expanded = resolve_includes(&included, lib_dir, depth + 1)?;
                    output.push_str("; --- begin lib: ");
                    output.push_str(&filename);
                    output.push_str(" ---\n");
                    output.push_str(&expanded);
                    if !expanded.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("; --- end lib: ");
                    output.push_str(&filename);
                    output.push_str(" ---\n");
                }
                None => {
                    return Err(AsmError {
                        line: line_num + 1,
                        message: format!("lib not found: '{}'", filename),
                    });
                }
            }
        } else if trimmed.to_lowercase().starts_with(".include") {
            // Parse: .include "filename" or .include filename
            let rest = trimmed[8..].trim();
            let filename = if (rest.starts_with('"') && rest.ends_with('"'))
                || (rest.starts_with('\'') && rest.ends_with('\''))
            {
                &rest[1..rest.len() - 1]
            } else {
                rest
            };
            let filename = filename.trim();
            if filename.is_empty() {
                return Err(AsmError {
                    line: line_num + 1,
                    message: ".include requires a filename".into(),
                });
            }
            // Search for the file
            let filepath = if let Some(dir) = lib_dir {
                let p = std::path::Path::new(dir).join(filename);
                if p.exists() {
                    Some(p)
                } else {
                    std::path::Path::new(filename)
                        .exists()
                        .then(|| std::path::PathBuf::from(filename))
                }
            } else {
                std::path::Path::new(filename)
                    .exists()
                    .then(|| std::path::PathBuf::from(filename))
            };
            match filepath {
                Some(path) => {
                    let included = std::fs::read_to_string(&path).map_err(|e| AsmError {
                        line: line_num + 1,
                        message: format!("cannot read include file '{}': {}", filename, e),
                    })?;
                    let expanded = resolve_includes(&included, lib_dir, depth + 1)?;
                    output.push_str("; --- begin included: ");
                    output.push_str(filename);
                    output.push_str(" ---\n");
                    output.push_str(&expanded);
                    if !expanded.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("; --- end included: ");
                    output.push_str(filename);
                    output.push_str(" ---\n");
                }
                None => {
                    return Err(AsmError {
                        line: line_num + 1,
                        message: format!("include file not found: '{}'", filename),
                    });
                }
            }
        } else {
            output.push_str(raw_line);
            output.push('\n');
        }
    }
    Ok(output)
}
