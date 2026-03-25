#!/usr/bin/env python3
import sys
import yaml
from pathlib import Path

# Add Ouroboros to path
OUROBOROS_PATH = Path("/home/jericho/zion/projects/ouroboros/ouroboros")
sys.path.insert(0, str(OUROBOROS_PATH))

from autonomous_loop import AutonomousLoop

def main():
    goal_path = Path(".ouroboros/goal.yaml")
    if not goal_path.exists():
        print(f"❌ Error: {goal_path} not found.")
        sys.exit(1)

    with open(goal_path, 'r') as f:
        config = yaml.safe_load(f)

    print(f"🚀 Launching Ouroboros Loop for: {config['name']}")
    print(f"🎯 Objective: {config['description']}")
    
    # Instantiate the loop with our goal.yaml parameters
    # Note: We override the target to the Sovereign Shell shader
    loop = AutonomousLoop(
        objective=config['description'],
        criteria="\n".join(config['success_criteria']),
        target="sovereign_shell_hud.wgsl",
        metric_name="loop_latency",
        max_iterations=config.get('max_iterations', 50)
    )

    try:
        loop.run()
    except KeyboardInterrupt:
        print("\n🛑 Loop stopped by user.")

if __name__ == "__main__":
    main()
