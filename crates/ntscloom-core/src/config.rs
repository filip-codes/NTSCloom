use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeConfig {
    pub subcarrier_phase_deg: f32,
    pub burst_amplitude: f32,
    pub chroma_level: f32,
}

impl Default for CompositeConfig {
    fn default() -> Self {
        Self {
            subcarrier_phase_deg: 0.0,
            burst_amplitude: 1.0,
            chroma_level: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub luma_bandwidth_mhz: f32,
    pub chroma_bandwidth_mhz: f32,
    pub luma_ringing: f32,
    pub luma_noise: f32,
    pub dot_crawl_intensity: f32,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            luma_bandwidth_mhz: 4.2,
            chroma_bandwidth_mhz: 1.5,
            luma_ringing: 0.2,
            luma_noise: 0.02,
            dot_crawl_intensity: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapeConfig {
    pub flutter_rate_hz: f32,
    pub flutter_depth: f32,
    pub tracking_error: f32,
    pub dropout_rate: f32,
    pub head_switch_jitter: f32,
}

impl Default for TapeConfig {
    fn default() -> Self {
        Self {
            flutter_rate_hz: 0.8,
            flutter_depth: 0.15,
            tracking_error: 0.1,
            dropout_rate: 0.02,
            head_switch_jitter: 0.05,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub bit_depth: u8,
    pub wet_dry_mix: f32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            bit_depth: 10,
            wet_dry_mix: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub composite: CompositeConfig,
    pub channel: ChannelConfig,
    pub tape: TapeConfig,
    pub artifacts: ArtifactConfig,
    pub demodulation: DemodulationConfig,
    pub precision: PrecisionConfig,
    pub debug: DebugConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DemodulationFilter {
    Lowpass,
    Box,
    Notch,
    Comb1D,
    Comb2D,
}

impl Default for DemodulationFilter {
    fn default() -> Self {
        Self::Lowpass
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemodulationConfig {
    pub filter: DemodulationFilter,
    pub box_kernel: usize,
    pub notch_bandwidth_mhz: f32,
    pub notch_depth: f32,
    pub comb_strength: f32,
}

impl Default for DemodulationConfig {
    fn default() -> Self {
        Self {
            filter: DemodulationFilter::Lowpass,
            box_kernel: 3,
            notch_bandwidth_mhz: 0.6,
            notch_depth: 0.5,
            comb_strength: 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactConfig {
    pub head_switch_enabled: bool,
    pub head_switch_height: f32,
    pub head_switch_intensity: f32,
    pub head_switch_randomness: f32,
    pub head_switch_phase_distortion: f32,
    pub vertical_jitter_enabled: bool,
    pub vertical_jitter_frequency: f32,
    pub vertical_jitter_amplitude: f32,
    pub horizontal_tbc_enabled: bool,
    pub horizontal_tbc_frequency: f32,
    pub horizontal_tbc_amplitude: f32,
    pub chroma_phase_drift_enabled: bool,
    pub chroma_phase_drift_rate: f32,
    pub chroma_phase_drift_depth: f32,
    pub dropout_enabled: bool,
    pub dropout_rate: f32,
    pub dropout_length: f32,
    pub crosstalk_dynamic: bool,
    pub saturation_enabled: bool,
    pub saturation_strength: f32,
}

impl Default for ArtifactConfig {
    fn default() -> Self {
        Self {
            head_switch_enabled: true,
            head_switch_height: 0.06,
            head_switch_intensity: 0.4,
            head_switch_randomness: 0.4,
            head_switch_phase_distortion: 0.3,
            vertical_jitter_enabled: true,
            vertical_jitter_frequency: 0.5,
            vertical_jitter_amplitude: 0.003,
            horizontal_tbc_enabled: true,
            horizontal_tbc_frequency: 1.2,
            horizontal_tbc_amplitude: 0.002,
            chroma_phase_drift_enabled: true,
            chroma_phase_drift_rate: 0.15,
            chroma_phase_drift_depth: 0.2,
            dropout_enabled: true,
            dropout_rate: 0.02,
            dropout_length: 0.03,
            crosstalk_dynamic: true,
            saturation_enabled: true,
            saturation_strength: 0.35,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecisionConfig {
    pub oversample_factor: u8,
    pub preview_oversample_factor: u8,
    pub resample_taps: u8,
    pub preview_resample_taps: u8,
    pub fix_vertical_stripes: bool,
    pub pll_phase_noise: f32,
    pub pll_lock_slew: f32,
    pub vhs_chroma_bandwidth_mhz: f32,
    pub chroma_delay_variation: f32,
}

impl Default for PrecisionConfig {
    fn default() -> Self {
        Self {
            oversample_factor: 2,
            preview_oversample_factor: 1,
            resample_taps: 16,
            preview_resample_taps: 8,
            fix_vertical_stripes: true,
            pll_phase_noise: 0.02,
            pll_lock_slew: 0.15,
            vhs_chroma_bandwidth_mhz: 0.8,
            chroma_delay_variation: 0.001,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    pub diagnostic_mode: bool,
    pub show_composite: bool,
    pub show_iq: bool,
    pub show_grid: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            diagnostic_mode: false,
            show_composite: false,
            show_iq: false,
            show_grid: false,
        }
    }
}
