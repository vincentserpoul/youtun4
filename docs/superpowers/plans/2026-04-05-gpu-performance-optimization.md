# GPU Performance Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce GPU usage of the synthwave UI while preserving the visual aesthetic.

**Architecture:** All changes are CSS-only in `crates/youtun4-ui/styles/main.css`. Each task targets one category of GPU-expensive effect: background filters, sun effects, backdrop-filters, logo shadows, and box-shadow/hover effects.

**Tech Stack:** CSS (no Rust changes)

---

### Task 1: Remove Grid Floor Filters

**Files:**
- Modify: `crates/youtun4-ui/styles/main.css:146-166`

- [ ] **Step 1: Remove filter and will-change from grid floor**

In `.grid-floor::before` (lines 146-166), change the block to remove the `filter` and `will-change` properties, and reduce grid line opacity from `0.7` to `0.5`:

```css
.grid-floor::before {
  content: "";
  position: absolute;
  left: -50%;
  right: -50%;
  bottom: 0;
  height: 100%;
  background:
    linear-gradient(to right, rgba(250, 196, 255, 0.5) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(250, 196, 255, 0.5) 1px, transparent 1px);
  background-size: 2rem 2rem;
  transform: rotateX(53deg)
    translateX(calc(var(--mouse-x, 0) * -10px));
  transform-origin: 50% 100%;
  animation: grid-scroll 16s linear infinite;
  /* Fade out at the top — grid converges to vanishing point */
  -webkit-mask-image: linear-gradient(to bottom, transparent 0%, #000 15%);
  mask-image: linear-gradient(to bottom, transparent 0%, #000 15%);
}
```

Changes from original:
- Removed `filter: drop-shadow(0 0 4px rgba(223, 115, 115, 0.8)) blur(0.3px);`
- Removed `will-change: background-position;`
- Changed `rgba(250, 196, 255, 0.7)` to `rgba(250, 196, 255, 0.5)` in both gradients

- [ ] **Step 2: Verify visually**

Run: `cargo tauri dev`

Expected: Grid floor still recedes to vanishing point with parallax and scrolling animation. Lines are slightly softer but no pink glow filter. No visual regression on the perspective effect.

- [ ] **Step 3: Commit**

```bash
git add crates/youtun4-ui/styles/main.css
git commit -m "perf: remove expensive filter and will-change from grid floor"
```

---

### Task 2: Simplify Synthwave Sun

**Files:**
- Modify: `crates/youtun4-ui/styles/main.css:70-128`

- [ ] **Step 1: Make sun glow use a pre-blurred gradient instead of filter**

Replace `.synthwave-sun::before` (lines 70-85) with a gradient that has softer, more diffuse stops to simulate the blur natively:

```css
/* Sun glow — large blurred halo behind the sun */
.synthwave-sun::before {
  content: "";
  position: absolute;
  inset: -60%;
  border-radius: 50%;
  background: radial-gradient(
    circle,
    rgba(253, 180, 40, 0.35) 0%,
    rgba(253, 180, 40, 0.25) 10%,
    rgba(246, 114, 202, 0.3) 18%,
    rgba(246, 114, 202, 0.2) 28%,
    rgba(185, 153, 255, 0.18) 35%,
    rgba(185, 153, 255, 0.1) 45%,
    rgba(139, 92, 246, 0.06) 55%,
    rgba(139, 92, 246, 0.02) 65%,
    transparent 75%
  );
  z-index: -1;
}
```

Changes from original:
- Removed `filter: blur(25px);`
- Doubled the number of gradient stops with intermediate opacity values to simulate the soft blur

- [ ] **Step 2: Remove sun body animation and filter**

In `.synthwave-sun` (lines 88-123), remove the `filter` and `animation` properties:

```css
/* Synthwave retro sun — gradient circle with static dark bands */
.synthwave-sun {
  position: absolute;
  z-index: 2;
  top: 18%;
  left: 50%;
  transform: translateX(-50%);
  width: min(28vmin, 220px);
  aspect-ratio: 1;
  background: linear-gradient(to bottom, #fdb428 0%, #f672ca 60%);
  border-radius: 50%;
  /* Variable-width bands via mask — thinner at bottom, wider at top */
  mask: linear-gradient(
    to top,
    #0000 0%, 0%, #000 2%, 0%,
    #0000 6%, 0%, #000 9%, 0%,
    #0000 14%, 0%, #000 18%, 0%,
    #0000 24%, 0%, #000 29%, 0%,
    #0000 35%, 0%, #000 42%, 0%,
    #0000 48%, 0%, #000 56%, 0%,
    #0000 60%, 0%, #000 62%
  );
  -webkit-mask: linear-gradient(
    to top,
    #0000 0%, 0%, #000 2%, 0%,
    #0000 6%, 0%, #000 9%, 0%,
    #0000 14%, 0%, #000 18%, 0%,
    #0000 24%, 0%, #000 29%, 0%,
    #0000 35%, 0%, #000 42%, 0%,
    #0000 48%, 0%, #000 56%, 0%,
    #0000 60%, 0%, #000 62%
  );
  mask-size: 100% 250%;
  -webkit-mask-size: 100% 250%;
}
```

Changes from original:
- Removed `filter: blur(0.5px);`
- Removed `animation: sun-bands-scroll 48s linear infinite;`

- [ ] **Step 3: Remove the now-unused sun-bands-scroll keyframes**

Delete lines 125-128:

```css
/* DELETE THIS BLOCK */
@keyframes sun-bands-scroll {
  from { mask-position: 0 0; -webkit-mask-position: 0 0; }
  to { mask-position: 0 250%; -webkit-mask-position: 0 250%; }
}
```

- [ ] **Step 4: Verify visually**

Run: `cargo tauri dev`

Expected: Sun still shows orange-to-pink gradient with horizontal band slices. Bands are static (no scrolling). Glow behind sun is still visible but rendered via gradient stops instead of blur filter.

- [ ] **Step 5: Commit**

```bash
git add crates/youtun4-ui/styles/main.css
git commit -m "perf: remove sun animation and replace glow blur with native gradient"
```

---

### Task 3: Replace Backdrop Filters with Solid Backgrounds

**Files:**
- Modify: `crates/youtun4-ui/styles/main.css` — lines 215-217, 4374-4376, 6163-6165, 6377-6379

- [ ] **Step 1: Replace sidebar backdrop-filter**

At lines 215-217 in `.layout-sidebar`, replace:

```css
  background: rgba(6, 3, 16, 0.5);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
```

with:

```css
  background: rgba(6, 3, 16, 0.88);
```

- [ ] **Step 2: Replace playlist detail panel backdrop-filter**

At lines 4374-4376 in `.playlist-detail-panel`, replace:

```css
  background: rgba(6, 3, 16, 0.5);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
```

with:

```css
  background: rgba(6, 3, 16, 0.88);
```

- [ ] **Step 3: Replace connected devices bar backdrop-filter**

At lines 6163-6165 in `.connected-devices-bar`, replace:

```css
  background: rgba(6, 3, 16, 0.75);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
```

with:

```css
  background: rgba(6, 3, 16, 0.92);
```

- [ ] **Step 4: Replace playlist table container backdrop-filter**

At lines 6377-6379 in `.playlist-table-container`, replace:

```css
  background: rgba(6, 3, 16, 0.5);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
```

with:

```css
  background: rgba(6, 3, 16, 0.88);
```

- [ ] **Step 5: Verify the overlay blur(2px) is untouched**

Confirm `.layout-overlay` at line 5161 still has `backdrop-filter: blur(2px)` — this one stays.

- [ ] **Step 6: Verify visually**

Run: `cargo tauri dev`

Expected: Sidebar, detail panel, device bar, and table container still appear as dark semi-transparent panels. Slightly less "frosted glass" feel but barely noticeable on the dark theme. Modal overlay still has subtle blur.

- [ ] **Step 7: Commit**

```bash
git add crates/youtun4-ui/styles/main.css
git commit -m "perf: replace backdrop-filter blur with solid semi-transparent backgrounds"
```

---

### Task 4: Replace Logo Drop-Shadows

**Files:**
- Modify: `crates/youtun4-ui/styles/main.css:455-468`

- [ ] **Step 1: Replace filter drop-shadows with text-shadow**

At lines 465-467 in the `.logo h1` rule (the rule containing `background-clip: text`), replace:

```css
  filter:
    drop-shadow(0 0 8px rgba(255, 45, 138, 0.9))
    drop-shadow(0 0 25px rgba(236, 72, 153, 0.5));
```

with:

```css
  text-shadow: 0 0 10px rgba(255, 45, 138, 0.7);
```

Note: `text-shadow` works alongside `background-clip: text` in WebKit-based renderers (Tauri uses WebKit on macOS, Chromium on Windows/Linux). The glow appears behind the clipped text.

- [ ] **Step 2: Verify visually**

Run: `cargo tauri dev`

Expected: Logo still has a pink neon glow. Slightly less intense than the double drop-shadow but still reads as synthwave.

- [ ] **Step 3: Commit**

```bash
git add crates/youtun4-ui/styles/main.css
git commit -m "perf: replace logo double drop-shadow filter with text-shadow"
```

---

### Task 5: Replace Hover/State Drop-Shadow Filters with Box-Shadows

**Files:**
- Modify: `crates/youtun4-ui/styles/main.css` — lines 611, 1048, 5864, 6537, 6548

- [ ] **Step 1: Replace btn-icon hover drop-shadow**

At line 611 in `.btn-icon:hover:not(:disabled)`, replace:

```css
  filter: drop-shadow(0 0 8px currentColor);
```

with:

```css
  box-shadow: 0 0 8px currentColor;
```

- [ ] **Step 2: Replace playlist-header hover drop-shadow**

At line 1048 in the playlist header hover rule, replace:

```css
  filter: drop-shadow(0 2px 8px var(--shadow-primary));
```

with:

```css
  box-shadow: 0 2px 8px var(--shadow-primary);
```

- [ ] **Step 3: Replace sync button hover drop-shadow**

At line 5864, replace:

```css
  filter: drop-shadow(0 0 10px rgba(236, 72, 153, 0.7));
```

with:

```css
  box-shadow: 0 0 10px rgba(236, 72, 153, 0.7);
```

- [ ] **Step 4: Replace status badge drop-shadows**

At line 6537, replace:

```css
  filter: drop-shadow(0 0 6px rgba(236, 72, 153, 0.5));
```

with:

```css
  box-shadow: 0 0 6px rgba(236, 72, 153, 0.5);
```

At line 6548, replace:

```css
  filter: drop-shadow(0 0 6px rgba(248, 113, 113, 0.5));
```

with:

```css
  box-shadow: 0 0 6px rgba(248, 113, 113, 0.5);
```

- [ ] **Step 5: Verify visually**

Run: `cargo tauri dev`

Expected: All hover/state glows look identical — box-shadow produces the same visual result for rectangular elements.

- [ ] **Step 6: Commit**

```bash
git add crates/youtun4-ui/styles/main.css
git commit -m "perf: replace hover drop-shadow filters with box-shadow equivalents"
```

---

### Task 6: Cap Large Box-Shadow Blur Radii

**Files:**
- Modify: `crates/youtun4-ui/styles/main.css` — lines 220-222, 553, 681, 4379, 5118, 6275, 6345, 6350, 6383

- [ ] **Step 1: Cap sidebar box-shadow**

At lines 220-222 in `.layout-sidebar`, replace:

```css
  box-shadow:
    0 0 20px rgba(34, 211, 238, 0.08),
    0 0 45px rgba(34, 211, 238, 0.04);
```

with:

```css
  box-shadow:
    0 0 12px rgba(34, 211, 238, 0.08),
    0 0 12px rgba(34, 211, 238, 0.04);
```

- [ ] **Step 2: Cap btn-primary hover box-shadow**

At line 553 in `.btn-primary:hover:not(:disabled)`, replace:

```css
  box-shadow: 0 0 20px rgba(236, 72, 153, 0.5), 0 0 40px rgba(139, 92, 246, 0.3);
```

with:

```css
  box-shadow: 0 0 12px rgba(236, 72, 153, 0.5), 0 0 12px rgba(139, 92, 246, 0.3);
```

- [ ] **Step 3: Cap btn-primary active box-shadow**

At line 681 in the `.btn-primary.active` rule, replace:

```css
  box-shadow: 0 0 24px var(--shadow-primary), inset 0 0 16px rgba(139, 92, 246, 0.05);
```

with:

```css
  box-shadow: 0 0 12px var(--shadow-primary), inset 0 0 8px rgba(139, 92, 246, 0.05);
```

- [ ] **Step 4: Cap playlist detail panel box-shadow**

At line 4379 in `.playlist-detail-panel`, replace:

```css
  box-shadow: 0 0 20px rgba(34, 211, 238, 0.08);
```

with:

```css
  box-shadow: 0 0 12px rgba(34, 211, 238, 0.08);
```

- [ ] **Step 5: Cap mobile sidebar box-shadow**

At line 5118, replace:

```css
    box-shadow: 4px 0 24px rgba(0, 0, 0, 0.5);
```

with:

```css
    box-shadow: 4px 0 12px rgba(0, 0, 0, 0.5);
```

- [ ] **Step 6: Cap connected status dot box-shadow**

At line 6275 in `.cdb-status-dot.connected`, replace:

```css
  box-shadow: 0 0 8px rgba(0, 255, 136, 0.8), 0 0 20px rgba(0, 255, 136, 0.4);
```

with:

```css
  box-shadow: 0 0 8px rgba(0, 255, 136, 0.8), 0 0 12px rgba(0, 255, 136, 0.4);
```

- [ ] **Step 7: Cap sync button box-shadows**

At line 6345 in `.btn-execute-sync`, replace:

```css
  box-shadow: 0 2px 12px rgba(236, 72, 153, 0.4), 0 0 25px rgba(139, 92, 246, 0.3);
```

with:

```css
  box-shadow: 0 2px 12px rgba(236, 72, 153, 0.4), 0 0 12px rgba(139, 92, 246, 0.3);
```

At line 6350 in `.btn-execute-sync:hover:not(:disabled)`, replace:

```css
  box-shadow: 0 4px 25px rgba(236, 72, 153, 0.6), 0 0 40px rgba(139, 92, 246, 0.4);
```

with:

```css
  box-shadow: 0 4px 12px rgba(236, 72, 153, 0.6), 0 0 12px rgba(139, 92, 246, 0.4);
```

- [ ] **Step 8: Cap playlist table container box-shadow**

At line 6383 in `.playlist-table-container`, replace:

```css
  box-shadow: 0 0 20px rgba(34, 211, 238, 0.08);
```

with:

```css
  box-shadow: 0 0 12px rgba(34, 211, 238, 0.08);
```

- [ ] **Step 9: Verify visually**

Run: `cargo tauri dev`

Expected: Glow effects are slightly tighter but still clearly visible. The synthwave neon vibe is preserved.

- [ ] **Step 10: Commit**

```bash
git add crates/youtun4-ui/styles/main.css
git commit -m "perf: cap box-shadow blur radii at 12px across all elements"
```
