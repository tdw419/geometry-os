"""Pixelflow server -- bridges GPT-2 shader inference to Geometry OS.

Runs as a standalone process. Geometry OS apps connect via Unix socket
to request inference, get back token results, and receive weight/activation
visualizations as pixel data.

Protocol:
  > predict <prompt_tokens_json>
  < logits_json

  > weights <layer> <name>
  < <width> <height>\n<raw_f32_bytes>

  > activation <layer> <step>
  < <width> <height>\n<raw_f32_bytes>

  > generate <prompt_text> <n_tokens>
  < token1\ntoken2\n...\n<END>
"""
import os, sys, json, time, struct, socket, traceback
import numpy as np
from pathlib import Path

os.environ.setdefault('__NV_PRIME_RENDER_OFFLOAD', '1')
os.environ.setdefault('__GLX_VENDOR_LIBRARY_NAME', 'nvidia')

SOCKET_PATH = "/tmp/pixelflow.sock"

class PixelflowServer:
    def __init__(self):
        from pixelflow.engine_v5 import GPT2Engine
        print("Loading GPT-2 engine...")
        self.engine = GPT2Engine()
        self.tokenizer = self.engine.tokenizer
        
        # Pre-load weights as visualizable pixel arrays
        self.weight_pixels = {}
        self._prepare_weight_visuals()
        
        # Cache last forward pass activations for visualization
        self.last_activations = {}
        
        print(f"Server ready at {SOCKET_PATH}")
    
    def _prepare_weight_visuals(self):
        """Convert weight matrices to 8-bit pixel arrays for visualization."""
        for name, arr in self.engine.weights.items():
            if arr.ndim == 2 and arr.shape[0] > 10:
                # Normalize to 0-255
                mn, mx = arr.min(), arr.max()
                if mx > mn:
                    vis = ((arr - mn) / (mx - mn) * 255).astype(np.uint8)
                else:
                    vis = np.zeros_like(arr, dtype=np.uint8)
                self.weight_pixels[name] = vis
    
    def handle_predict(self, tokens_json):
        tokens = json.loads(tokens_json)
        logits = self.engine.forward(tokens, use_gpu=True)
        return json.dumps(logits.tolist())
    
    def handle_generate(self, text, n_tokens=10):
        tokens = self.tokenizer.encode(text)
        result_tokens = []
        for _ in range(n_tokens):
            logits = self.engine.forward(tokens, use_gpu=True)
            next_tok = int(np.argmax(logits))
            tokens.append(next_tok)
            result_tokens.append(next_tok)
        return "\n".join(self.tokenizer.decode([t]) for t in result_tokens) + "\n<END>"
    
    def handle_weights(self, layer, name_prefix):
        """Return weight visualization as raw bytes."""
        full_name = f"transformer_h_{layer}_{name_prefix}_weight"
        if full_name in self.weight_pixels:
            vis = self.weight_pixels[full_name]
            h, w = vis.shape
            return f"{w} {h}\n".encode() + vis.tobytes()
        return b"0 0\n"
    
    def handle_all_weights_visual(self):
        """Return a composite image of all weight matrices as 256x256 pixels."""
        # Create a 256x256 image showing weight matrix thumbnails
        img = np.zeros((256, 256), dtype=np.uint8)
        
        # Layout: 4 columns, each weight gets a thumbnail
        matrices = sorted([(n, v) for n, v in self.weight_pixels.items() 
                          if v.shape[0] <= 1024], key=lambda x: x[0])
        
        col_w = 64  # 4 columns of 64px each in 256px
        row_h = 32
        col, row = 0, 0
        
        for name, vis in matrices:
            x0 = col * col_w
            y0 = row * row_h
            
            # Resize to fit thumbnail
            h, w = vis.shape
            scale_x = min(col_w / w, 1.0)
            scale_y = min(row_h / h, 1.0)
            scale = min(scale_x, scale_y)
            new_w = max(1, int(w * scale))
            new_h = max(1, int(h * scale))
            
            # Simple nearest-neighbor resize
            yi = np.linspace(0, h-1, new_h).astype(int)
            xi = np.linspace(0, w-1, new_w).astype(int)
            thumb = vis[np.ix_(yi, xi)]
            
            # Place in composite image
            if y0 + new_h <= 256 and x0 + new_w <= 256:
                img[y0:y0+new_h, x0:x0+new_w] = thumb
            
            col += 1
            if col >= 4:
                col = 0
                row += 1
        
        return img.tobytes()
    
    def handle_logits_visual(self, text="The"):
        """Return logits as a 256x8 pixel bar visualization."""
        tokens = self.tokenizer.encode(text)
        logits = self.engine.forward(tokens, use_gpu=True)
        
        # Normalize logits to 0-255
        top_k = min(256, len(logits))
        top_indices = np.argsort(logits)[-top_k:]
        top_logits = logits[top_indices]
        mn, mx = top_logits.min(), top_logits.max()
        if mx > mn:
            normalized = ((top_logits - mn) / (mx - mn) * 255).astype(np.uint8)
        else:
            normalized = np.zeros(top_k, dtype=np.uint8)
        
        # Create 256x8 bar chart
        img = np.zeros((8, 256), dtype=np.uint8)
        for i, val in enumerate(normalized[:256]):
            bar_h = max(1, int(val / 255.0 * 8))
            img[8-bar_h:8, i] = val
        
        return img.tobytes()
    
    def run(self):
        if os.path.exists(SOCKET_PATH):
            os.unlink(SOCKET_PATH)
        
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(SOCKET_PATH)
        server.listen(5)
        server.setblocking(True)
        
        print(f"Listening on {SOCKET_PATH}...")
        
        while True:
            conn, _ = server.accept()
            try:
                self._handle_connection(conn)
            except Exception as e:
                traceback.print_exc()
            finally:
                conn.close()
    
    def _handle_connection(self, conn):
        data = b""
        while True:
            chunk = conn.recv(65536)
            if not chunk:
                break
            data += chunk
        
        request = data.decode().strip()
        if not request:
            return
        
        parts = request.split(None, 1)
        cmd = parts[0]
        args = parts[1] if len(parts) > 1 else ""
        
        if cmd == "predict":
            result = self.handle_predict(args)
            conn.sendall(result.encode())
        
        elif cmd == "generate":
            gen_parts = args.split(None, 1)
            text = gen_parts[0] if gen_parts else "The"
            n = int(gen_parts[1]) if len(gen_parts) > 1 else 10
            result = self.handle_generate(text, n)
            conn.sendall(result.encode())
        
        elif cmd == "weights":
            wparts = args.split()
            layer = int(wparts[0]) if wparts else 0
            name = wparts[1] if len(wparts) > 1 else "attn_c_attn"
            result = self.handle_weights(layer, name)
            conn.sendall(result)
        
        elif cmd == "all_weights":
            result = self.handle_all_weights_visual()
            conn.sendall(result)
        
        elif cmd == "logits":
            text = args if args else "The"
            result = self.handle_logits_visual(text)
            conn.sendall(result)
        
        elif cmd == "status":
            status = json.dumps({
                "model": "gpt2",
                "layers": 12,
                "heads": 12,
                "dim": 768,
                "vocab": 50257,
                "weight_matrices": len(self.weight_pixels),
            })
            conn.sendall(status.encode())
        
        else:
            conn.sendall(f"Unknown command: {cmd}\n".encode())


def geo_cmd(cmd):
    """Send command to Geometry OS socket."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect('/tmp/geo_cmd.sock')
    s.sendall((cmd + '\n').encode())
    s.shutdown(socket.SHUT_WR)
    data = b''
    while True:
        chunk = s.recv(65536)
        if not chunk: break
        data += chunk
    s.close()
    return data.decode()


if __name__ == "__main__":
    server = PixelflowServer()
    server.run()
