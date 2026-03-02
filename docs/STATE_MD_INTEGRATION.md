# STATE.md Integration for Geometry OS Font Toolkit

## Why STATE.md Fits This Project

The Geometry OS Font Toolkit is an ideal candidate for the STATE.md auto-prompting loop system. Here's why:

### 1. Multi-Step, Cross-Subsystem Tasks

Adding a new instruction glyph requires coordinating changes across:

```
core/atlas_gen.py          → Register glyph with opcode
geometry_os_spirv/scripts/ → Add opcode handling
web/VisualCompiler.js      → Update browser compiler
web/VisualIDE.js           → Update IDE UI
docs/*.md                  → Document new instruction
```

**Without STATE.md**: Easy to forget a step, lose track of progress
**With STATE.md**: Each iteration tracks exactly what's done and what's next

### 2. Experimental, Iterative Development

This is a research project with novel concepts (morphological computation, Hilbert curves, visual programming). Development often involves:

- "Try adding this opcode and see what happens"
- "Test the visual output, adjust parameters"
- "Run the SPIR-V compiler, debug failures"

STATE.md excels at **exploratory tasks** where the path isn't fully known upfront.

### 3. Already AI-Friendly

The project has `AI_ONBOARDING.md` - it's designed for AI assistance. STATE.md extends this philosophy:

```
AI_ONBOARDING.md  →  "Here's how to understand the project"
STATE.md          →  "Here's what I'm doing right now"
```

### 4. RGB Encoding Alignment

**Geometry OS Font Toolkit** uses RGB semantic encoding:
- R = Visual structure
- G = Opcode (instruction type)
- B = Operand (value)

**STATE.md** uses the same RGB metaphor:
- R = Context (what we know)
- G = Action (what to do)
- B = Target (where to apply)

This is **philosophically aligned** - the same mental model works for both the font system and task state.

---

## Concrete Use Cases

### Use Case 1: Adding a New Instruction

**Task**: "Add a MUL (multiply) instruction with opcode 140"

**STATE.md evolution**:

```markdown
# Iteration 1
## R: Context
- **Goal**: Add MUL instruction, opcode 140
- **Progress**: Starting, need to understand current instruction set
- **Files**: None yet
- **Blockers**: None

## G: Action
ANALYZE: Review existing instruction implementations

## B: Target
target: core/atlas_gen.py
```

```markdown
# Iteration 2
## R: Context
- **Goal**: Add MUL instruction, opcode 140
- **Progress**: Found ADD (132), SUB (133) patterns
- **Files**: core/atlas_gen.py (read)
- **Blockers**: None

## G: Action
EDIT: Add MUL glyph registration to atlas_gen.py

## B: Target
target: core/atlas_gen.py
content: |
  # Add after SUB registration:
  # (140, 'MUL', ...glyph params...)
```

```markdown
# Iteration 3
## R: Context
- **Goal**: Add MUL instruction, opcode 140
- **Progress**: Atlas registration done, need SPIR-V handler
- **Files**: core/atlas_gen.py (edited)
- **Blockers**: None

## G: Action
EDIT: Add MUL opcode handler to visual_to_spirv.py

## B: Target
target: geometry_os_spirv/scripts/visual_to_spirv.py
```

```markdown
# Iteration 4
## R: Context
- **Goal**: Add MUL instruction, opcode 140
- **Progress**: Core implementation complete, need to regenerate atlas
- **Files**: atlas_gen.py, visual_to_spirv.py (edited)
- **Blockers**: None

## G: Action
RUN: Regenerate font atlas

## B: Target
target: python3 core/atlas_gen.py
```

```markdown
# Iteration 5
## R: Context
- **Goal**: Add MUL instruction, opcode 140
- **Progress**: Atlas regenerated, need to test in web IDE
- **Files**: atlas_gen.py, visual_to_spirv.py, assets/glyph_info.json
- **Blockers**: None

## G: Action
RUN: Start web server and verify MUL renders correctly

## B: Target
target: cd web && python3 -m http.server 8770
```

```markdown
# Iteration 6
## R: Context
- **Goal**: Add MUL instruction, opcode 140
- **Progress**: Verified working in web IDE
- **Files**: Multiple files updated
- **Blockers**: None

## G: Action
DONE: MUL instruction added and tested successfully

## B: Target
target: .
```

### Use Case 2: Debugging SPIR-V Compilation

**Task**: "Fix SPIR-V compilation error for shader programs"

**STATE.md captures debugging state**:

```markdown
## R: Context
- **Goal**: Fix SPIR-V compilation error
- **Progress**: Found error in emit_spirv.py at line 145, suspecting operand mismatch
- **Files**: visual_to_spirv.py, emit_spirv.py
- **Blockers**: Error only appears with certain glyph combinations

## G: Action
ANALYZE: Trace operand types through compilation pipeline

## B: Target
target: geometry_os_spirv/scripts/emit_spirv.py
```

The debugging context persists across iterations - no need to re-explain the problem.

### Use Case 3: Cross-Reference Documentation Updates

**Task**: "Update all docs to reflect new memory-mapped I/O addresses"

STATE.md tracks which files are done:

```markdown
## R: Context
- **Goal**: Update docs for new I/O addresses (123-126 → 200-203)
- **Progress**: Updated MORPHOLOGICAL_COMPUTATION.md, AI_ONBOARDING.md
- **Files**: 2/4 docs updated
- **Blockers**: Need to verify SKILL.md files also reference correct addresses

## G: Action
EDIT: Update geometry_os_spirv/SKILL.md with new addresses

## B: Target
target: geometry_os_spirv/SKILL.md
```

---

## Implementation Plan

### Step 1: Add .loop/ Directory

```bash
mkdir -p .loop
```

### Step 2: Copy Loop Files

From the auto-prompting project, copy:
- `start.py`
- `runner.py`
- `SYSTEM.md`
- `schema.md`

### Step 3: Customize SYSTEM.md

Add Geometry OS Font Toolkit context:

```markdown
# SYSTEM.md for Geometry OS Font Toolkit

You are working on the Geometry OS Font Toolkit - a morphological font system
where text glyphs carry RGB-encoded semantic data.

## Project Structure
- core/ - Python font generation (atlas_gen.py, ttf_export.py)
- geometry_os_spirv/ - SPIR-V compilation pipeline
- web/ - Browser-based IDE and execution
- docs/ - Technical documentation

## RGB Alignment
This project uses RGB encoding for glyphs:
- R = Visual structure
- G = Opcode (>=128 = instruction)
- B = Operand value

STATE.md uses the same RGB metaphor:
- R = Context
- G = Action
- B = Target

## Common Workflows
1. Adding instructions: atlas_gen.py → visual_to_spirv.py → regenerate atlas
2. Web IDE changes: Edit JS files → refresh browser → test
3. SPIR-V debugging: Check emit_spirv.py for binary emission issues

## Testing
- npm test - Run vitest for web components
- python3 core/atlas_gen.py - Regenerate font atlas
- Web demo at localhost:8770
```

### Step 4: Try It

```bash
# Start a task
python .loop/start.py "Add a DIV instruction with opcode 141"

# Run the loop
python .loop/runner.py
```

---

## Benefits Summary

| Benefit | How It Helps |
|---------|--------------|
| **Never lose context** | STATE.md persists across sessions, Ctrl+C safe |
| **Track multi-file changes** | R: Context lists all files modified |
| **Resume after interrupt** | State is saved, just run again |
| **Debug with history** | See what was tried, what failed |
| **Aligned mental model** | RGB encoding matches the font system |
| **Autonomous execution** | Let Claude complete tasks while you do other things |

## When STATE.md Is Most Valuable

1. **Complex tasks** spanning 3+ files
2. **Debugging sessions** that might take multiple attempts
3. **Documentation updates** affecting multiple files
4. **New feature additions** requiring coordination across subsystems
5. **Experimental changes** where you're exploring what works

## When STATE.md Is Overkill

1. Single-file edits
2. Quick typo fixes
3. Simple queries ("what does this function do?")
4. One-command tasks

---

## Philosophical Fit

The Geometry OS Font Toolkit is built on the idea that **visual patterns can carry semantic meaning**. STATE.md extends this philosophy:

- **Glyphs** encode program instructions visually
- **STATE.md** encodes task progress visually (in markdown)

Both treat structured data as a "texture" that can be read, modified, and compiled into action. Using STATE.md for this project isn't just practical - it's conceptually coherent.
