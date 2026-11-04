# Institutional Deep-Dive Investigation & Root-Cause Summary

**Project**: UScreen  
**Platform**: Ubuntu 24.04 LTS (GNOME 46 / Wayland)  
**Hardware**: Intel Core Ultra (Meteor Lake-P), Intel Arc Graphics (`i915`), Samsung Galaxy Tab S9 FE (`SM-X616B`)  
**Investigator**: Principal Systems Engineer & Lead Developer  
**Status**: Investigation Complete — Root Causes Formally Identified and Documented  

---

## 1. Executive Summary

Four distinct bugs severely degraded the user experience of UScreen on Intel Meteor Lake hardware running GNOME Wayland:
1. **Bug 1 (GUI Missing Encoder)**: `hevc_vaapi` hardware encoding worked in the CLI/daemon but was omitted from the `uscreen-gui` dropdown.
2. **Bug 2 (System Freeze on Tablet Plug-in)**: A kernel DRM race condition between `evdi` and Intel `i915` caused hard system freezes upon connecting the tablet.
3. **Bug 3 (Directional Lag & Ghost Cursor)**: Moving the mouse from tablet to PC caused severe compositor stutter, locked tablet FPS to 1, and burned a ghost cursor into the tablet screen.
4. **Bug 4 (Stubborn Meta-0 Monitor)**: A persistent `1024x600` phantom monitor persisted across sessions due to a legacy systemd override.

All four issues have been deeply investigated, traced to root causes in code and system configuration, and verified with unedited raw kernel logs and D-Bus state inspections.

---

## 2. Root Cause & Solution Matrix

| Bug | Root Cause | File / System Target | Remediation Strategy |
| :--- | :--- | :--- | :--- |
| **Bug 1: GUI Missing Encoder** | ComboBox hardcoded match arms in [gui/src/main.rs](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/gui/src/main.rs) omitted `"hevc_vaapi"`. | `gui/src/main.rs` | Add `"hevc_vaapi" => "AMD / Intel HEVC (VAAPI)"` to `selected_text` and selectable values. |
| **Bug 2: Desktop Freeze on Plug-in** | Missing `softdep evdi pre: drm_kms_helper i915` and missing EVDI udev rule allowed DRM mutex deadlock during hotplug. | `/etc/modprobe.d/evdi.conf`<br>`/etc/udev/rules.d/99-uscreen-evdi.rules` | Enforce module load ordering via `softdep` and assign `uaccess` tag to EVDI cards. |
| **Bug 3: Directional Lag & Ghost Cursor** | 1. 250ms watchdog in `evdi_helper.c` forcibly cleared `g_update_pending` and grabbed pixels without `update_ready`, causing 4 Hz kernel ioctl flood and Mutter lock contention.<br>2. Idle screen starvation dropped FIFO writes to 1-5 FPS.<br>3. Software cursor left cursor burned in framebuffer when Mutter stopped painting idle display.<br>4. `pointer_area = None` on GNOME warped cursor across entire desktop on pen lift. | `host/evdi/evdi_helper.c`<br>`host/src/input.rs`<br>`/etc/environment` | 1. Restructure EVDI event loop: never grab or re-request while a kernel request is pending.<br>2. Maintain smooth keepalive frame pacing from `g_latest` when idle.<br>3. Call `evdi_enable_cursor_events(handle, false)`.<br>4. Calculate true desktop bounds for GNOME pointer handoff. |
| **Bug 4: Stubborn Meta-0 Monitor** | `~/.config/systemd/user/org.gnome.Shell@wayland.service.d/persistent-virtual-monitor.conf` passed `--virtual-monitor 1024x600`. | `org.gnome.Shell@wayland.service.d`<br>Mutter D-Bus `ApplyMonitorsConfig` | Deleted override directory, reloaded systemd user daemon, dynamically removed `Meta-0` via D-Bus without reboot. |

---

## 3. Detailed Investigation Reports
- [Bug 1: GUI Missing Encoder Option](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/experiments_and_findings/01_bug1_gui_encoder_missing.md)
- [Bug 2: System Freeze on Tablet Plug-in](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/experiments_and_findings/02_bug2_evdi_intel_freeze.md)
- [Bug 3: Directional Lag and Ghost Cursor](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/experiments_and_findings/03_bug3_directional_lag_ghost_cursor.md)
- [Bug 4: Stubborn Meta-0 Virtual Monitor](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/experiments_and_findings/04_bug4_meta0_virtual_monitor.md)

---

## 4. Next Implementation Steps
1. Apply GUI patch in `gui/src/main.rs`.
2. Apply `evdi_helper.c` watchdog and event loop fix.
3. Apply `input.rs` GNOME pointer area mapping fix.
4. Prepare system configuration files for EVDI modprobe and udev.
5. Compile with `make build` and `cargo build --release`.
6. Deploy binaries to `target/release`, `~/.local/bin`, and system paths.
7. Restart UScreen service and verify zero kernel warnings and smooth bidirectional cursor motion.
