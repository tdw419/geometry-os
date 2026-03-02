# Tone Opcode Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the Sound System by adding the `tone` opcode to the Visual IDE palette so users can draw audio programs.

**Architecture:** The Sound System already exists (SoundSystem.js, SpirvRunner.js integration, executor.wgsl OP_TONE). Only the UI palette is missing. We add 'tone' to the instructions array with manual glyph info since no visual glyph exists in the atlas.

**Tech Stack:** JavaScript, Web Audio API, existing Geometry OS infrastructure

---

## Current State Analysis

### What Exists (Verified)

| Component | File | Status |
|-----------|------|--------|
| SoundSystem.js | web/SoundSystem.js | ✅ Web Audio integration |
| SpirvRunner.js | web/SpirvRunner.js | ✅ Processes RAM[123-126] |
| executor.wgsl | web/executor.wgsl | ✅ OP_TONE (opcode 201) |
| VisualCompiler.js | web/VisualCompiler.js | ✅ tone opcode (0x82) |
| **demo.html palette** | web/demo.html:303 | ❌ Missing 'tone' |

### Memory Map for Sound

| Address | Purpose |
|---------|---------|
| RAM[123] | Volume (0.0-1.0) |
| RAM[124] | Duration (seconds) |
| RAM[125] | Frequency (Hz) |
| RAM[126] | Trigger flag |

### Stack Behavior

```
Stack before: [..., freq, dur, vol]
tone opcode pops 3 values
Stack after: [...]
Result: Sound plays
```

---

### Task 1: Add Tone to Palette

**Files:**
- Modify: `web/demo.html:303`

**Step 1: Add 'tone' to instructions array**

Change line 303 from:
```javascript
const instructions = ['+', '-', '*', '/', '>', '<', 'sin', 'cos', 'st', 'ld', '?', ':', 'rect', 'clr'];
```

To:
```javascript
const instructions = ['+', '-', '*', '/', '>', '<', 'sin', 'cos', 'st', 'ld', '?', ':', 'rect', 'clr', 'tone'];
```

**Step 2: Update createPaletteItem to handle 'tone'**

The existing code at line 305-310 handles special labels for '?' and ':'. Add handling for 'tone':

```javascript
instructions.forEach(char => {
    let label = char;
    if (char === '?') label = 'jnz';
    if (char === ':') label = 'lbl';
    if (char === 'tone') {
        // Tone has no glyph in atlas, use manual info
        createPaletteItem('tone', { g: 0x82, b: 0 });
        return;
    }
    const info = font.getGlyphInfo(char);
    createPaletteItem(label, info);
});
```

**Step 3: Verify in browser**

1. Start server: `cd web && python3 -m http.server 8770`
2. Open http://localhost:8770/demo.html
3. Check palette has 'tone' button
4. Click 'tone' - should highlight when selected

**Step 4: Commit**

```bash
git add web/demo.html
git commit -m "feat: add tone opcode to Visual IDE palette

Completes Sound System integration by exposing the tone opcode
in the UI. Users can now draw audio programs.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 2: Add Sound Constants to Palette

**Files:**
- Modify: `web/demo.html:312`

**Step 1: Add useful sound constants**

Change line 312 from:
```javascript
[0, 1, 10, 20, 50, 100, 127, 128].forEach(val => {
```

To:
```javascript
[0, 1, 10, 20, 50, 100, 127, 128, 440, 880, 0.5, 1.0].forEach(val => {
```

This adds:
- 440 - A4 note frequency
- 880 - A5 note frequency
- 0.5 - Half second duration
- 1.0 - Full volume

**Step 2: Verify in browser**

1. Refresh http://localhost:8770/demo.html
2. Check palette has new constant buttons

**Step 3: Commit**

```bash
git add web/demo.html
git commit -m "feat: add sound-related constants to palette

Adds 440Hz (A4), 880Hz (A5), 0.5s duration, and 1.0 volume
constants for easier audio program creation.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 3: Integration Test

**Files:**
- None (manual testing)

**Step 1: Create test program**

In the Visual IDE:
1. Select '440' (frequency)
2. Click grid cell
3. Select '0.5' (duration)
4. Click next cell in Hilbert path
5. Select '1' (volume)
6. Click next cell
7. Select 'tone'
8. Click next cell

**Step 2: Run and verify**

1. Click "Compile & Run IDE Program"
2. Expected: Hear 440Hz tone for 0.5 seconds
3. Check RAM Monitor - RAM[123-126] should update

**Step 3: Document test result**

```bash
git add -A
git commit -m "test: verify tone opcode integration

Manual test: 440Hz tone plays correctly when program executed.
Sound System fully functional.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Verification Checklist

- [ ] 'tone' button appears in palette
- [ ] Clicking 'tone' selects it (highlights)
- [ ] Drawing 'tone' on grid works
- [ ] Program with 440, 0.5, 1, tone compiles
- [ ] Running program plays sound
- [ ] RAM Monitor shows sound addresses updating
- [ ] All commits made with proper messages

---

## Future Enhancements (Out of Scope)

- Add visual glyph for 'tone' to font atlas
- Add ADSR envelope support
- Add multiple oscillator types (sine, square, sawtooth)
- Add multi-voice polyphony
