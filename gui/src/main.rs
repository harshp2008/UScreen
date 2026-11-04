#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What one device reported about itself and per-device overrides.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
struct TabletProfile {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    pub width: u32,
    pub height: u32,
    pub width_mm: u32,
    pub height_mm: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_scale: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ten_bit: Option<bool>,
    #[serde(flatten)]
    pub extra: toml::value::Table,
}

impl TabletProfile {
    pub fn display_name<'a>(&'a self, fallback: &'a str) -> &'a str {
        if let Some(ref n) = self.nickname {
            let trimmed = n.trim();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
        if !self.label.trim().is_empty() {
            return self.label.trim();
        }
        fallback
    }

    pub fn has_display(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Mirror of the daemon's config file (~/.config/uscreen/config.toml).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
struct FileConfig {
    encoder: String,
    fps: u32,
    bitrate: u32,
    width: u32,
    height: u32,
    quality: u32,
    stream_scale: u32,
    pen_only: bool,
    position: String,
    ten_bit: bool,
    require_token: bool,
    check_updates: bool,
    max_tablets: u32,
    wifi_address: String,
    vaapi_device: String,
    #[serde(default, alias = "devices")]
    tablets: std::collections::HashMap<String, TabletProfile>,
    pointer_handoff: bool,
    auto_resolution: bool,
    video_port: u16,
    input_port: u16,
    auto_launch_app: bool,
    max_performance_mode: bool,
    hevc_low_latency_tuning: bool,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            encoder: "h264_nvenc".into(),
            fps: 60,
            bitrate: 20000,
            width: 2960,
            height: 1848,
            quality: DEFAULT_QUALITY,
            stream_scale: 1,
            pen_only: false,
            position: "right".into(),
            ten_bit: false,
            require_token: true,
            check_updates: true,
            max_tablets: 1,
            wifi_address: String::new(),
            vaapi_device: String::new(),
            tablets: std::collections::HashMap::new(),
            pointer_handoff: true,
            auto_resolution: true,
            video_port: 8890,
            input_port: 8891,
            auto_launch_app: true,
            max_performance_mode: false,
            hevc_low_latency_tuning: true,
        }
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/uscreen/config.toml")
}

const MAX_BITRATE_KBPS: u32 = 60_000;
const MIN_BITRATE_KBPS: u32 = 1_000;
const MAX_FPS: u32 = 90;
const MIN_FPS: u32 = 10;
const DEFAULT_QUALITY: u32 = 18;
const MIN_QUALITY: u32 = 12;
const MAX_QUALITY: u32 = 32;

impl FileConfig {
    fn load() -> Self {
        let mut cfg: Self = std::fs::read_to_string(config_path())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        cfg.bitrate = cfg.bitrate.clamp(MIN_BITRATE_KBPS, MAX_BITRATE_KBPS);
        cfg.fps = cfg.fps.clamp(MIN_FPS, MAX_FPS);
        cfg.quality = cfg.quality.clamp(MIN_QUALITY, MAX_QUALITY);
        cfg.stream_scale = cfg.stream_scale.clamp(1, 4);
        cfg
    }

    fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self).unwrap_or_default())
    }
}

fn autostart_enabled() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-enabled", "uscreen.service"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(false)
}

fn set_autostart(on: bool) -> Result<(), String> {
    let verb = if on { "enable" } else { "disable" };
    let out = Command::new("systemctl")
        .args(["--user", verb, "--now", "uscreen.service"])
        .output()
        .map_err(|e| format!("systemctl failed: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[derive(Default, Clone, PartialEq, Eq)]
struct ConnectedDevice {
    serial: String,
    model: String,
    transport: String,
}

#[derive(Default, Clone)]
struct Status {
    daemon_running: bool,
    daemon_pid: u32,
    device_connected: bool,
    device_model: String,
    connected_devices: Vec<ConnectedDevice>,
    /// -1 = evdi module not loaded, otherwise the device count
    evdi_count: i32,
    ffmpeg_ok: bool,
    adb_ok: bool,
    autostart: bool,
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn pid_path() -> PathBuf {
    PathBuf::from(format!("{}/.local/share/uscreen/uscreen.pid", home()))
}

fn find_uscreen_bin() -> Option<PathBuf> {
    let installed = PathBuf::from(format!("{}/.local/bin/uscreen", home()));
    if installed.exists() {
        return Some(installed);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("uscreen");
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }
    if Command::new("uscreen").arg("--version").output().is_ok() {
        return Some(PathBuf::from("uscreen"));
    }
    None
}

fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[allow(clippy::field_reassign_with_default)]
fn poll_status() -> Status {
    let mut s = Status::default();

    s.evdi_count = std::fs::read_to_string("/sys/devices/evdi/count")
        .ok()
        .and_then(|t| t.trim().parse::<i32>().ok())
        .unwrap_or(-1);
    s.ffmpeg_ok = command_exists("ffmpeg");
    s.autostart = autostart_enabled();
    s.adb_ok = command_exists("adb");

    if let Ok(pid_str) = std::fs::read_to_string(pid_path()) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            if PathBuf::from(format!("/proc/{}", pid)).exists() {
                s.daemon_running = true;
                s.daemon_pid = pid;
            }
        }
    }

    if let Ok(out) = Command::new("adb").args(["devices", "-l"]).output() {
        let text = String::from_utf8_lossy(&out.stdout);
        let ready: Vec<&str> = text
            .lines()
            .skip(1)
            .filter(|l| l.split_whitespace().nth(1) == Some("device"))
            .collect();
        for line in &ready {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let serial = parts.first().copied().unwrap_or("").to_string();
            if serial.is_empty() {
                continue;
            }
            let model = parts
                .iter()
                .find_map(|t| t.strip_prefix("model:"))
                .map(|m| m.replace('_', " "))
                .unwrap_or_else(|| serial.clone());
            let transport = if serial.contains(':') || serial.contains("._adb") {
                "Wi-Fi".to_string()
            } else {
                "USB".to_string()
            };
            s.connected_devices.push(ConnectedDevice {
                serial,
                model,
                transport,
            });
        }
        if let Some(first) = s.connected_devices.first() {
            s.device_connected = true;
            s.device_model = first.model.clone();
        }
    }
    if let Ok(fakes) = std::env::var("USCREEN_FAKE_TABLET") {
        for fake in fakes.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            if !s.connected_devices.iter().any(|t| t.serial == fake) {
                let model = format!("Emulated Device ({})", fake);
                s.connected_devices.push(ConnectedDevice {
                    serial: fake.to_string(),
                    model: model.clone(),
                    transport: "Emulated".to_string(),
                });
                s.device_connected = true;
                if s.device_model.is_empty() {
                    s.device_model = model;
                }
            }
        }
    }
    s
}

fn run_system_setup() -> Result<(), String> {
    let script = "set -e; \
        echo 'options evdi initial_device_count=2' > /etc/modprobe.d/uscreen-evdi.conf; \
        printf 'evdi\nuinput\n' > /etc/modules-load.d/uscreen.conf; \
        modprobe evdi || true; modprobe uinput || true; \
        if [ \"$(cat /sys/devices/evdi/count 2>/dev/null || echo 0)\" = \"0\" ]; then echo 1 > /sys/devices/evdi/add; fi";
    let out = Command::new("pkexec")
        .args(["sh", "-c", script])
        .output()
        .map_err(|e| format!("pkexec failed to run: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Setup failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn start_daemon() -> Result<(), String> {
    let bin = find_uscreen_bin().ok_or("uscreen binary not found — run `make install`")?;
    let log_dir = PathBuf::from(format!("{}/.local/share/uscreen", home()));
    let _ = std::fs::create_dir_all(&log_dir);
    let log = std::fs::File::create(log_dir.join("daemon.log")).map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;
    let mut child = Command::new(bin)
        .arg("start")
        .stdout(log)
        .stderr(log_err)
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start daemon: {}", e))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn stop_daemon() -> Result<(), String> {
    let bin = find_uscreen_bin().ok_or("uscreen binary not found")?;
    Command::new(bin)
        .arg("stop")
        .output()
        .map_err(|e| format!("Failed to stop daemon: {}", e))?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SelectedView {
    Global,
    Device(String),
}

struct App {
    cfg: FileConfig,
    saved_cfg: FileConfig,
    status: Arc<Mutex<Status>>,
    message: String,
    update: Arc<Mutex<Option<String>>>,
    selected_view: SelectedView,
}

const RELEASES_API: &str = "https://api.github.com/repos/majmichu1/UScreen/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/majmichu1/UScreen/releases/latest";

fn os_release_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_default()
        + " / " + &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
        + " (" + &std::env::var("XDG_SESSION_TYPE").unwrap_or_default() + ")"
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn version_parts(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn check_for_update() -> Option<String> {
    let out = Command::new("curl")
        .args(["-sS", "--max-time", "4", "-H", "Accept: application/vnd.github+json",
               "-H", concat!("User-Agent: uscreen-gui/", env!("CARGO_PKG_VERSION")), RELEASES_API])
        .output().ok()?;
    if !out.status.success() { return None; }
    let body = String::from_utf8_lossy(&out.stdout);
    let tag = body.split("\"tag_name\"").nth(1)?.split('"').nth(1)?.trim_start_matches('v').to_string();
    (version_parts(&tag) > version_parts(env!("CARGO_PKG_VERSION"))).then_some(tag)
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cfg = FileConfig::load();
        let status = Arc::new(Mutex::new(Status::default()));

        let update = Arc::new(Mutex::new(None));
        if cfg.check_updates {
            let slot = update.clone();
            std::thread::spawn(move || {
                if let Some(v) = check_for_update() {
                    if let Ok(mut g) = slot.lock() { *g = Some(v); }
                }
            });
        }

        let status_bg = status.clone();
        std::thread::spawn(move || loop {
            let s = poll_status();
            if let Ok(mut guard) = status_bg.lock() {
                *guard = s;
            }
            std::thread::sleep(Duration::from_secs(2));
        });

        Self {
            saved_cfg: cfg.clone(),
            cfg,
            status,
            message: String::new(),
            update,
            selected_view: SelectedView::Global,
        }
    }

    fn apply(&mut self, restart: bool) {
        match self.cfg.save() {
            Ok(_) => {
                self.saved_cfg = self.cfg.clone();
                self.message = "Settings saved".into();
                if restart {
                    let _ = stop_daemon();
                    std::thread::sleep(Duration::from_millis(500));
                    match start_daemon() {
                        Ok(_) => self.message = "Settings saved — daemon restarted".into(),
                        Err(e) => self.message = e,
                    }
                }
            }
            Err(e) => self.message = format!("Save failed: {}", e),
        }
    }

    fn render_global_settings(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.heading(egui::RichText::new("Global Settings").size(20.0));
        ui.label(egui::RichText::new("Default settings applied to all connected devices unless overridden.").weak().size(12.0));
        ui.add_space(10.0);

        // System setup banner if needed
        let needs_setup = status.evdi_count <= 0;
        let missing_pkgs = !status.ffmpeg_ok || !status.adb_ok;
        if needs_setup || missing_pkgs {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(50, 38, 22))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        egui::RichText::new("Setup needed")
                            .strong()
                            .color(egui::Color32::from_rgb(255, 180, 80)),
                    );
                    if missing_pkgs {
                        let mut pkgs = vec![];
                        if !status.ffmpeg_ok { pkgs.push("ffmpeg"); }
                        if !status.adb_ok { pkgs.push("android-tools (adb)"); }
                        ui.label(format!("Install with package manager: {}", pkgs.join(", ")));
                    }
                    if needs_setup {
                        ui.label(if status.evdi_count < 0 {
                            "The EVDI kernel module is not loaded (install evdi/evdi-dkms)."
                        } else {
                            "The virtual display device needs to be enabled (one time)."
                        });
                        if ui.button("Enable virtual display (asks for password)").clicked() {
                            match run_system_setup() {
                                Ok(_) => self.message = "System setup complete".into(),
                                Err(e) => self.message = e,
                            }
                        }
                    }
                });
            ui.add_space(10.0);
        }

        egui::Grid::new("global_settings_grid")
            .num_columns(2)
            .spacing([18.0, 12.0])
            .show(ui, |ui| {
                ui.label("Global Encoder");
                egui::ComboBox::from_id_salt("global_encoder")
                    .selected_text(encoder_label(&self.cfg.encoder))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.cfg.encoder, "h264_nvenc".to_string(), "NVIDIA H.264 (NVENC)");
                        ui.selectable_value(&mut self.cfg.encoder, "hevc_nvenc".to_string(), "NVIDIA HEVC (NVENC)");
                        ui.selectable_value(&mut self.cfg.encoder, "h264_vaapi".to_string(), "AMD / Intel H.264 (VAAPI)");
                        ui.selectable_value(&mut self.cfg.encoder, "hevc_vaapi".to_string(), "AMD / Intel HEVC (VAAPI)");
                        ui.selectable_value(&mut self.cfg.encoder, "libx264".to_string(), "CPU (libx264)");
                    });
                ui.end_row();

                ui.label("Quality");
                ui.vertical(|ui| {
                    let mut sharpness = (MAX_QUALITY - self.cfg.quality.clamp(MIN_QUALITY, MAX_QUALITY)) as f32;
                    let span = (MAX_QUALITY - MIN_QUALITY) as f32;
                    if ui.add(egui::Slider::new(&mut sharpness, 0.0..=span).show_value(false)).changed() {
                        self.cfg.quality = MAX_QUALITY - sharpness.round() as u32;
                    }
                    ui.label(egui::RichText::new("This, not the bitrate, sets how sharp text looks").weak().size(11.0));
                });
                ui.end_row();

                ui.label("Bitrate ceiling");
                ui.vertical(|ui| {
                    let mut mbps = self.cfg.bitrate as f32 / 1000.0;
                    if ui.add(egui::Slider::new(&mut mbps, 5.0..=60.0).suffix(" Mbps")).changed() {
                        self.cfg.bitrate = (mbps * 1000.0) as u32;
                    }
                    ui.label(egui::RichText::new("Only a cap for bursts — desktop streams well below it").weak().size(11.0));
                });
                ui.end_row();

                ui.label("Frame rate");
                egui::ComboBox::from_id_salt("global_fps")
                    .selected_text(format!("{} fps", self.cfg.fps))
                    .show_ui(ui, |ui| {
                        for f in [30u32, 60, 90] {
                            ui.selectable_value(&mut self.cfg.fps, f, format!("{} fps", f));
                        }
                    });
                ui.end_row();

                ui.label("Colour depth");
                ui.vertical(|ui| {
                    let hevc = self.cfg.encoder.contains("hevc");
                    ui.add_enabled(hevc, egui::Checkbox::new(&mut self.cfg.ten_bit, "10-bit (HEVC Main10)"));
                    ui.label(egui::RichText::new(if hevc {
                        "Smooths banding on gradients."
                    } else {
                        "Needs an HEVC encoder — H.264 is 8-bit only."
                    }).small().weak());
                });
                ui.end_row();

                ui.label("Resolution");
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.cfg.auto_resolution, "Auto (match the device)");
                    ui.horizontal(|ui| {
                        ui.add_enabled(!self.cfg.auto_resolution, egui::DragValue::new(&mut self.cfg.width).range(640..=8192).speed(8));
                        ui.label("×");
                        ui.add_enabled(!self.cfg.auto_resolution, egui::DragValue::new(&mut self.cfg.height).range(480..=8192).speed(8));
                    });
                    if self.cfg.auto_resolution {
                        ui.label(egui::RichText::new(format!("currently {} × {}", self.cfg.width, self.cfg.height)).weak().size(11.0));
                    }
                });
                ui.end_row();

                ui.label("Stream detail");
                ui.vertical(|ui| {
                    egui::ComboBox::from_id_salt("global_stream_scale")
                        .selected_text(match self.cfg.stream_scale {
                            1 => "Full — sharpest",
                            2 => "Half — lowest latency",
                            n => scale_label(n),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.cfg.stream_scale, 1, "Full — sharpest");
                            ui.selectable_value(&mut self.cfg.stream_scale, 2, "Half — lowest latency");
                        });
                    ui.label(egui::RichText::new("Half sends 1/4 the pixels, decoding faster with lower latency.").weak().size(11.0));
                });
                ui.end_row();

                ui.label("Position");
                egui::ComboBox::from_id_salt("global_position")
                    .selected_text(match self.cfg.position.as_str() {
                        "left" => "Left of the other screens",
                        "above" => "Above the other screens",
                        "below" => "Below the other screens",
                        _ => "Right of the other screens",
                    })
                    .show_ui(ui, |ui| {
                        for (v, label) in [
                            ("right", "Right of the other screens"),
                            ("left", "Left of the other screens"),
                            ("above", "Above the other screens"),
                            ("below", "Below the other screens"),
                        ] {
                            ui.selectable_value(&mut self.cfg.position, v.to_string(), label);
                        }
                    });
                ui.end_row();

                ui.label("Devices");
                ui.vertical(|ui| {
                    ui.add(egui::Slider::new(&mut self.cfg.max_tablets, 1..=4).text("at once"));
                    ui.label(egui::RichText::new("Each device becomes its own screen. Needs matching EVDI virtual count.").small().weak());
                });
                ui.end_row();

                ui.label("Stylus Mode");
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.cfg.pen_only, "Graphics tablet mode instead of display");
                    ui.label(egui::RichText::new("No video streamed: pen/touch directly drives the host display with zero latency.").weak().size(11.0));
                });
                ui.end_row();

                ui.label("Security");
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.cfg.require_token, "Require session token");
                    ui.label(egui::RichText::new("Protects loopback ports from unauthorized local access.").small().weak());
                });
                ui.end_row();

                ui.label("Updates");
                ui.checkbox(&mut self.cfg.check_updates, "Check for newer release on start");
                ui.end_row();

                ui.label("Plug & play");
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.cfg.auto_launch_app, "Open app on the device automatically");
                    ui.checkbox(&mut self.cfg.pointer_handoff, "Leave mouse cursor where pen lifted");
                    let mut auto = status.autostart;
                    if ui.checkbox(&mut auto, "Start UScreen with the desktop").changed() {
                        match set_autostart(auto) {
                            Ok(_) => {
                                self.message = if auto {
                                    "Autostart on — plugging the cable in is now enough".into()
                                } else {
                                    "Autostart off".into()
                                }
                            }
                            Err(e) => self.message = e,
                        }
                    }
                });
                ui.end_row();

                ui.label("Performance");
                ui.vertical(|ui| {
                    ui.checkbox(
                        &mut self.cfg.max_performance_mode,
                        "Max Performance Mode (Force 60FPS on Performance Profile)",
                    ).on_hover_text(
                        "Eliminates idle FPS throttling when system power profile is 'performance'.",
                    );
                });
                ui.end_row();
            });
    }

    fn render_device_settings(&mut self, ui: &mut egui::Ui, serial: &str, status: &Status) {
        let conn = status.connected_devices.iter().find(|t| t.serial == serial);
        let is_connected = conn.is_some();
        let transport = conn.map(|t| t.transport.as_str()).unwrap_or("Offline");
        let fallback_model = conn.map(|t| t.model.as_str()).unwrap_or(serial);

        // Snapshot global defaults before mutably borrowing device profile
        let global_encoder = self.cfg.encoder.clone();
        let global_fps = self.cfg.fps;
        let global_quality = self.cfg.quality;
        let global_bitrate = self.cfg.bitrate;
        let global_ten_bit = self.cfg.ten_bit;
        let global_stream_scale = self.cfg.stream_scale;
        let global_position = self.cfg.position.clone();

        let profile = self.cfg.tablets.entry(serial.to_string()).or_default();
        if profile.label.is_empty() && conn.is_some() {
            profile.label = fallback_model.to_string();
        }

        let display_title = profile.display_name(fallback_model).to_string();

        ui.heading(egui::RichText::new(format!("Device: {}", display_title)).size(20.0));
        ui.label(egui::RichText::new("Configure per-device parameters and overrides.").weak().size(12.0));
        ui.add_space(8.0);

        // Hardware & Identity Card
        egui::Frame::group(ui.style())
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    status_dot(ui, is_connected, if is_connected { format!("Connected ({})", transport) } else { "Saved Profile (Offline)".to_string() }.as_str(), "");
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Serial:").weak().size(11.0));
                    ui.label(egui::RichText::new(serial).strong().size(11.0));
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Hardware Model:").weak().size(11.0));
                    ui.label(egui::RichText::new(if profile.label.is_empty() { fallback_model } else { &profile.label }).size(11.0));
                    if profile.has_display() {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(format!("Panel: {}×{} ({}×{} mm)", profile.width, profile.height, profile.width_mm, profile.height_mm)).weak().size(11.0));
                    }
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Custom Nickname:").strong());
                    let mut nick = profile.nickname.clone().unwrap_or_default();
                    let hint = if profile.label.is_empty() { fallback_model } else { &profile.label };
                    if ui.add(egui::TextEdit::singleline(&mut nick).hint_text(hint).desired_width(220.0)).changed() {
                        if nick.trim().is_empty() {
                            profile.nickname = None;
                        } else {
                            profile.nickname = Some(nick);
                        }
                    }
                });
            });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Per-Device Overrides").strong().size(15.0));
        ui.label(egui::RichText::new("Check 'Override' to customize a setting specifically for this device. Unchecked settings follow Global Settings.").weak().size(11.0));
        ui.add_space(8.0);

        egui::Grid::new(format!("device_grid_{}", serial))
            .num_columns(2)
            .spacing([18.0, 12.0])
            .show(ui, |ui| {
                // 1. Encoder Override
                ui.label("Encoder");
                ui.vertical(|ui| {
                    let mut is_overridden = profile.encoder.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Encoder").changed() {
                        profile.encoder = if is_overridden { Some(global_encoder.clone()) } else { None };
                    }
                    if let Some(ref mut enc) = profile.encoder {
                        egui::ComboBox::from_id_salt(format!("dev_enc_{}", serial))
                            .selected_text(encoder_label(enc))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(enc, "h264_nvenc".to_string(), "NVIDIA H.264 (NVENC)");
                                ui.selectable_value(enc, "hevc_nvenc".to_string(), "NVIDIA HEVC (NVENC)");
                                ui.selectable_value(enc, "h264_vaapi".to_string(), "AMD / Intel H.264 (VAAPI)");
                                ui.selectable_value(enc, "hevc_vaapi".to_string(), "AMD / Intel HEVC (VAAPI)");
                                ui.selectable_value(enc, "libx264".to_string(), "CPU (libx264)");
                            });
                    } else {
                        ui.label(egui::RichText::new(format!("Using Global ({})", encoder_label(&global_encoder))).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 2. Frame Rate Override
                ui.label("Frame rate");
                ui.vertical(|ui| {
                    let mut is_overridden = profile.fps.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Frame Rate").changed() {
                        profile.fps = if is_overridden { Some(global_fps) } else { None };
                    }
                    if let Some(ref mut fps) = profile.fps {
                        egui::ComboBox::from_id_salt(format!("dev_fps_{}", serial))
                            .selected_text(format!("{} fps", fps))
                            .show_ui(ui, |ui| {
                                for f in [30u32, 60, 90] {
                                    ui.selectable_value(fps, f, format!("{} fps", f));
                                }
                            });
                    } else {
                        ui.label(egui::RichText::new(format!("Using Global ({} fps)", global_fps)).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 3. Quality Override
                ui.label("Quality");
                ui.vertical(|ui| {
                    let mut is_overridden = profile.quality.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Quality").changed() {
                        profile.quality = if is_overridden { Some(global_quality) } else { None };
                    }
                    if let Some(ref mut q) = profile.quality {
                        let mut sharpness = (MAX_QUALITY - (*q).clamp(MIN_QUALITY, MAX_QUALITY)) as f32;
                        let span = (MAX_QUALITY - MIN_QUALITY) as f32;
                        if ui.add(egui::Slider::new(&mut sharpness, 0.0..=span).show_value(false)).changed() {
                            *q = MAX_QUALITY - sharpness.round() as u32;
                        }
                    } else {
                        ui.label(egui::RichText::new(format!("Using Global (Quality {})", global_quality)).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 4. Bitrate Ceiling Override
                ui.label("Bitrate ceiling");
                ui.vertical(|ui| {
                    let mut is_overridden = profile.bitrate.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Bitrate Ceiling").changed() {
                        profile.bitrate = if is_overridden { Some(global_bitrate) } else { None };
                    }
                    if let Some(ref mut br) = profile.bitrate {
                        let mut mbps = *br as f32 / 1000.0;
                        if ui.add(egui::Slider::new(&mut mbps, 5.0..=60.0).suffix(" Mbps")).changed() {
                            *br = (mbps * 1000.0) as u32;
                        }
                    } else {
                        ui.label(egui::RichText::new(format!("Using Global ({} Mbps)", global_bitrate as f32 / 1000.0)).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 5. Colour Depth (10-bit) Override
                ui.label("Colour depth");
                ui.vertical(|ui| {
                    let active_enc = profile.encoder.as_ref().unwrap_or(&global_encoder);
                    let hevc = active_enc.contains("hevc");
                    let mut is_overridden = profile.ten_bit.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Colour Depth").changed() {
                        profile.ten_bit = if is_overridden { Some(global_ten_bit) } else { None };
                    }
                    if let Some(ref mut tb) = profile.ten_bit {
                        ui.add_enabled(hevc, egui::Checkbox::new(tb, "10-bit (HEVC Main10)"));
                        if !hevc {
                            ui.label(egui::RichText::new("Needs an HEVC encoder override or global HEVC").small().weak());
                        }
                    } else {
                        let global_desc = if global_ten_bit { "10-bit (HEVC Main10)" } else { "8-bit" };
                        ui.label(egui::RichText::new(format!("Using Global ({})", global_desc)).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 6. Stream Detail Override
                ui.label("Stream detail");
                ui.vertical(|ui| {
                    let mut is_overridden = profile.stream_scale.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Stream Detail").changed() {
                        profile.stream_scale = if is_overridden { Some(global_stream_scale) } else { None };
                    }
                    if let Some(ref mut scale) = profile.stream_scale {
                        egui::ComboBox::from_id_salt(format!("dev_scale_{}", serial))
                            .selected_text(match *scale {
                                1 => "Full — sharpest",
                                2 => "Half — lowest latency",
                                n => scale_label(n),
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(scale, 1, "Full — sharpest");
                                ui.selectable_value(scale, 2, "Half — lowest latency");
                            });
                    } else {
                        let global_scale_desc = match global_stream_scale {
                            1 => "Full — sharpest",
                            2 => "Half — lowest latency",
                            n => scale_label(n),
                        };
                        ui.label(egui::RichText::new(format!("Using Global ({})", global_scale_desc)).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 7. Position Override
                ui.label("Position");
                ui.vertical(|ui| {
                    let mut is_overridden = profile.position.is_some();
                    if ui.checkbox(&mut is_overridden, "Override Screen Position").changed() {
                        profile.position = if is_overridden { Some(global_position.clone()) } else { None };
                    }
                    if let Some(ref mut pos) = profile.position {
                        egui::ComboBox::from_id_salt(format!("dev_pos_{}", serial))
                            .selected_text(match pos.as_str() {
                                "left" => "Left of the other screens",
                                "above" => "Above the other screens",
                                "below" => "Below the other screens",
                                _ => "Right of the other screens",
                            })
                            .show_ui(ui, |ui| {
                                for (v, label) in [
                                    ("right", "Right of the other screens"),
                                    ("left", "Left of the other screens"),
                                    ("above", "Above the other screens"),
                                    ("below", "Below the other screens"),
                                ] {
                                    ui.selectable_value(pos, v.to_string(), label);
                                }
                            });
                    } else {
                        let pos_desc = match global_position.as_str() {
                            "left" => "Left of the other screens",
                            "above" => "Above the other screens",
                            "below" => "Below the other screens",
                            _ => "Right of the other screens",
                        };
                        ui.label(egui::RichText::new(format!("Using Global ({})", pos_desc)).weak().size(11.0));
                    }
                });
                ui.end_row();

                // 8. Custom Resolution
                ui.label("Custom Resolution");
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut profile.width).range(0..=8192).speed(8));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut profile.height).range(0..=8192).speed(8));
                    });
                    ui.label(egui::RichText::new("0 × 0 uses device auto-negotiated panel resolution.").weak().size(11.0));
                });
                ui.end_row();
            });

        let mut remove_profile = false;
        if !is_connected {
            ui.add_space(20.0);
            if ui.button(egui::RichText::new("Forget / Delete Saved Profile").color(egui::Color32::from_rgb(240, 100, 100))).clicked() {
                remove_profile = true;
            }
        }

        if remove_profile {
            self.cfg.tablets.remove(serial);
            self.selected_view = SelectedView::Global;
        }
    }
}

fn encoder_label(enc: &str) -> &'static str {
    match enc {
        "h264_nvenc" => "NVIDIA H.264 (NVENC)",
        "hevc_nvenc" => "NVIDIA HEVC (NVENC)",
        "h264_vaapi" | "vaapih264enc" => "AMD / Intel H.264 (VAAPI)",
        "hevc_vaapi" => "AMD / Intel HEVC (VAAPI)",
        "libx264" => "CPU (libx264)",
        _ => "Custom",
    }
}

fn scale_label(n: u32) -> &'static str {
    match n {
        3 => "Third — very soft",
        _ => "Quarter — very soft",
    }
}

fn status_dot(ui: &mut egui::Ui, on: bool, label: &str, detail: &str) {
    ui.horizontal(|ui| {
        let color = if on {
            egui::Color32::from_rgb(76, 175, 80)
        } else {
            egui::Color32::from_rgb(120, 120, 130)
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, color);
        ui.label(egui::RichText::new(label).strong());
        if !detail.is_empty() {
            ui.label(egui::RichText::new(detail).weak());
        }
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1));
        let status = self.status.lock().map(|s| s.clone()).unwrap_or_default();

        // 1. Top Panel: Header with Daemon status & PID & Start/Stop control
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("UScreen").size(18.0).strong());
                ui.label(egui::RichText::new("USB Virtual Display").weak().size(11.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_text = if status.daemon_running { "Stop Daemon" } else { "Start Daemon" };
                    if ui.button(btn_text).clicked() {
                        if status.daemon_running {
                            match stop_daemon() {
                                Ok(_) => self.message = "Daemon stopped".into(),
                                Err(e) => self.message = e,
                            }
                        } else {
                            match start_daemon() {
                                Ok(_) => self.message = "Daemon starting…".into(),
                                Err(e) => self.message = e,
                            }
                        }
                    }

                    ui.add_space(10.0);
                    status_dot(
                        ui,
                        status.daemon_running,
                        if status.daemon_running { "Running" } else { "Stopped" },
                        &if status.daemon_running { format!("(PID {})", status.daemon_pid) } else { String::new() },
                    );
                });
            });
            ui.add_space(4.0);
        });

        // 2. Bottom Panel: Footer with status messages & Apply/Discard actions
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(6.0);
            let dirty = self.cfg != self.saved_cfg;
            ui.horizontal(|ui| {
                if !self.message.is_empty() {
                    ui.label(egui::RichText::new(&self.message).color(egui::Color32::from_rgb(100, 200, 255)).size(11.0));
                } else {
                    ui.label(egui::RichText::new(format!("config: {}", config_path().display())).weak().size(10.0));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if status.daemon_running {
                        "Apply & restart"
                    } else {
                        "Save"
                    };
                    if ui.add_enabled(dirty, egui::Button::new(label)).clicked() {
                        self.apply(status.daemon_running);
                    }
                    if dirty && ui.button("Discard").clicked() {
                        self.cfg = self.saved_cfg.clone();
                    }
                });
            });
            ui.add_space(6.0);
        });

        // 3. Left Navigation Sidebar
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(220.0)
            .min_width(180.0)
            .max_width(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_space(8.0);
                    // Section 1: Navigation
                    ui.label(egui::RichText::new("NAVIGATION").strong().size(10.0).color(egui::Color32::from_rgb(140, 140, 150)));
                    ui.add_space(4.0);
                    let is_global = self.selected_view == SelectedView::Global;
                    if ui.add_sized(
                        egui::vec2(ui.available_width(), 26.0),
                        egui::Button::selectable(is_global, "🌐  Global Settings"),
                    ).clicked() {
                        self.selected_view = SelectedView::Global;
                    }

                    ui.add_space(14.0);

                    // Section 2: Devices
                    ui.label(egui::RichText::new("DEVICES").strong().size(10.0).color(egui::Color32::from_rgb(140, 140, 150)));
                    ui.add_space(4.0);

                    struct DeviceItem {
                        serial: String,
                        display_name: String,
                        connected: bool,
                        transport: String,
                    }
                    let mut device_list: Vec<DeviceItem> = Vec::new();

                    for cd in &status.connected_devices {
                        let disp = if let Some(p) = self.cfg.tablets.get(&cd.serial) {
                            p.display_name(&cd.model).to_string()
                        } else {
                            cd.model.clone()
                        };
                        device_list.push(DeviceItem {
                            serial: cd.serial.clone(),
                            display_name: disp,
                            connected: true,
                            transport: cd.transport.clone(),
                        });
                    }
                    for (serial, p) in &self.cfg.tablets {
                        if !device_list.iter().any(|d| &d.serial == serial) {
                            device_list.push(DeviceItem {
                                serial: serial.clone(),
                                display_name: p.display_name(serial).to_string(),
                                connected: false,
                                transport: "Saved".to_string(),
                            });
                        }
                    }

                    if device_list.is_empty() {
                        ui.label(egui::RichText::new("No devices detected").weak().size(11.0));
                    } else {
                        for dev in device_list {
                            let is_selected = match &self.selected_view {
                                SelectedView::Device(s) => s == &dev.serial,
                                _ => false,
                            };
                            let dot = if dev.connected { "●" } else { "○" };
                            let item_text = format!("{} {} ({})", dot, dev.display_name, dev.transport);
                            let mut rich = egui::RichText::new(item_text).size(12.0);
                            if dev.connected {
                                rich = rich.color(egui::Color32::from_rgb(180, 230, 180));
                            }
                            if is_selected {
                                rich = rich.strong();
                            }
                            if ui.add_sized(
                                egui::vec2(ui.available_width(), 26.0),
                                egui::Button::selectable(is_selected, rich),
                            ).clicked() {
                                self.selected_view = SelectedView::Device(dev.serial.clone());
                            }
                        }
                    }

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Utility links
                    ui.horizontal(|ui| {
                        if ui.small_button("Compatibility").clicked() {
                            let body = format!(
                                "Result: \n\nDistribution and desktop: {}\nGPU and encoder: {}\nDevice, Android, stylus: {}\nUScreen version: {}\n\nLatency line from the log (optional):\n\nNotes:\n",
                                os_release_name(), self.cfg.encoder, status.device_model, env!("CARGO_PKG_VERSION"));
                            let url = format!(
                                "https://github.com/majmichu1/UScreen/issues/new?template=compatibility.yml&title={}&body={}",
                                urlencode("Compatibility: "), urlencode(&body));
                            let _ = Command::new("xdg-open").arg(url).spawn();
                        }
                        if ui.small_button("GitHub").clicked() {
                            let _ = Command::new("xdg-open").arg("https://github.com/majmichu1/UScreen").spawn();
                        }
                    });
                    if let Some(v) = self.update.lock().ok().and_then(|g| g.clone()) {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("v{} available", v)).strong().size(11.0));
                            if ui.link("view").clicked() {
                                let _ = Command::new("xdg-open").arg(RELEASES_PAGE).spawn();
                            }
                        });
                    }
                    ui.add_space(10.0);
                });
            });

        // 4. Central Panel: Device or Global Settings
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(6.0);
                match &self.selected_view {
                    SelectedView::Global => {
                        self.render_global_settings(ui, &status);
                    }
                    SelectedView::Device(serial) => {
                        let serial = serial.clone();
                        self.render_device_settings(ui, &serial, &status);
                    }
                }
                ui.add_space(16.0);
            });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 620.0])
            .with_min_inner_size([600.0, 480.0])
            .with_app_id("uscreen")
            .with_icon(egui::IconData {
                rgba: include_bytes!("../../packaging/icons/uscreen-64.rgba").to_vec(),
                width: 64,
                height: 64,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "UScreen",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
