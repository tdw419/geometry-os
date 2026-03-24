// Visual Circuit Diagrams
// Draws ASCII schematics representing VM code logic flow
// Part of Visual Cognition Phase 1

use reqwest;
use serde_json;
use std::collections::HashMap;

const FB_WIDTH: usize = 80;  // ASCII width
const FB_HEIGHT: usize = 24; // ASCII height

#[derive(Debug, Clone)]
pub enum CircuitNode {
    Input { name: String, value: i32 },
    Output { name: String },
    Operation { op: char, inputs: usize },
    Register { name: char, value: i32 },
}

#[derive(Debug, Clone)]
pub struct CircuitEdge {
    from: usize,
    to: usize,
    label: Option<String>,
}

pub struct CircuitDiagram {
    nodes: Vec<CircuitNode>,
    edges: Vec<CircuitEdge>,
    grid: Vec<Vec<char>>,
}

impl CircuitDiagram {
    pub fn new() -> Self {
        CircuitDiagram {
            nodes: Vec::new(),
            edges: Vec::new(),
            grid: vec![vec![' '; FB_WIDTH]; FB_HEIGHT],
        }
    }

    /// Parse VM code into circuit nodes
    pub fn parse_code(&mut self, code: &str) {
        self.nodes.clear();
        self.edges.clear();

        let tokens: Vec<&str> = code.split_whitespace().collect();
        let mut stack: Vec<usize> = Vec::new();
        let mut registers: HashMap<char, usize> = HashMap::new();

        for (i, token) in tokens.iter().enumerate() {
            if let Ok(n) = token.parse::<i32>() {
                // Push input value
                let node_idx = self.nodes.len();
                self.nodes.push(CircuitNode::Input {
                    name: format!("const_{}", n),
                    value: n,
                });
                stack.push(node_idx);
            } else {
                match *token {
                    "+" | "-" | "*" | "/" => {
                        // Binary operation
                        let b = stack.pop().unwrap_or(0);
                        let a = stack.pop().unwrap_or(0);
                        let op_idx = self.nodes.len();
                        self.nodes.push(CircuitNode::Operation {
                            op: token.chars().next().unwrap(),
                            inputs: 2,
                        });
                        self.edges.push(CircuitEdge { from: a, to: op_idx, label: None });
                        self.edges.push(CircuitEdge { from: b, to: op_idx, label: None });
                        stack.push(op_idx);
                    }
                    "." => {
                        // Output
                        let src = stack.last().copied().unwrap_or(0);
                        let out_idx = self.nodes.len();
                        self.nodes.push(CircuitNode::Output { name: "out".to_string() });
                        self.edges.push(CircuitEdge { from: src, to: out_idx, label: None });
                    }
                    ":" => {
                        // Dup
                        if let Some(&src) = stack.last() {
                            stack.push(src);
                        }
                    }
                    "@" => {
                        // Halt - connect to output
                        let src = stack.last().copied().unwrap_or(0);
                        let out_idx = self.nodes.len();
                        self.nodes.push(CircuitNode::Output { name: "halt".to_string() });
                        self.edges.push(CircuitEdge { from: src, to: out_idx, label: Some("@".to_string()) });
                    }
                    _ => {
                        let c = token.chars().next().unwrap();
                        if c.is_ascii_lowercase() {
                            // Store to register
                            let src = stack.pop().unwrap_or(0);
                            let reg_idx = if let Some(&idx) = registers.get(&c) {
                                idx
                            } else {
                                let idx = self.nodes.len();
                                self.nodes.push(CircuitNode::Register { name: c.to_ascii_uppercase(), value: 0 });
                                registers.insert(c, idx);
                                idx
                            };
                            self.edges.push(CircuitEdge { from: src, to: reg_idx, label: Some(format!("→{}", c.to_ascii_uppercase())) });
                        } else if c.is_ascii_uppercase() {
                            // Load from register
                            if let Some(&src) = registers.get(&c.to_ascii_lowercase()) {
                                stack.push(src);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render circuit to ASCII art
    pub fn render(&mut self) -> String {
        // Clear grid
        for row in &mut self.grid {
            for cell in row.iter_mut() {
                *cell = ' ';
            }
        }

        // Layout nodes in layers
        let mut x = 2;
        let mut y = 2;

        // Draw border
        self.draw_box(0, 0, FB_WIDTH - 1, FB_HEIGHT - 1, "Circuit");

        // Collect node info first to avoid borrow conflicts
        let node_info: Vec<(String, usize, usize, usize)> = self.nodes.iter().enumerate().map(|(i, node)| {
            match node {
                CircuitNode::Input { value, .. } => (format!("{}:{}", value, i), 15, x, y),
                CircuitNode::Operation { op, .. } => (format!("({})", op), 5, x, y),
                CircuitNode::Output { name } => (format!("[{}]", name), 8, x, y),
                CircuitNode::Register { name, value } => (format!("{}:{}", name, value), 12, x, y),
            }
        }).collect();

        // Draw nodes
        for (text, width, nx, ny) in &node_info {
            self.draw_box(*nx, *ny, *nx + 12, *ny + 2, "");
            self.print_at(*nx + 1, *ny + 1, text);
        }

        // Draw edges as arrows
        let edges: Vec<(usize, usize)> = self.edges.iter().map(|e| (e.from, e.to)).collect();
        for (from, to) in edges {
            // Simple arrow representation
            self.draw_arrow(from, to);
        }

        // Convert grid to string
        self.grid.iter().map(|row| row.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    fn draw_box(&mut self, x1: usize, y1: usize, x2: usize, y2: usize, title: &str) {
        // Draw corners
        self.grid[y1][x1] = '┌';
        self.grid[y1][x2] = '┐';
        self.grid[y2][x1] = '└';
        self.grid[y2][x2] = '┘';

        // Draw sides
        for x in (x1 + 1)..x2 {
            self.grid[y1][x] = '─';
            self.grid[y2][x] = '─';
        }
        for y in (y1 + 1)..y2 {
            self.grid[y][x1] = '│';
            self.grid[y][x2] = '│';
        }

        // Draw title
        if !title.is_empty() {
            self.print_at(x1 + 2, y1, title);
        }
    }

    fn print_at(&mut self, x: usize, y: usize, text: &str) {
        for (i, c) in text.chars().enumerate() {
            if x + i < FB_WIDTH && y < FB_HEIGHT {
                self.grid[y][x + i] = c;
            }
        }
    }

    fn draw_arrow(&mut self, _from: usize, _to: usize) {
        // Simplified: just draw a small arrow
        // In full version, would trace actual node positions
    }

    /// Generate flowchart-style ASCII
    pub fn to_flowchart(&self) -> String {
        let mut result = String::new();
        result.push_str("┌─────────────────────────────────────────────────────────┐\n");
        result.push_str("│                   CIRCUIT DIAGRAM                       │\n");
        result.push_str("├─────────────────────────────────────────────────────────┤\n");

        for (i, node) in self.nodes.iter().enumerate() {
            let node_str = match node {
                CircuitNode::Input { value, .. } => format!("[{}]", value),
                CircuitNode::Operation { op, .. } => format!("({})", op),
                CircuitNode::Output { name } => format!("⟹{}", name),
                CircuitNode::Register { name, value } => format!("⟨{}:{}⟩", name, value),
            };

            // Find incoming edges
            let incoming: Vec<_> = self.edges.iter()
                .filter(|e| e.to == i)
                .map(|e| e.from)
                .collect();

            let prefix = if incoming.is_empty() {
                "  ".to_string()
            } else {
                format!("{}→", incoming.iter().map(|n| format!("#{}", n)).collect::<Vec<_>>().join(","))
            };

            result.push_str(&format!("│ {}#{}: {}{}\n", prefix, i, node_str,
                if i % 3 == 2 { "" } else { "" }));
        }

        result.push_str("└─────────────────────────────────────────────────────────┘\n");
        result
    }
}

// LLM integration for circuit generation
const FEW_SHOT_PREFIX: &str = "VM Code (opcodes only):\n\
    Push 5, halt = 5 @\n\
    Push 5, push 3, add, halt = 5 3 + . @\n";

async fn generate_code(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let full_prompt = format!("{}{} =", FEW_SHOT_PREFIX, prompt);

    let response = client
        .post("http://localhost:1234/v1/completions")
        .json(&serde_json::json!({
            "model": "tinyllama-1.1b-chat-v1.0",
            "prompt": full_prompt,
            "max_tokens": 50,
            "temperature": 0.0,
            "stop": ["\n", "Explanation"]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json["choices"][0]["text"].as_str().unwrap_or("ERROR").to_string())
}

fn clean_code(raw: &str) -> String {
    let valid: std::collections::HashSet<char> =
        "0123456789+-*/.,:@&?<>^v!_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz "
        .chars().collect();
    let cleaned: String = raw.chars().filter(|c| valid.contains(c) || c.is_whitespace()).collect();
    let tokens: Vec<&str> = cleaned.split_whitespace().take(20).collect();
    let mut result = tokens.join(" ");
    if !result.ends_with('@') && !result.is_empty() { result.push_str(" @"); }
    if result.is_empty() { "1 @".to_string() } else { result }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║            VISUAL CIRCUIT DIAGRAMS                       ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Visual Cognition Phase 1                               ║");
    println!("║  LLM generates code → Circuit visualization             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let tests = vec![
        ("Addition", "Push 5, push 3, add, halt"),
        ("Multiply", "Push 10, push 4, multiply, halt"),
        ("Complex", "Push 2, push 3, add, push 4, multiply, halt"),
    ];

    for (name, prompt) in tests {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[TEST: {}]", name);
        println!("[PROMPT] {}", prompt);

        // Generate code
        let raw = generate_code(prompt).await?;
        let code = clean_code(&raw);
        println!("[CODE] {}", code);

        // Parse and visualize
        let mut circuit = CircuitDiagram::new();
        circuit.parse_code(&code);

        // Show flowchart
        println!();
        println!("{}", circuit.to_flowchart());
        println!();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    FEATURES                              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ Parse VM code into circuit nodes                    ║");
    println!("║  ✅ Track data flow (edges)                             ║");
    println!("║  ✅ Generate ASCII flowchart                            ║");
    println!("║  ✅ Visual debugging aid                                ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}
