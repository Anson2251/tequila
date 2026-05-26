# Graphics Backend UI/UX Design

## Overview

Graphics backends involve two distinct user journeys:

1. **Global: Install/Remove** — Download and manage graphics backends at the application level (Settings window)
2. **Per-prefix: Activate/Deactivate** — Apply an installed backend to a specific Wine prefix

The document specifies the UI for both journeys, covering all three backends (DXMT, D3DMetal, DXVK+VKD3D).

---

## 1. Global Backend Management (Settings)

*Already implemented in `ui/src/settings/graphics.rs` — minor adjustments noted here for consistency.*

### Settings → Graphics Backends

```
┌─────────────────────────────────────────────────────┐
│  Tequila Settings                     ✕ (close)     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ═══ General ═══                                    │
│                                                     │
│  ┌─────────────────────────────────────────────────┐│
│  │ Wine Runtime          GHC 4.21 · 3 runtimes  >  ││
│  └─────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────┐│
│  │ Graphics Backends    dxmt-1.0 · d3dmetal-1.0  > ││
│  └─────────────────────────────────────────────────┘│
│                                                     │
│  ═══ GStreamer ═══                                  │
│  ┌─────────────────────────────────────────────────┐│
│  │ GStreamer                 ✓ Installed (1.24.x)  ││
│  └─────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────┘
```

Tapping "Graphics Backends" pushes a subpage:

```
┌─────────────────────────────────────────────────────┐
│  ◀ (back)  Graphics Backends                        │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Each row is a ManagedDownloadRow (already built):   │
│                                                     │
│  ┌─────────────────────────────────────────────────┐│
│  │ DXMT                                     ✓      ││
│  │ DirectX → Metal translation layer                ││
│  │ (recommended)                          [Remove]  ││
│  └─────────────────────────────────────────────────┘│
│                                                     │
│  ┌─────────────────────────────────────────────────┐│
│  │ D3DMetal (via GPTK)                      ✓      ││
│  │ Apple's Game Porting Toolkit            [Remove] ││
│  └─────────────────────────────────────────────────┘│
│                                                     │
│  ── macOS only ── Currently as-is.                  │
│  ── Linux shows DXVK+VKD3D instead.                 │
│                                                     │
└─────────────────────────────────────────────────────┘
```

No significant changes needed here — this part works.