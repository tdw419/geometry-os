# Ouroboros Project Onboarding

Welcome to this Ouroboros self-improving project!

---

## Quick Start

### 1. Check Provider Status
```bash
./queue.sh check
```

Make sure at least one provider is available (🟢).

### 2. Login to OAuth Providers (if needed)
```bash
./queue.sh login gemini
./queue.sh login claude
```

### 3. Start the Dashboard
```bash
./queue.sh dashboard --watch
```

### 4. Read the Roadmap
```bash
cat ROADMAP.md
```

---

## Your Mission

Improve this codebase autonomously by:

1. **Reading** `ROADMAP.md` to understand current phase and goals
2. **Reading** `.ouroboros/goal.yaml` to understand target metrics
3. **Checking** `./queue.sh status` to see provider availability
4. **Enqueuing** prompts via `./queue.sh enqueue "your prompt"`
5. **Processing** the queue via `./queue.sh process`

---

## Queue Manager Commands

```bash
./queue.sh dashboard           # Visual dashboard
./queue.sh dashboard --watch   # Live dashboard
./queue.sh status              # Provider status
./queue.sh enqueue "prompt"    # Add to queue
./queue.sh process             # Process queue
./queue.sh clear all           # Clear queue
```

---

## Safety

- Protected files are listed in `.ouroboros/safety.yaml`
- All changes must pass tests
- Auto-rollback is enabled

---

## Files

```
.ouroboros/
├── goal.yaml          # Target metrics and success criteria
├── safety.yaml        # Safety configuration
├── queue/             # Persistent queue state
│   ├── prompt_queue.json
│   └── rate_limits.json
└── v2/                # V2 specific files

ROADMAP.md             # Strategic roadmap
PROMPT_ONBOARDING.md   # This file
queue.sh               # Queue manager CLI
```

---

Please confirm you understand the project structure and are ready to begin.
