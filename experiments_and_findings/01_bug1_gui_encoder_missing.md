# Experiment & Findings: Bug 1 - GUI Missing Encoder Option

## 1. Problem Description
The `hevc_vaapi` encoder was added to the backend in `host/src/encoders.rs` and `host/src/capture.rs`, allowing the daemon and CLI to successfully encode HEVC frames via Intel Arc VA-API. However, the graphical user interface (`uscreen-gui`) did not list `hevc_vaapi` in the encoder dropdown. When `encoder = "hevc_vaapi"` was present in `~/.config/uscreen/config.toml`, the GUI fell through to the `other => other` match arm, displaying the raw string without a friendly label, and users could not select it from the dropdown.

## 2. Code Inspection & Root Cause
Located in [gui/src/main.rs](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/gui/src/main.rs#L571-L602):

```rust
ui.label("Encoder");
egui::ComboBox::from_id_salt("encoder")
    .selected_text(match self.cfg.encoder.as_str() {
        "h264_nvenc" => "NVIDIA H.264 (NVENC)",
        "hevc_nvenc" => "NVIDIA HEVC (NVENC)",
        "h264_vaapi" | "vaapih264enc" => "AMD / Intel (VAAPI)",
        "libx264" => "CPU (libx264)",
        other => other,
    })
    .show_ui(ui, |ui| {
        ui.selectable_value(
            &mut self.cfg.encoder,
            "h264_nvenc".to_string(),
            "NVIDIA H.264 (NVENC)",
        );
        ui.selectable_value(
            &mut self.cfg.encoder,
            "hevc_nvenc".to_string(),
            "NVIDIA HEVC (NVENC)",
        );
        ui.selectable_value(
            &mut self.cfg.encoder,
            "h264_vaapi".to_string(),
            "AMD / Intel (VAAPI)",
        );
        ui.selectable_value(
            &mut self.cfg.encoder,
            "libx264".to_string(),
            "CPU (libx264)",
        );
    });
```

Notice that:
1. `hevc_vaapi` was completely missing from `selected_text`.
2. `hevc_vaapi` was completely missing from the selectable values in `show_ui`.
3. `h264_vaapi` was labelled `"AMD / Intel (VAAPI)"` which is ambiguous when multiple VA-API codecs exist. It should be `"AMD / Intel H.264 (VAAPI)"` and `"AMD / Intel HEVC (VAAPI)"`.
4. Downstream in line 707, `let hevc = self.cfg.encoder.contains("hevc");` enables 10-bit mode whenever the encoder contains `"hevc"`. Adding `hevc_vaapi` will also properly enable the 10-bit checkbox in the GUI for VA-API HEVC.

## 3. Planned Patch
Update [gui/src/main.rs](file:///home/harsh/Documents/GITHUB%20PROJECTS/UScreen/gui/src/main.rs) to:
- Map `"hevc_vaapi"` in `selected_text` to `"AMD / Intel HEVC (VAAPI)"`.
- Map `"h264_vaapi"` to `"AMD / Intel H.264 (VAAPI)"`.
- Add `ui.selectable_value(&mut self.cfg.encoder, "hevc_vaapi".to_string(), "AMD / Intel HEVC (VAAPI)");` to the dropdown menu.
