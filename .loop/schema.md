# STATE.md Schema

Geometry OS RGB Encoding for Auto-Prompting Loop State

## Overview

STATE.md uses the Geometry OS RGB encoding metaphor to represent agent state:
- **R (Red)**: Context - What we know, what's been done
- **G (Green)**: Action - What to do next
- **B (Blue)**: Target - Where to apply the action

## Schema

```markdown
# STATE: RUNNING | DONE

## R: Context
- **Goal**: [what we're building]
- **Progress**: [what's done]
- **Files**: [key files modified]
- **Blockers**: [any blockers]

## G: Action
[ACTION]: [specific next step]

## B: Target
target: [file path or command]
content: |
  [content if WRITE/EDIT]
```

## Actions

| Action | Purpose | Example |
|--------|---------|---------|
| READ | Read file contents | `READ: Check current implementation in src/main.py` |
| WRITE | Create new file | `WRITE: Create new utility module` |
| EDIT | Modify existing file | `EDIT: Add error handling to function` |
| RUN | Execute command | `RUN: Run tests to verify changes` |
| ANALYZE | Analyze codebase/state | `ANALYZE: Review project structure` |
| DONE | Signal completion | `DONE: Task completed successfully` |

## Status Values

- **RUNNING**: Task is in progress, loop should continue
- **DONE**: Task completed, loop should terminate

## Termination Signal

Loop terminates when `DONE:` appears in the G: Action section:

```markdown
## G: Action
DONE: All objectives achieved
```

## Example State

```markdown
# STATE: RUNNING

## R: Context
- **Goal**: Create hello.txt with greeting message
- **Progress**: Task initialized, ready to write file
- **Files**: None yet
- **Blockers**: None

## G: Action
WRITE: Create hello.txt with specified content

## B: Target
target: hello.txt
content: |
  Hello from Geometry OS!
```

## File Size Limit

STATE.md should remain under 50KB (NFR-3). Claude should summarize progress
rather than include full file contents or verbose history.

## Atomic Writes

STATE.md should be written atomically using temp file + rename pattern to
prevent corruption during crashes (FR-9, NFR-4).
