# Geometry OS Auto-Prompting Loop

A single-agent auto-prompting loop that uses RGB encoding principles for state management.

## Quick Start

```bash
# 1. Initialize a task
python .loop/start.py "Create hello.txt with content 'Hello from Geometry OS!'"

# 2. Run the loop
python .loop/runner.py
```

## Commands

### start.py - Initialize Task

Creates STATE.md and TASK.md with initial state.

```bash
python .loop/start.py "<task-description>"
```

**Arguments:**
- `<task-description>` - What you want Claude to accomplish

**Example:**
```bash
python .loop/start.py "Add error handling to src/main.py"
```

**Output:**
- `.loop/STATE.md` - Current state with R/G/B encoding
- `.loop/TASK.md` - Task description and timestamp

**Error Handling:**
- Fails if a task is already RUNNING (prevents accidental overwrite)
- To clear existing task: `rm .loop/STATE.md .loop/TASK.md`

### runner.py - Execute Loop

Reads STATE.md, invokes Claude, repeats until DONE.

```bash
python .loop/runner.py
```

**Behavior:**
- Max 100 iterations (configurable in code)
- 10-minute timeout per iteration
- Exits 0 on task completion
- Exits 1 on max iterations exceeded
- Exits 130 on Ctrl+C interrupt

## RGB Encoding

STATE.md uses Geometry OS RGB encoding to represent agent state:

| Channel | Name    | Contains                           |
|---------|---------|-------------------------------------|
| R       | Context | Goal, progress, files, blockers    |
| G       | Action  | Current action: READ, WRITE, etc.  |
| B       | Target  | File path, command, or content     |

### Example STATE.md

```markdown
# STATE: RUNNING

## R: Context
- **Goal**: Create hello.txt with greeting
- **Progress**: Task initialized
- **Files**: None yet
- **Blockers**: None

## G: Action
ANALYZE: Check if hello.txt exists

## B: Target
target: hello.txt
```

## Available Actions

| Action  | Purpose                    | Example                              |
|---------|----------------------------|--------------------------------------|
| READ    | Read file contents         | `READ: Check src/main.py`           |
| WRITE   | Create new file            | `WRITE: Create utils module`        |
| EDIT    | Modify existing file       | `EDIT: Add error handling`          |
| RUN     | Execute shell command      | `RUN: python -m pytest tests/`      |
| ANALYZE | Analyze without changes    | `ANALYZE: Review project structure` |
| DONE    | Signal completion          | `DONE: Task completed`              |

## Loop Flow

1. **start.py** creates initial STATE.md with `ANALYZE` action
2. **runner.py** reads STATE.md, invokes Claude
3. Claude executes action, updates STATE.md
4. Loop repeats until `DONE:` appears in G: Action
5. runner.py exits with code 0

## File Structure

```
.loop/
  README.md     # This file
  schema.md     # STATE.md schema specification
  SYSTEM.md     # Claude loop instructions
  start.py      # Task initialization
  runner.py     # Main loop runner
  STATE.md      # Current state (created by start.py)
  TASK.md       # Task description (created by start.py)
```

## Requirements

- Python 3.6+
- Claude CLI installed and configured
- No external Python dependencies (stdlib only)

## Troubleshooting

**"A task is already running"**
```bash
rm .loop/STATE.md .loop/TASK.md
```

**Loop not terminating**
- Check STATE.md for `DONE:` in G: Action section
- Max 100 iterations enforced

**Claude not responding**
- Check Claude CLI is installed: `claude --version`
- Check timeout (10 min default)

## Testing

The loop includes a comprehensive test suite with unit, integration, and verification tests.

### Running Tests

```bash
# Run all tests
cd .loop && ./tests/run_tests.sh

# Run specific test categories
./tests/run_tests.sh --unit          # Unit tests only (fast)
./tests/run_tests.sh --integration   # Integration tests only (requires Claude CLI)
./tests/run_tests.sh --verification  # Verification tests only
./tests/run_tests.sh --quick         # Skip slow integration tests

# Run with pytest directly
python -m pytest tests/ -v
```

### Test Categories

| Category | File | Description | Timeout |
|----------|------|-------------|---------|
| Unit | `test_runner.py` | Tests for `is_done()`, `get_clean_env()`, `atomic_write()` | 60s |
| Integration | `test_integration.py` | Full loop tests with real Claude CLI | 300s |
| Verification | `test_verification.py` | Cross-agent validation, schema checks | 120s |

### Test Fixtures

| Fixture | Location | Description |
|---------|----------|-------------|
| `temp_loop_dir` | `conftest.py` | Temporary loop directory for isolated tests |
| `sample_state` | `conftest.py` | Sample STATE.md content with RGB encoding |
| `done_state` | `conftest.py` | STATE.md content indicating DONE action |

### Test Files

```
.loop/tests/
  __init__.py           # Package marker
  conftest.py           # Pytest fixtures
  test_runner.py        # Unit tests for runner.py functions
  test_integration.py   # Integration tests with Claude CLI
  test_verification.py  # Cross-agent validation tests
  run_tests.sh          # Test runner script with summary
```

### Requirements for Testing

- Python 3.6+
- pytest: `pip install pytest pytest-timeout`
- Claude CLI (for integration tests)

### CI Integration

The test runner exits with code 0 on success, 1 on failure:

```bash
./tests/run_tests.sh && echo "All tests passed" || echo "Tests failed"
```
