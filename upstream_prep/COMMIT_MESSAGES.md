# Suggested Squashed Commit History for Upstream Maintainer

If you prefer merging these changes as a series of clean, logical, atomic commits rather than a single merge commit, here is the recommended 7-commit squashed structure:

---

### Commit 1: Hardware HEVC (H.265) Encoding & VAAPI Color Pipeline Fix
```text
feat(encoder): add hevc_vaapi hardware encoding with zero-copy pipeline

- Add hevc_vaapi to allowed encoder registry, CLI args, and GUI dropdown selector.
- Correct FFmpeg color metadata flag placement: position -color_primaries bt709
  and -colorspace bt709 prior to input flag -i to prevent FFmpeg 7+ from
  auto-inserting CPU swscale conversion filters on incoming EVDI NV12 frames.
- Add graceful fallback from hevc_vaapi to h264_vaapi upon initialization error.
- Add support for 10-bit Main10 profile encoding on supported VAAPI hardware.
```

---

### Commit 2: Linux Kernel EVDI Hotplug & Mutter Cursor Synchronization
```text
fix(drm): eliminate EVDI hotplug mutex deadlock and ghost cursor freezing

- Provide packaging/uscreen-evdi.conf enforcing softdep ordering:
  softdep evdi pre: drm_kms_helper i915 to prevent DRM driver initialization races.
- Provide packaging/99-uscreen-evdi.rules applying uaccess tag to EVDI DRM devices.
- Disable EVDI software cursor plane burning via evdi_enable_cursor_events(handle, false).
- Implement Dynamic Boundary Synchronization Engine using a lightweight 1x1 X11
  nudge window to cleanly flush Mutter's cursor plane when crossing display edges.
- Fix desktop coordinate boundaries in host/src/input.rs for GNOME Wayland pointer handoff.
```

---

### Commit 3: Android Client Opt-In Immersive Mode
```text
feat(android): add opt-in immersive edge-to-edge display mode

- Implement edge-to-edge full screen rendering utilizing WindowInsetsControllerCompat.
- Handle camera notches and panel cutouts cleanly using LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS.
- Intercept system back gestures to toggle the settings bottom sheet without disconnecting.
- Default immersive mode to disabled in Prefs.kt to guarantee backwards compatibility.
```

---

### Commit 4: High-Resolution Frame Pacing & Max Performance Mode
```text
feat(pacing): stabilize system-wide frame pacing and add Max Performance Mode

- Replace imprecise sleeps in evdi_helper.c with POSIX monotonic high-resolution timers.
- Prevent Mutter display starvation by serving keepalive frames during screen idle periods.
- Add max_performance_mode configuration toggle in config.toml, CLI, and GUI.
- When power profile is "performance", sustain continuous 60/90 FPS stream even when
  pointer operates exclusively on the primary monitor.
```

---

### Commit 5: Low-Latency HEVC Tuning & Exynos Codec2 Deadlock Fix
```text
perf(hevc): implement low-latency VAAPI tuning and eliminate Exynos Codec2 deadlock

- Apply low-latency FFmpeg parameters (-b_depth 1 -sei 0 -aud 0 -async_depth 1) in CQP mode.
- Inject low-latency decoder parameters (KEY_LATENCY=0, KEY_LOW_LATENCY=1, and Exynos vendor keys).
- Fix Android VideoReceiver surface lifecycle: cleanly drain and release MediaCodec
  buffers prior to surface destruction to prevent hardware decoder hangs.
- Add microsecond-resolution timing probe (/dev/shm/uscreen_timing_0) for pipeline instrumentation.
```

---

### Commit 6: Multi-Device UX Architecture Redesign & Full Per-Device Overrides
```text
feat(gui): overhaul multi-device management with sidebar navigation and full overrides

- Redesign uscreen-gui architecture from monolithic scroll view to a dedicated
  left-sidebar navigation layout (Global Settings vs individual connected/saved devices).
- Extend TabletProfile/DeviceProfile in host/src/config.rs with nickname, ten_bit,
  and alias support (#[serde(alias = "devices")]).
- Implement full per-device configuration overrides across all properties:
  encoder, fps, quality, bitrate, 10-bit, stream_scale, position, and resolution.
- Add user-editable nicknames in GUI with automatic fallback to ADB model name.
- Transition UI terminology from "Tablet" to generic "Device" semantics.
- Add pinned bottom action bar with persistent Apply & restart / Discard controls.
- Prevent active device overrides from overwriting global defaults in config.toml.
- Stabilize uscreen-gui against accesskit/zbus executor panics by scoping native rendering.
```

---

### Commit 7: Documentation & Lab Findings
```text
docs: add comprehensive laboratory notebooks, empirical benchmarks, and traces

- Add 12 detailed academic laboratory reports in experiments_and_findings/ covering
  investigations, root-cause analyses, kernel traces, and latency profiling data.
- Include verification screenshots and empirical benchmark scripts.
- Document system configuration recommendations for hybrid graphics setups.
```
