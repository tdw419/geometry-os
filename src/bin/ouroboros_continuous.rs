// Ouroboros Continuous — Overnight Self-Improvement
//
// The loop that never stops. Run this before bed.
// Wake up to a self-optimized swarm.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use std::process::Command;

const VISION_MODEL: &str = "qwen/qwen3-vl-8b";
const REASON_MODEL: &str = "qwen2.5-coder-7b-instruct";

const LM_STUDIO_URL: &str = "http://localhost:1234";
const MAX_ITERATIONS: usize = 1000;
const ITERATION_DELAY_SECS: u64 = 2;

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
    #[serde(default)]
    core_agents: u32,
    #[serde(default)]
    inner_agents: u32,
    #[serde(default)]
    periphery_agents: u32,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl ShaderParameters {
    fn apply(&mut self, changes: &[ParameterChange]) {
        for change in changes {
            match change.name.as_str() {
                "drift_coefficient" => self.drift_coefficient = change.new_value,
                "core_gravity" => self.core_gravity = change.new_value,
                "periphery_repulsion" => self.periphery_repulsion = change.new_value,
                "color_sorting_strength" => self.color_sorting_strength = change.new_value,
                _ => {}
            }
        }
    }
}

struct OuroborosContinuous {
    client: Client,
    iteration: usize,
    history: Vec<(usize, f32, ShaderParameters)>,
    params: ShaderParameters,
    start_time: Instant,
}

// ============================================================================
// IMPLEMENTATION
// ============================================================================

impl OuroborosContinuous {
    fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
            iteration: 0,
            history: vec![],
            params: ShaderParameters::default(),
            start_time: Instant::now(),
        }
    }

    /// Render the courier swarm with current parameters
    fn render(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n[RENDER] Running courier swarm...");
        
        // Call courier_swarm binary
        let output = Command::new("cargo")
            .args(&["run", "--release", "--bin", "courier_swarm"])
            .current_dir("/home/jericho/zion/projects/ascii_world/gpu")
            .output()?;
        
        if !output.status.success() {
            println!("[RENDER] Warning: Render returned non-zero exit code");
        }
        
        // Verify output exists
        let output_path = "/home/jericho/zion/projects/ascii_world/gpu/output/courier_swarm.png";
        if !Path::new(output_path).exists() {
            return Err("Framebuffer not generated".into());
        }
        
        println!("[RENDER] ✓ Frame saved to {}", output_path);
        Ok(())
    }

    /// Vision: Analyze the framebuffer
    async fn vision(&self, image_path: &str) -> Result<VisionResponse, Box<dyn std::error::Error>> {
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

    /// Reason: Assess state and recommend changes
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
            self.params,
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

        // Strip markdown code fences
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

        let assessment: Assessment = match serde_json::from_str(clean) {
            Ok(a) => a,
            Err(e) => {
                println!("[REASON] Parse error: {}", e);
                Assessment {
                    score: 0.5,
                    recommendations: vec!["Could not parse AI response".to_string()],
                    parameter_changes: vec![],
                }
            }
        };

        Ok(assessment)
    }

    /// Full iteration
    async fn iterate(&mut self) -> Result<f32, Box<dyn std::error::Error>> {
        self.iteration += 1;
        let iter_start = Instant::now();

        println!("\n{}", "═".repeat(60));
        println!("║  OUROBOROS ITERATION {:>5}                            ║", self.iteration);
        println!("{}", "═".repeat(60));

        // 1. Render
        self.render()?;

        // 2. Vision
        println!("\n[VISION] Analyzing courier swarm...");
        let vision = self.vision("output/courier_swarm.png").await?;
        println!("  Description: {}", vision.description.chars().take(80).collect::<String>());
        println!("  Issues: {:?}", vision.issues.iter().take(2).collect::<Vec<_>>());

        // 3. Reason
        println!("\n[REASON] Assessing state...");
        let last_score = self.history.last().map(|(_, s, _)| *s);
        let assessment = self.reason(&vision, last_score).await?;
        println!("  Score: {:.2}", assessment.score);
        println!("  Recommendations: {:?}", assessment.recommendations.iter().take(2).collect::<Vec<_>>());

        // 4. Apply
        println!("\n[APPLY] Updating parameters...");
        self.params.apply(&assessment.parameter_changes);
        println!("  New params: drift={:.4} gravity={:.2} repulsion={:.2} sorting={:.2}",
            self.params.drift_coefficient,
            self.params.core_gravity,
            self.params.periphery_repulsion,
            self.params.color_sorting_strength);

        // Record history
        self.history.push((self.iteration, assessment.score, self.params.clone()));

        let elapsed = iter_start.elapsed();
        let total_elapsed = self.start_time.elapsed();
        
        println!("\n[METRICS]");
        println!("  Iteration time: {:?}", elapsed);
        println!("  Total runtime: {:?}", total_elapsed);
        println!("  Score trend: {:?}", self.history.iter().rev().take(5).map(|(_, s, _)| *s).collect::<Vec<_>>());

        // Save checkpoint
        if self.iteration % 10 == 0 {
            self.save_checkpoint()?;
        }

        Ok(assessment.score)
    }

    fn save_checkpoint(&self) -> Result<(), Box<dyn std::error::Error>> {
        let checkpoint = serde_json::to_string_pretty(&self.history)?;
        fs::write("output/ouroboros_checkpoint.json", checkpoint)?;
        println!("\n[CHECKPOINT] Saved to output/ouroboros_checkpoint.json");
        Ok(())
    }

    fn print_summary(&self) {
        println!("\n{}", "═".repeat(60));
        println!("║              OUROBOROS SESSION SUMMARY              ║");
        println!("{}", "═".repeat(60));
        println!("  Iterations: {}", self.iteration);
        println!("  Total runtime: {:?}", self.start_time.elapsed());
        
        if !self.history.is_empty() {
            let scores: Vec<f32> = self.history.iter().map(|(_, s, _)| *s).collect();
            let min = scores.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let avg = scores.iter().sum::<f32>() / scores.len() as f32;
            
            println!("  Score range: {:.2} → {:.2} (avg: {:.2})", min, max, avg);
            println!("  Final params: {:?}", self.params);
        }
        
        println!("{}", "═".repeat(60));
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("{}", "═".repeat(60));
    println!("║         OUROBOROS CONTINUOUS — OVERNIGHT MODE       ║");
    println!("{}", "═".repeat(60));
    println!("║  The loop that never stops. Run this before bed.    ║");
    println!("║  Wake up to a self-optimized swarm.                 ║");
    println!("{}", "═".repeat(60));
    println!("  Max iterations: {}", MAX_ITERATIONS);
    println!("  Delay between iterations: {}s", ITERATION_DELAY_SECS);
    println!("  Models: qwen3-vl-8b + qwen2.5-coder-7b");
    println!("{}", "═".repeat(60));
    println!();

    // Check LM Studio
    println!("[CHECK] Testing LM Studio connection...");
    let client = Client::new();
    match client.get(&format!("{}/v1/models", LM_STUDIO_URL)).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("[CHECK] ✓ LM Studio is running");
        }
        _ => {
            println!("[CHECK] ✗ LM Studio not responding");
            return Err("LM Studio not running".into());
        }
    }

    // Initialize
    let mut ouroboros = OuroborosContinuous::new();

    // Run the loop
    for _ in 0..MAX_ITERATIONS {
        match ouroboros.iterate().await {
            Ok(score) => {
                println!("\n✓ Iteration {} complete (score: {:.2})", ouroboros.iteration, score);
            }
            Err(e) => {
                println!("\n✗ Iteration {} failed: {}", ouroboros.iteration, e);
            }
        }
        
        tokio::time::sleep(Duration::from_secs(ITERATION_DELAY_SECS)).await;
    }

    // Final summary
    ouroboros.print_summary();
    ouroboros.save_checkpoint()?;

    println!("\n🌙 Good morning! The swarm has evolved.");
    println!("Check output/ouroboros_checkpoint.json for full history.");

    Ok(())
}
