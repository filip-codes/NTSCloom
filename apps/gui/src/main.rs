use eframe::egui;
use egui::{ColorImage, TextureHandle};
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use ntscloom_core::{
    process_frame, process_frame_with_progress, DemodulationFilter, Frame, PipelineConfig,
};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PreviewQuality {
    Realtime,
    Full,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Preset {
    name: String,
    config: PipelineConfig,
}

struct AppState {
    config: PipelineConfig,
    presets: Vec<Preset>,
    selected_preset: usize,
    preview_quality: PreviewQuality,
    input_path: Option<PathBuf>,
    input_image: Option<DynamicImage>,
    preview_texture: Option<TextureHandle>,
    status: Option<String>,
    rendering: bool,
    render_progress: Arc<AtomicUsize>,
    render_done: Arc<AtomicBool>,
    render_result: Arc<Mutex<Option<DynamicImage>>>,
}

impl Default for AppState {
    fn default() -> Self {
        let presets = default_presets();
        Self {
            config: presets[0].config.clone(),
            presets,
            selected_preset: 0,
            preview_quality: PreviewQuality::Realtime,
            input_path: None,
            input_image: None,
            preview_texture: None,
            status: Some("Import an image to begin.".to_string()),
            rendering: false,
            render_progress: Arc::new(AtomicUsize::new(0)),
            render_done: Arc::new(AtomicBool::new(false)),
            render_result: Arc::new(Mutex::new(None)),
        }
    }
}

impl AppState {
    fn load_image(&mut self) {
        let file = FileDialog::new()
            .add_filter("Image or Video", &["png", "jpg", "jpeg", "bmp", "tga", "gif", "mp4", "mov", "avi", "webm"])
            .pick_file();
        if let Some(path) = file {
            match image::open(&path) {
                Ok(img) => {
                    self.input_path = Some(path);
                    self.input_image = Some(img);
                    self.status = Some("Image imported successfully.".to_string());
                }
                Err(_) => {
                    self.status = Some("Unable to import video in this prototype. Please import an image.".to_string());
                }
            }
        }
    }

    fn update_preview(&mut self, ctx: &egui::Context) {
        let Some(input) = self.input_image.as_ref() else {
            return;
        };
        let preview_image = match self.preview_quality {
            PreviewQuality::Realtime => {
                let target_w = (input.width() / 2).max(1);
                let target_h = (input.height() / 2).max(1);
                input.resize(target_w, target_h, FilterType::Triangle)
            }
            PreviewQuality::Full => input.clone(),
        };

        let mut config = self.config.clone();
        if self.preview_quality == PreviewQuality::Realtime {
            config.precision.oversample_factor = config.precision.preview_oversample_factor;
            config.precision.resample_taps = config.precision.preview_resample_taps;
        }
        let frame = image_to_frame(&preview_image);
        let processed = process_frame(&frame, &config, 14_318_180.0);
        let color_image = frame_to_color_image(&processed);
        self.preview_texture = Some(ctx.load_texture("preview", color_image, egui::TextureOptions::LINEAR));
    }

    fn start_render(&mut self) {
        let Some(input) = self.input_image.as_ref() else {
            self.status = Some("Import an image before rendering.".to_string());
            return;
        };
        if self.rendering {
            return;
        }
        self.rendering = true;
        self.render_progress.store(0, Ordering::Relaxed);
        self.render_done.store(false, Ordering::Relaxed);
        let config = self.config.clone();
        let input = input.clone();
        let progress = self.render_progress.clone();
        let done = self.render_done.clone();
        let result = self.render_result.clone();
        thread::spawn(move || {
            let frame = image_to_frame(&input);
            let processed = process_frame_with_progress(&frame, &config, 14_318_180.0, |p| {
                let value = (p * 100.0) as usize;
                progress.store(value, Ordering::Relaxed);
            });
            let output = frame_to_image(&processed);
            let mut guard = result.lock().expect("render result lock");
            *guard = Some(output);
            done.store(true, Ordering::Relaxed);
        });
    }

    fn finish_render(&mut self) {
        if !self.render_done.load(Ordering::Relaxed) {
            return;
        }
        self.rendering = false;
        self.render_done.store(false, Ordering::Relaxed);
        let output = {
            let mut guard = self.render_result.lock().expect("render result lock");
            guard.take()
        };
        if let Some(image) = output {
            let save_path = FileDialog::new()
                .set_file_name("ntscloom_output.png")
                .add_filter("PNG", &["png"])
                .add_filter("JPEG", &["jpg", "jpeg"])
                .save_file();
            if let Some(path) = save_path {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let format = match ext.to_lowercase().as_str() {
                        "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                        _ => image::ImageFormat::Png,
                    };
                    if image.save_with_format(path, format).is_ok() {
                        self.status = Some("Render exported successfully.".to_string());
                    } else {
                        self.status = Some("Failed to save rendered image.".to_string());
                    }
                }
            }
        }
    }

    fn save_preset(&mut self) {
        let name = format!("Custom {}", self.presets.len() + 1);
        let preset = Preset {
            name,
            config: self.config.clone(),
        };
        let save_path = FileDialog::new()
            .set_file_name("ntscloom_preset.json")
            .add_filter("Preset", &["json"])
            .save_file();
        if let Some(path) = save_path {
            if serde_json::to_writer_pretty(std::fs::File::create(path).expect("preset file"), &preset).is_ok() {
                self.status = Some("Preset saved.".to_string());
            } else {
                self.status = Some("Failed to save preset.".to_string());
            }
        }
    }

    fn load_preset(&mut self) {
        let open_path = FileDialog::new()
            .add_filter("Preset", &["json"])
            .pick_file();
        if let Some(path) = open_path {
            match std::fs::File::open(path) {
                Ok(file) => {
                    if let Ok(preset) = serde_json::from_reader::<_, Preset>(file) {
                        self.config = preset.config.clone();
                        self.presets.push(preset);
                        self.selected_preset = self.presets.len() - 1;
                        self.status = Some("Preset loaded.".to_string());
                    } else {
                        self.status = Some("Failed to parse preset.".to_string());
                    }
                }
                Err(_) => {
                    self.status = Some("Unable to open preset.".to_string());
                }
            }
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.finish_render();

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Import Video or Image").clicked() {
                    self.load_image();
                    self.update_preview(ctx);
                }
                if ui.button("Update Preview").clicked() {
                    self.update_preview(ctx);
                }
                if ui.button("Render / Export").clicked() {
                    self.start_render();
                }
                if ui.button("Save Preset").clicked() {
                    self.save_preset();
                }
                if ui.button("Load Preset").clicked() {
                    self.load_preset();
                }
            });
        });

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Presets");
            egui::ComboBox::from_id_source("preset_combo")
                .selected_text(self.presets[self.selected_preset].name.clone())
                .show_ui(ui, |ui| {
                    for (idx, preset) in self.presets.iter().enumerate() {
                        if ui.selectable_label(idx == self.selected_preset, &preset.name).clicked() {
                            self.selected_preset = idx;
                            self.config = preset.config.clone();
                        }
                    }
                });

            ui.separator();
            ui.heading("Preview Quality");
            egui::ComboBox::from_id_source("preview_quality")
                .selected_text(match self.preview_quality {
                    PreviewQuality::Realtime => "Low (Realtime)",
                    PreviewQuality::Full => "High (Full)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.preview_quality, PreviewQuality::Realtime, "Low (Realtime)")
                        .on_hover_text("Lower quality, faster preview.");
                    ui.selectable_value(&mut self.preview_quality, PreviewQuality::Full, "High (Full)")
                        .on_hover_text("Full quality preview.");
                });

            ui.separator();
            if let Some(path) = &self.input_path {
                ui.label(format!("Source: {}", path.display()));
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Preview");
            ui.vertical_centered(|ui| {
                if let Some(texture) = &self.preview_texture {
                    let available = ui.available_size();
                    let image_size = texture.size_vec2();
                    let scale = (available.x / image_size.x).min(available.y / image_size.y).min(1.0);
                    ui.image(texture);
                } else {
                    ui.label("Import media to preview.");
                }
            });
            if self.rendering {
                let progress = self.render_progress.load(Ordering::Relaxed) as f32 / 100.0;
                ui.add(egui::ProgressBar::new(progress).text("Rendering..."));
            }
            if let Some(status) = &self.status {
                ui.label(status);
            }
        });

        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            let scroll = egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .id_source("artifact_scroll");
            let mut output = scroll.show(ui, |ui| {
                egui::CollapsingHeader::new("Composite Encoding").default_open(true).show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut self.config.composite.subcarrier_phase_deg, -180.0..=180.0))
                        .on_hover_text("Phase offset of the NTSC subcarrier.");
                    ui.add(egui::Slider::new(&mut self.config.composite.burst_amplitude, 0.0..=2.0))
                        .on_hover_text("Colorburst amplitude per scanline.");
                    ui.add(egui::Slider::new(&mut self.config.composite.chroma_level, 0.0..=2.0))
                        .on_hover_text("Chroma level applied during encoding.");
                });

                egui::CollapsingHeader::new("Channel Filters").default_open(true).show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut self.config.channel.luma_bandwidth_mhz, 0.1..=8.0))
                        .on_hover_text("Luma bandwidth in MHz.");
                    ui.add(egui::Slider::new(&mut self.config.channel.chroma_bandwidth_mhz, 0.1..=6.0))
                        .on_hover_text("Chroma bandwidth in MHz.");
                    ui.add(egui::Slider::new(&mut self.config.channel.luma_ringing, 0.0..=1.0))
                        .on_hover_text("Luma ringing/overshoot amount.");
                    ui.add(egui::Slider::new(&mut self.config.channel.luma_noise, 0.0..=1.0))
                        .on_hover_text("Luma noise level.");
                    ui.add(egui::Slider::new(&mut self.config.channel.dot_crawl_intensity, 0.0..=1.0))
                        .on_hover_text("Dot crawl intensity.");
                });

                egui::CollapsingHeader::new("Tape / VHS").default_open(false).show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut self.config.tape.flutter_rate_hz, 0.1..=20.0))
                        .on_hover_text("Flutter rate in Hz.");
                    ui.add(egui::Slider::new(&mut self.config.tape.flutter_depth, 0.0..=1.0))
                        .on_hover_text("Flutter depth.");
                    ui.add(egui::Slider::new(&mut self.config.tape.tracking_error, 0.0..=1.0))
                        .on_hover_text("Tracking error amount.");
                    ui.add(egui::Slider::new(&mut self.config.tape.dropout_rate, 0.0..=1.0))
                        .on_hover_text("Dropout frequency.");
                    ui.add(egui::Slider::new(&mut self.config.tape.head_switch_jitter, 0.0..=1.0))
                        .on_hover_text("Head switch timing jitter.");
                });

                egui::CollapsingHeader::new("Artifacts").default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut self.config.artifacts.head_switch_enabled, "Head switching")
                        .on_hover_text("Simulate head switching noise band.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.head_switch_height, 0.0..=0.2))
                        .on_hover_text("Head switching band height.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.head_switch_intensity, 0.0..=1.0))
                        .on_hover_text("Head switching intensity.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.head_switch_randomness, 0.0..=1.0))
                        .on_hover_text("Head switching randomness.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.head_switch_phase_distortion, 0.0..=1.0))
                        .on_hover_text("Head switching phase distortion.");
                    ui.separator();
                    ui.checkbox(&mut self.config.artifacts.vertical_jitter_enabled, "Vertical jitter")
                        .on_hover_text("Vertical sync instability.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.vertical_jitter_frequency, 0.0..=5.0))
                        .on_hover_text("Vertical jitter frequency.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.vertical_jitter_amplitude, 0.0..=0.01))
                        .on_hover_text("Vertical jitter amplitude.");
                    ui.checkbox(&mut self.config.artifacts.horizontal_tbc_enabled, "Horizontal TBC")
                        .on_hover_text("Horizontal timebase error.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.horizontal_tbc_frequency, 0.0..=5.0))
                        .on_hover_text("TBC flutter frequency.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.horizontal_tbc_amplitude, 0.0..=0.01))
                        .on_hover_text("TBC amplitude.");
                    ui.checkbox(&mut self.config.artifacts.chroma_phase_drift_enabled, "Chroma phase drift")
                        .on_hover_text("Slow chroma phase drift.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.chroma_phase_drift_rate, 0.0..=1.0))
                        .on_hover_text("Chroma phase drift rate.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.chroma_phase_drift_depth, 0.0..=1.0))
                        .on_hover_text("Chroma phase drift depth.");
                    ui.checkbox(&mut self.config.artifacts.dropout_enabled, "Dropout clusters")
                        .on_hover_text("Tape dropout clusters.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.dropout_rate, 0.0..=0.1))
                        .on_hover_text("Dropout rate.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.dropout_length, 0.0..=0.2))
                        .on_hover_text("Dropout length.");
                    ui.checkbox(&mut self.config.artifacts.crosstalk_dynamic, "Dynamic Y/C crosstalk")
                        .on_hover_text("Dynamic luma/chroma crosstalk.");
                    ui.checkbox(&mut self.config.artifacts.saturation_enabled, "Tape saturation")
                        .on_hover_text("Analog saturation/soft clipping.");
                    ui.add(egui::Slider::new(&mut self.config.artifacts.saturation_strength, 0.0..=1.0))
                        .on_hover_text("Saturation strength.");
                });

                egui::CollapsingHeader::new("Demodulation").default_open(false).show(ui, |ui| {
                    egui::ComboBox::from_id_source("demod_filter")
                        .selected_text(format!("{:?}", self.config.demodulation.filter))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.config.demodulation.filter, DemodulationFilter::Lowpass, "Lowpass");
                            ui.selectable_value(&mut self.config.demodulation.filter, DemodulationFilter::Box, "Box");
                            ui.selectable_value(&mut self.config.demodulation.filter, DemodulationFilter::Notch, "Notch");
                            ui.selectable_value(&mut self.config.demodulation.filter, DemodulationFilter::Comb1D, "1D Comb");
                            ui.selectable_value(&mut self.config.demodulation.filter, DemodulationFilter::Comb2D, "2D Comb");
                        });
                    ui.add(egui::Slider::new(&mut self.config.demodulation.box_kernel, 1..=9))
                        .on_hover_text("Box filter kernel size.");
                    ui.add(egui::Slider::new(&mut self.config.demodulation.notch_bandwidth_mhz, 0.1..=1.5))
                        .on_hover_text("Notch bandwidth in MHz.");
                    ui.add(egui::Slider::new(&mut self.config.demodulation.notch_depth, 0.0..=1.0))
                        .on_hover_text("Notch depth.");
                    ui.add(egui::Slider::new(&mut self.config.demodulation.comb_strength, 0.0..=1.0))
                        .on_hover_text("Comb filter strength.");
                });

                egui::CollapsingHeader::new("Precision & Resampling").default_open(false).show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut self.config.precision.oversample_factor, 1..=4))
                        .on_hover_text("Oversampling factor for final render.");
                    ui.add(egui::Slider::new(&mut self.config.precision.preview_oversample_factor, 1..=2))
                        .on_hover_text("Oversampling factor for realtime preview.");
                    ui.checkbox(&mut self.config.precision.fix_vertical_stripes, "Fix vertical stripes")
                        .on_hover_text("Enable sinc resampling and anti-aliasing to reduce column-aligned artifacts.");
                    ui.add(egui::Slider::new(&mut self.config.precision.resample_taps, 4..=32))
                        .on_hover_text("Resampler tap count (higher = cleaner, slower).");
                    ui.add(egui::Slider::new(&mut self.config.precision.preview_resample_taps, 4..=16))
                        .on_hover_text("Preview resampler tap count.");
                    ui.add(egui::Slider::new(&mut self.config.precision.pll_phase_noise, 0.0..=0.2))
                        .on_hover_text("PLL phase noise.");
                    ui.add(egui::Slider::new(&mut self.config.precision.pll_lock_slew, 0.0..=1.0))
                        .on_hover_text("PLL lock slew.");
                    ui.add(egui::Slider::new(&mut self.config.precision.vhs_chroma_bandwidth_mhz, 0.1..=1.5))
                        .on_hover_text("VHS chroma bandwidth.");
                    ui.add(egui::Slider::new(&mut self.config.precision.chroma_delay_variation, 0.0..=0.01))
                        .on_hover_text("Chroma delay variation.");
                });

                egui::CollapsingHeader::new("Diagnostics").default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut self.config.debug.diagnostic_mode, "Diagnostic mode")
                        .on_hover_text("Enable debug overlays.");
                    ui.checkbox(&mut self.config.debug.show_composite, "Show composite waveform")
                        .on_hover_text("Render composite waveform overlay.");
                    ui.checkbox(&mut self.config.debug.show_iq, "Show demodulated I/Q")
                        .on_hover_text("Render I/Q visualization overlay.");
                    ui.checkbox(&mut self.config.debug.show_grid, "Show diagnostic grid")
                        .on_hover_text("Overlay sample/grid lines for debugging.");
                });

                egui::CollapsingHeader::new("Output").default_open(false).show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut self.config.output.bit_depth, 8..=12))
                        .on_hover_text("Output bit depth.");
                    ui.add(egui::Slider::new(&mut self.config.output.wet_dry_mix, 0.0..=1.0))
                        .on_hover_text("Wet/dry mix.");
                });
            });

            let mut scroll_state = output.state;
            if ui.input(|i| i.key_pressed(egui::Key::PageDown)) {
                scroll_state.offset.y += 200.0;
            }
            if ui.input(|i| i.key_pressed(egui::Key::PageUp)) {
                scroll_state.offset.y = (scroll_state.offset.y - 200.0).max(0.0);
            }
            scroll_state.store(ui.ctx(), output.id);
        });
    }
}

fn default_presets() -> Vec<Preset> {
    let mut clean = PipelineConfig::default();
    clean.channel.luma_noise = 0.0;
    clean.tape.flutter_depth = 0.02;
    clean.artifacts.head_switch_intensity = 0.1;
    clean.artifacts.dropout_rate = 0.0;
    clean.demodulation.filter = DemodulationFilter::Comb2D;

    let mut consumer = PipelineConfig::default();
    consumer.tape.flutter_depth = 0.2;
    consumer.artifacts.chroma_phase_drift_depth = 0.3;
    consumer.demodulation.filter = DemodulationFilter::Comb1D;

    let mut damaged = PipelineConfig::default();
    damaged.tape.dropout_rate = 0.08;
    damaged.artifacts.dropout_rate = 0.08;
    damaged.artifacts.head_switch_intensity = 0.6;
    damaged.artifacts.saturation_strength = 0.5;
    damaged.demodulation.filter = DemodulationFilter::Lowpass;

    let mut severe = PipelineConfig::default();
    severe.tape.tracking_error = 0.4;
    severe.artifacts.horizontal_tbc_amplitude = 0.008;
    severe.artifacts.vertical_jitter_amplitude = 0.006;
    severe.demodulation.filter = DemodulationFilter::Box;

    let mut camcorder = PipelineConfig::default();
    camcorder.channel.chroma_bandwidth_mhz = 0.8;
    camcorder.precision.vhs_chroma_bandwidth_mhz = 0.6;
    camcorder.artifacts.chroma_phase_drift_depth = 0.4;
    camcorder.demodulation.filter = DemodulationFilter::Notch;

    vec![
        Preset {
            name: "Clean Broadcast NTSC".to_string(),
            config: clean,
        },
        Preset {
            name: "Consumer VHS".to_string(),
            config: consumer,
        },
        Preset {
            name: "Damaged Tape".to_string(),
            config: damaged,
        },
        Preset {
            name: "Severe Tracking Error".to_string(),
            config: severe,
        },
        Preset {
            name: "Vintage Camcorder".to_string(),
            config: camcorder,
        },
    ]
}

fn image_to_frame(image: &DynamicImage) -> Frame {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    let mut frame = Frame::new(width as usize, height as usize);
    for (idx, pixel) in rgb.pixels().enumerate() {
        let [r, g, b] = pixel.0;
        let base = idx * 3;
        frame.data[base] = r as f32 / 255.0;
        frame.data[base + 1] = g as f32 / 255.0;
        frame.data[base + 2] = b as f32 / 255.0;
    }
    frame
}

fn frame_to_image(frame: &Frame) -> DynamicImage {
    let mut buffer = image::RgbImage::new(frame.width as u32, frame.height as u32);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let idx = (y * frame.width + x) * 3;
            let r = (frame.data[idx].clamp(0.0, 1.0) * 255.0) as u8;
            let g = (frame.data[idx + 1].clamp(0.0, 1.0) * 255.0) as u8;
            let b = (frame.data[idx + 2].clamp(0.0, 1.0) * 255.0) as u8;
            buffer.put_pixel(x as u32, y as u32, image::Rgb([r, g, b]));
        }
    }
    DynamicImage::ImageRgb8(buffer)
}

fn frame_to_color_image(frame: &Frame) -> ColorImage {
    let mut pixels = Vec::with_capacity(frame.width * frame.height);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let idx = (y * frame.width + x) * 3;
            let r = (frame.data[idx].clamp(0.0, 1.0) * 255.0) as u8;
            let g = (frame.data[idx + 1].clamp(0.0, 1.0) * 255.0) as u8;
            let b = (frame.data[idx + 2].clamp(0.0, 1.0) * 255.0) as u8;
            pixels.push(egui::Color32::from_rgb(r, g, b));
        }
    }
    ColorImage {
        size: [frame.width, frame.height],
        pixels,
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "NTSCloom",
        options,
        Box::new(|_cc| Box::<AppState>::default()),
    )
}
