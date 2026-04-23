import torch
import numpy as np
import json
from pathlib import Path
from transformers import GPT2LMHeadModel

def export_gpt2_to_pixelflow(model_id="gpt2", output_dir="gpt2_weights"):
    output_path = Path(output_dir)
    output_path.mkdir(exist_ok=True)
    
    print(f"Loading {model_id}...")
    model = GPT2LMHeadModel.from_pretrained(model_id)
    sd = model.state_dict()
    
    config = {
        "n_embd": model.config.n_embd,
        "n_head": model.config.n_head,
        "n_layer": model.config.n_layer,
        "vocab_size": model.config.vocab_size,
    }
    
    with open(output_path / "config.json", "w") as f:
        json.dump(config, f)
        
    print("Exporting weights...")
    for name, tensor in sd.items():
        # Convert to float32 and then to numpy
        arr = tensor.detach().cpu().numpy().astype(np.float32)
        
        # GPT-2 uses Conv1D layers which are transposed relative to standard Linear
        # attn.c_attn is [768, 2304] (Q, K, V concatenated)
        # We want to keep them in a format our matmul likes (M x K)
        
        # Simplify names for file system
        safe_name = name.replace(".", "_")
        np.save(output_path / f"{safe_name}.npy", arr)
        print(f"  {name} -> {safe_name}.npy {arr.shape}")

if __name__ == "__main__":
    import sys
    # Use a small subset or just the config if running in a constrained env
    # But here we want the real deal for the demo.
    export_gpt2_to_pixelflow()
