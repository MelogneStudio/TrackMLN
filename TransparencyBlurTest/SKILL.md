---
name: layered-hwnd-transparency
description: Implement and maintain genuine per-pixel window transparency in a WinUI 3 (C#) app using a native Win32 layered HWND driven by UpdateLayeredWindow. Use whenever the task involves per-pixel alpha, translucent windows, click-through regions, UpdateLayeredWindow, WS_EX_LAYERED, premultiplied BGRA, or "the desktop must show through" in this codebase.
---

# Layered-HWND Per-Pixel Transparency (WinUI 3 / C#)

## The core discovery (do not relitigate this)

A normal WinUI 3 `Window` renders its XAML through a compositor-backed swap-chain surface
that is **opaque at the HWND boundary**. Setting a XAML `Background` to `#26000000` only
darkens the WinUI surface — the real desktop does NOT show through. Adding `WS_EX_LAYERED`
to a XAML window alone is also not enough.

**The only method that yields genuine per-pixel transparency here** is a native Win32
**layered HWND** fed by `UpdateLayeredWindow`. The app supplies a 32-bit premultiplied
BGRA bitmap with independent alpha per pixel; DWM composites those pixels directly against
the desktop.

```
Pure WinUI/XAML Window  → compositor surface → opaque native window boundary
Native layered HWND     → app supplies BGRA + per-pixel alpha → DWM composites vs desktop
```

Reference implementation: `LayeredTestWindow.cs`. Launched from `App.xaml.cs`'s
`OnLaunched`, which calls the layered window directly instead of a XAML `MainWindow`.

## Non-negotiable rules

1. **Bitmap is premultiplied BGRA, top-down.**
   - Byte order per pixel: `[B, G, R, A]`.
   - **Premultiplied** means every color channel must be `channel * alpha / 255`.
     - Opaque white: `(255,255,255,255)`.
     - `#000000` at alpha 38: `(0,0,0,38)` — the zero color already satisfies premultiplication.
     - `#FFFFFF` at alpha 128 would be `(128,128,128,128)`, NOT `(255,255,255,128)`.
     Getting this wrong produces bright fringes / washed-out edges.
   - Top-down bitmap: set `biHeight = -Height` in `BITMAPINFOHEADER`.

2. **Call `UpdateLayeredWindow` with `AC_SRC_ALPHA` and `SourceConstantAlpha = 255`.**
   `BLENDFUNCTION{ BlendOp=AC_SRC_OVER(0), BlendFlags=0, SourceConstantAlpha=255, AlphaFormat=AC_SRC_ALPHA(1) }`
   and flags `ULW_ALPHA (2)`. This makes Windows use the bitmap's per-pixel alpha directly.

3. **NEVER use `SetLayeredWindowAttributes` with a global alpha** on this window. It applies
   one opacity to the whole window, which would make the intended-opaque regions translucent
   too. It is mutually exclusive with the `UpdateLayeredWindow` path.

4. **Window style:** `WS_POPUP | WS_VISIBLE`, extended style `WS_EX_LAYERED`. No caption/frame
   — the frame would be an opaque non-client area you don't control.

## Resource & lifetime management (leaks here are silent)

Per paint/update, in a `try/finally`:
- `GetDC(NULL)` → screen DC; `CreateCompatibleDC` → memory DC; `CreateDIBSection` → bitmap.
- `SelectObject` the bitmap in, keep the returned old bitmap.
- After `UpdateLayeredWindow`: `SelectObject` old bitmap back, then `DeleteObject(bitmap)`,
  `DeleteDC(memoryDc)`, `ReleaseDC(NULL, screenDc)`. Missing any of these leaks GDI objects
  (hard cap ~10k/process → eventual failure).
- Keep the `WndProc` delegate in a **static field** so the GC can't collect it while native
  code still holds the pointer (see `WindowProcedure` in the reference).
- Register the window class once; treat error `1410` (already registered) as success.
- Handle `WM_DESTROY` → `PostQuitMessage`.

## Prefer a persistent DIB for animated / frequently-updated windows

The reference recreates the DIB every paint (fine for a static test). For a real UI that
redraws often, allocate the `CreateDIBSection` **once**, write pixels into its `bits`
pointer each frame, and call `UpdateLayeredWindow` again — recreating GDI objects per frame
is slow and leak-prone.

## Blur is NOT available on this path

`UpdateLayeredWindow` composites your exact pixels; DWM never samples what's behind the
window, so you **cannot** blur the desktop through a ULW window. Frosted-glass blur requires
Acrylic/Mica `SystemBackdrop` or `DwmEnableBlurBehindWindow`, which is the opaque-compositor
path. You get **either** arbitrary per-pixel alpha (ULW) **or** blur (backdrop) per surface,
never both in one pass. Choose per window/region.

## Verification checklist before claiming it works

- [ ] Build the WinUI 3 project (net10.0-windows target).
- [ ] Launch; confirm the desktop / another window is genuinely visible through the
      translucent region (move a bright window behind it — you should see it change).
- [ ] Confirm the intended-opaque region stays 100% opaque (not tinted by the background alpha).
- [ ] Check no GDI leak: repeated updates shouldn't grow the GDI object count (Task Manager
      → Details → GDI objects column).
