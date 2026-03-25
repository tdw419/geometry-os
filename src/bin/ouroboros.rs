// Ouroboros Loop — Self-Improving Framebuffer AI
//
// The system that sees itself, judges itself, and improves itself.
//
// Loop: RENDER → VISION → REASON → GENERATE → APPLY → RENDER

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{Duration, Instant};

const VISION_MODEL: &str = "qwen/qwen3-vl-8b";
const REASON_MODEL: &str = "qwen2.5-coder-7b-instruct";  // Quinn
const GENERATE_MODEL: &str = "qwen2.5-coder-7b-instruct";  // Same model for generation

const LM_STUDIO_URL: &str = "http://localhost:1234";

// ============================================================================
// STRUCTS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct VisionResponse {
    description: String,
    zones: ZoneAnalysis,
    issues: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ZoneAnalysis {
    core_agents: u32,
    inner_agents: u32,
    periphery_agents: u32,
    avg_velocity: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Assessment {
    score: f32,
    recommendations: Vec<String>,
    parameter_changes: Vec<ParameterChange>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ParameterChange {
    name: String,
    old_value: f32,
    new_value: f32,
    reason: String,
}

struct OuroborosLoop {
    client: Client,
    iteration: u32,
    history: Vec<f32>,  // Score history
    parameters: ShaderParameters,
}

#[derive(Debug, Clone, Serialize)]
struct ShaderParameters {
    drift_coefficient: f32,
    core_gravity: f32,
    periphery_repulsion: f32,
    color_sorting_strength: f32,
}

impl Default for ShaderParameters {
    fn default() -> Self {
        Self {
            drift_coefficient: 0.0001,
            core_gravity: 0.5,
            periphery_repulsion: 0.0,
            color_sorting_strength: 0.05,
        }
    }
}

// ============================================================================
// LM STUDIO API
// ============================================================================

impl OuroborosLoop {
    fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap(),
            iteration: 0,
            history: vec![],
            parameters: ShaderParameters::default(),
        }
    }

    /// Step 1: Vision — Analyze framebuffer with qwen3-vl-8b
    async fn vision(&self, image_path: &str) -> Result<VisionResponse, Box<dyn std::error::Error>> {
        // Read and encode image
        let image_data = fs::read(image_path)?;
        use base64::{Engine as _, engine::general_purpose};
        let base64_image = general_purpose::STANDARD.encode(&image_data);

        let prompt = r#"Analyze this 64-agent swarm framebuffer.
Count agents in each zone (core, inner, periphery).
Identify any issues.
Output JSON only:
{
  "description": "brief description",
  "zones": {"core_agents": N, "inner_agents": N, "periphery_agents": N, "avg_velocity": X.X},
  "issues": ["issue1", "issue2"]
}"#;

        let response = self.client
            .post(&format!("{}/v1/chat/completions", LM_STUDIO_URL))
            .json(&serde_json::json!({
                "model": VISION_MODEL,
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64_image)}}
                    ]
                }],
                "max_tokens": 500,
                "temperature": 0.0
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}");

        // Parse JSON from response
        let vision: VisionResponse = serde_json::from_str(content)
            .unwrap_or(VisionResponse {
                description: content.to_string(),
                zones: ZoneAnalysis {
                    core_agents: 0,
                    inner_agents: 0,
                    periphery_agents: 0,
                    avg_velocity: 0.0,
                },
                issues: vec![],
            });

        Ok(vision)
    }

    /// Step 2: Reason — Assess state with qwen3-coder-30b
    async fn reason(&self, vision: &VisionResponse, last_score: Option<f32>) -> Result<Assessment, Box<dyn std::error::Error>> {
        let system_prompt = r#"You are a swarm optimization AI.
Goal: Maximize cluster quality while maintaining drift balance.

Score the current state (0.0-1.0) and suggest parameter changes.
Output JSON only."#;

        let user_prompt = format!(
            r#"Current state:
- Description: {}
- Core agents: {}
- Inner agents: {}
- Periphery agents: {}
- Issues: {:?}
- Previous score: {:?}
- Current parameters: {:?}

Assess and recommend changes. Output JSON:
{{
  "score": 0.X,
  "recommendations": ["rec1", "rec2"],
  "parameter_changes": [
    {{"name": "drift_coefficient", "old_value": X, "new_value": Y, "reason": "why"}}
  ]
}}"#,
            vision.description,
            vision.zones.core_agents,
            vision.zones.inner_agents,
            vision.zones.periphery_agents,
            vision.issues,
            last_score,
            self.parameters,
        );

        let response = self.client
            .post(&format!("{}/v1/chat/completions", LM_STUDIO_URL))
            .json(&serde_json::json!({
                "model": REASON_MODEL,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "max_tokens": 1000,
                "temperature": 0.3
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}");

        // Debug: print raw response (full)
        println!("      Raw response length: {} bytes", content.len());
        if content.len() < 1000 {
            println!("      Full response: {}", content);
        } else {
            println!("      Response preview: {}...", &content[..500]);
        }

        // Strip markdown code fences if present
        let mut clean = content.trim();
        if clean.starts_with("```json") {
            clean = &clean[7..];
        } else if clean.starts_with("```") {
            clean = &clean[3..];
        }
        if clean.ends_with("```") {
            clean = &clean[..clean.len()-3];
        }
        clean = clean.trim();

        // Parse JSON from response
        let assessment: Assessment = match serde_json::from_str(clean) {
            Ok(a) => a,
            Err(e) => {
                println!("      Parse error: {}", e);
                println!("      Clean content: {}", clean);
                Assessment {
                    score: 0.5,
                    recommendations: vec!["Could not parse AI response".to_string()],
                    parameter_changes: vec![],
                }
            }
        };

        Ok(assessment)
    }

    /// Step 3: Generate — Create shader patches (optional, can use reason output directly)
    async fn generate(&self, assessment: &Assessment) -> Result<String, Box<dyn std::error::Error>> {
        if assessment.parameter_changes.is_empty() {
            return Ok("// No changes needed".to_string());
        }

        let prompt = format!(
            "Generate WGSL parameter changes:\n{:?}\n\nFormat: const NAME: f32 = VALUE; // reason",
            assessment.parameter_changes
        );

        let response = self.client
            .post(&format!("{}/v1/completions", LM_STUDIO_URL))
            .json(&serde_json::json!({
                "model": GENERATE_MODEL,
                "prompt": prompt,
                "max_tokens": 200,
                "temperature": 0.0,
                "stop": ["\n\n", "// END"]
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        let patches = json["choices"][0]["text"]
            .as_str()
            .unwrap_or("// No patches generated")
            .to_string();

        Ok(patches)
    }

    /// Step 4: Apply — Update parameters
    fn apply(&mut self, assessment: &Assessment) {
        for change in &assessment.parameter_changes {
            match change.name.as_str() {
                "drift_coefficient" => {
                    self.parameters.drift_coefficient = change.new_value;
                }
                "core_gravity" => {
                    self.parameters.core_gravity = change.new_value;
                }
                "periphery_repulsion" => {
                    self.parameters.periphery_repulsion = change.new_value;
                }
                "color_sorting_strength" => {
                    self.parameters.color_sorting_strength = change.new_value;
                }
                _ => {}
            }
        }
    }

    /// Full iteration
    async fn iterate(&mut self, framebuffer_path: &str) -> Result<f32, Box<dyn std::error::Error>> {
        self.iteration += 1;
        let start = Instant::now();

        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║  OUROBOROS ITERATION {:>5}                              ║", self.iteration);
        println!("╚══════════════════════════════════════════════════════════╝");

        // 1. Vision
        println!("\n[1/4] VISION — Analyzing framebuffer...");
        let vision = self.vision(framebuffer_path).await?;
        println!("      Description: {}", vision.description);
        println!("      Issues: {:?}", vision.issues);

        // 2. Reason
        println!("\n[2/4] REASON — Assessing state...");
        let last_score = self.history.last().copied();
        let assessment = self.reason(&vision, last_score).await?;
        println!("      Score: {:.2}", assessment.score);
        println!("      Recommendations: {:?}", assessment.recommendations);

        // 3. Generate (optional)
        println!("\n[3/4] GENERATE — Creating patches...");
        let patches = self.generate(&assessment).await?;
        println!("      Patches:\n{}", patches);

        // 4. Apply
        println!("\n[4/4] APPLY — Updating parameters...");
        self.apply(&assessment);
        println!("      New parameters: {:?}", self.parameters);

        // Record score
        self.history.push(assessment.score);

        let elapsed = start.elapsed();
        println!("\n✓ Iteration complete in {:?}", elapsed);
        println!("  Score trend: {:?}", self.history.iter().rev().take(5).collect::<Vec<_>>());

        Ok(assessment.score)
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              OUROBOROS LOOP — Self-Improving AI          ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Loop: RENDER → VISION → REASON → GENERATE → APPLY       ║");
    println!("║  Models: qwen3-vl-8b + qwen3-coder-30b + tinyllama      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Check LM Studio
    println!("[CHECK] Testing LM Studio connection...");
    let client = Client::new();
    match client.get(&format!("{}/v1/models", LM_STUDIO_URL)).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("[CHECK] ✓ LM Studio is running");
            } else {
                println!("[CHECK] ✗ LM Studio returned error: {}", resp.status());
                return Err("LM Studio not ready".into());
            }
        }
        Err(e) => {
            println!("[CHECK] ✗ Cannot connect to LM Studio: {}", e);
            return Err("LM Studio not running".into());
        }
    }

    // Initialize loop
    let mut ouroboros = OuroborosLoop::new();

    // Use existing radial_drift.png as input
    let framebuffer_path = "output/radial_drift.png";

    if !std::path::Path::new(framebuffer_path).exists() {
        println!("[ERROR] Framebuffer not found: {}", framebuffer_path);
        println!("[INFO] Run `cargo run --release --bin radial_drift` first");
        return Err("Framebuffer not found".into());
    }

    // Run 3 iterations
    for _ in 0..3 {
        let score = ouroboros.iterate(framebuffer_path).await?;
        println!("\n  Current score: {:.2}", score);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║              OUROBOROS COMPLETE                          ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Iterations: {:>5}                                      ║", ouroboros.iteration);
    println!("║  Score history: {:?}", ouroboros.history);
    println!("║  Final parameters: {:?}", ouroboros.parameters);
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}
