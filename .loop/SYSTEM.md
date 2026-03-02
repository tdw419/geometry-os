# SYSTEM.md: Claude Loop Instructions

## Loop Behavior

You are running in an auto-prompting loop. Each iteration:
1. Read STATE.md to understand current context
2. Execute the action specified in G: Action
3. Update STATE.md with progress and next action
4. The loop continues until DONE: appears in G: Action

## RGB Encoding (Geometry OS)

STATE.md uses RGB channels to represent agent state:

| Channel | Meaning | Contains |
|---------|---------|----------|
| **R (Red)** | Context | Goal, progress, files modified, blockers |
| **G (Green)** | Action | Current action opcode + description |
| **B (Blue)** | Target | File path, command, or content |

## Actions

Execute exactly one action per iteration:

### READ
Read file contents to gather information.
```
G: Action
READ: Check current implementation in src/main.py

B: Target
target: src/main.py
```

### WRITE
Create a new file with specified content.
```
G: Action
WRITE: Create new utility module

B: Target
target: src/utils.py
content: |
  def helper():
      pass
```

### EDIT
Modify an existing file.
```
G: Action
EDIT: Add error handling to process function

B: Target
target: src/main.py
content: |
  # Add try/except block around process()
```

### RUN
Execute a shell command.
```
G: Action
RUN: Run tests to verify changes

B: Target
target: python -m pytest tests/
```

### ANALYZE
Analyze codebase or state without file changes.
```
G: Action
ANALYZE: Review project structure for missing modules
```

### DONE
Signal task completion. Include summary.
```
G: Action
DONE: Created hello.txt with greeting message
```

## Completion Signal

When the task is complete, set G: Action to DONE: with a summary:

```markdown
## G: Action
DONE: All objectives achieved - created hello.txt with greeting
```

The loop terminates automatically when DONE: is detected.

## Rules

1. **No questions**: Never ask for clarification. Make reasonable assumptions.

2. **Always update state**: After each action, update STATE.md:
   - R: Context - add progress, files modified
   - G: Action - set next action or DONE:
   - B: Target - set target for next action

3. **One action per iteration**: Execute only what's in G: Action, then update state.

4. **Atomic writes**: When writing files, use temp file + rename for crash safety.

5. **Summarize progress**: Keep R: Context concise. Don't include full file contents.

6. **Document blockers**: If stuck, add to R: Context blockers section.

7. **No external dependencies**: Only use Python standard library and Claude tools.

## Example Loop Flow

**Initial STATE.md:**
```markdown
# STATE: RUNNING

## R: Context
- **Goal**: Create hello.txt with greeting
- **Progress**: Task initialized
- **Files**: None
- **Blockers**: None

## G: Action
ANALYZE: Check if hello.txt already exists

## B: Target
target: hello.txt
```

**After ANALYZE (file doesn't exist):**
```markdown
# STATE: RUNNING

## R: Context
- **Goal**: Create hello.txt with greeting
- **Progress**: Analyzed - hello.txt does not exist
- **Files**: None
- **Blockers**: None

## G: Action
WRITE: Create hello.txt with greeting message

## B: Target
target: hello.txt
content: |
  Hello from Geometry OS!
```

**After WRITE:**
```markdown
# STATE: RUNNING

## R: Context
- **Goal**: Create hello.txt with greeting
- **Progress**: Created hello.txt
- **Files**: hello.txt
- **Blockers**: None

## G: Action
DONE: Created hello.txt with greeting message

## B: Target
target: hello.txt
```

Loop terminates on DONE: detection.
