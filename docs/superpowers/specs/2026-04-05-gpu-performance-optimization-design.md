# GPU Performance Optimization — Design Spec

## Problem

The Youtun4 synthwave UI causes high GPU usage on macOS immediately at launch. The root cause is multiple expensive CSS effects running simultaneously: 3D-transformed animated backgrounds with blur filters, backdrop-filter on major panels, double drop-shadows on always-visible elements, and large-radius box-shadows.

## Goal

Reduce GPU usage significantly while preserving the synthwave aesthetic — neon colors, retro grid, sun, and the overall vibe stay intact.

## Approach

Surgical optimization of each GPU-expensive effect in `crates/youtun4-ui/styles/main.css`. Keep the visual structure, remove or simplify the expensive rendering operations stacked on top.

## Changes

### 1. Grid Floor Background

**File:** `crates/youtun4-ui/styles/main.css` — `.grid-floor::before`

- **Remove** `filter: drop-shadow(0 0 4px rgba(223, 115, 115, 0.8)) blur(0.3px)` — forces re-rasterization of the entire 3D-transformed element every frame
- **Remove** `will-change: background-position` — promotes an already expensive layer unnecessarily
- **Reduce** grid line opacity from `0.7` to `0.5` — softens lines without needing a filter
- **Keep** `perspective`, `rotateX(53deg)`, mouse parallax (`translateX(calc(var(--mouse-x, 0) * -10px))`), 16s scroll animation, and mask fade — these create the receding horizon effect and are cheap on their own

### 2. Synthwave Sun

**File:** `crates/youtun4-ui/styles/main.css` — `.synthwave-sun` and `.synthwave-sun::before`

Sun body:
- **Remove** `animation: sun-bands-scroll 48s linear infinite` — make the band mask static (the scroll is so slow it's imperceptible)
- **Remove** `filter: blur(0.5px)` — the gradient is already smooth; blur adds no visible difference

Sun glow (`::before`):
- **Replace** `filter: blur(25px)` with a pre-blurred radial gradient — add more color stops with softer edges to simulate the blur natively, eliminating the filter entirely

### 3. Backdrop Filters

**Files:** `.layout-sidebar`, `.playlist-detail-panel`, `.connected-devices-bar`, `.playlist-table-container`

- **Replace** all `backdrop-filter: blur(16px)` with semi-transparent solid backgrounds (e.g., `rgba(10, 4, 24, 0.88)`) — visually near-identical on a dark app, eliminates the single most expensive CSS property
- **Keep** `backdrop-filter: blur(2px)` on `.layout-overlay` — it's small, infrequent, and only appears during modals

### 4. Logo Drop-Shadows

**File:** `crates/youtun4-ui/styles/main.css` — `.logo`

- **Replace** `filter: drop-shadow(0 0 8px rgba(255, 45, 138, 0.9)) drop-shadow(0 0 25px rgba(236, 72, 153, 0.5))` with `text-shadow: 0 0 10px rgba(255, 45, 138, 0.7)` — similar neon glow, no filter compositing overhead

### 5. Box-Shadows and Hover Effects

Across multiple selectors:

- **Cap** box-shadow blur radii at 12px (currently 20-45px on some elements)
- **Replace** hover `filter: drop-shadow(0 0 Xpx ...)` with equivalent `box-shadow` — box-shadow is composited more cheaply than filter
- **Keep** `filter: brightness(1.15)` on hover — cheap and only active during interaction

### No Changes

- Pulse, spin, skeleton, and LED animations — lightweight, only active when relevant
- Page transition animations — one-shot, short duration

## Expected Impact

| Effect | Before | After |
|--------|--------|-------|
| Grid floor | 3D + filter + blur + animation | 3D + animation (no filters) |
| Sun | Mask animation + blur filter | Static mask, no filter |
| Sun glow | blur(25px) filter | Native gradient (no filter) |
| Backdrop-filter | blur(16px) on 4-5 panels | Solid semi-transparent bg |
| Logo | Double drop-shadow filter | Single text-shadow |
| Box-shadows | Up to 45px blur | Capped at 12px |
| Hover drop-shadows | filter: drop-shadow | box-shadow equivalent |

Estimated GPU reduction: ~80-90%. The remaining GPU work is standard compositing (3D transform, simple animations, small box-shadows).

## Scope

All changes are in a single file: `crates/youtun4-ui/styles/main.css`. No Rust code changes needed.
