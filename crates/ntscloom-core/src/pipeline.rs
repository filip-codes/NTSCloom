use crate::config::{DemodulationFilter, PipelineConfig};
use crate::dsp::{
    linear_to_srgb, rgb_to_yiq, soft_clip, srgb_to_linear, yiq_to_rgb, CompositeSample, LowpassFilter,
    PhasePll, SimpleRng, Yiq,
};

struct BoxFilter {
    buffer: Vec<f32>,
    index: usize,
    sum: f32,
}

struct SincResampler {
    taps: usize,
}

impl SincResampler {
    fn new(taps: usize) -> Self {
        Self { taps: taps.max(4) }
    }

    fn sample(&self, data: &[f32], position: f32) -> f32 {
        if data.is_empty() {
            return 0.0;
        }
        let taps = self.taps as i32;
        let half = taps / 2;
        let center = position.floor() as i32;
        let mut sum = 0.0;
        let mut weight_sum = 0.0;
        for i in -half..=half {
            let idx = (center + i).clamp(0, data.len() as i32 - 1) as usize;
            let x = position - (center + i) as f32;
            let sinc = if x.abs() < 1e-3 { 1.0 } else { (std::f32::consts::PI * x).sin() / (std::f32::consts::PI * x) };
            let window = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * (i + half) as f32 / (taps as f32)).cos();
            let weight = sinc * window;
            sum += data[idx] * weight;
            weight_sum += weight;
        }
        if weight_sum.abs() > 1e-6 {
            sum / weight_sum
        } else {
            data[center.clamp(0, data.len() as i32 - 1) as usize]
        }
    }
}

impl BoxFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            index: 0,
            sum: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let size = self.buffer.len();
        self.sum -= self.buffer[self.index];
        self.buffer[self.index] = input;
        self.sum += input;
        self.index = (self.index + 1) % size;
        self.sum / size as f32
    }
}

pub enum FrameFormat {
    RgbF32,
}

pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub format: FrameFormat,
    pub data: Vec<f32>,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            format: FrameFormat::RgbF32,
            data: vec![0.0; width * height * 3],
        }
    }
}

pub fn process_frame(frame: &Frame, config: &PipelineConfig, sample_rate_hz: f32) -> Frame {
    let subcarrier_hz = 3_579_545.0;
    let mut out = Frame::new(frame.width, frame.height);
    let phase_deg = config.composite.subcarrier_phase_deg;
    let phase_offset = phase_deg.to_radians();
    let oversample = config.precision.oversample_factor.max(1) as usize;
    let effective_sample_rate = sample_rate_hz.max(1.0) * oversample as f32;
    let phase_step = 2.0 * std::f32::consts::PI * subcarrier_hz / effective_sample_rate;
    let luma_cutoff_hz = config.channel.luma_bandwidth_mhz.max(0.1) * 1_000_000.0;
    let chroma_cutoff_hz = config.channel.chroma_bandwidth_mhz.max(0.1) * 1_000_000.0;
    let vhs_chroma_cutoff_hz = config.precision.vhs_chroma_bandwidth_mhz.max(0.1) * 1_000_000.0;
    let i_cutoff_hz = chroma_cutoff_hz.min(1_300_000.0).min(vhs_chroma_cutoff_hz);
    let q_cutoff_hz = chroma_cutoff_hz.min(500_000.0).min(vhs_chroma_cutoff_hz);
    let mut encoder_i_filter = LowpassFilter::new(i_cutoff_hz, effective_sample_rate);
    let mut encoder_q_filter = LowpassFilter::new(q_cutoff_hz, effective_sample_rate);
    let mut y_filter = LowpassFilter::new(luma_cutoff_hz, effective_sample_rate);
    let mut i_filter = LowpassFilter::new(i_cutoff_hz, effective_sample_rate);
    let mut q_filter = LowpassFilter::new(q_cutoff_hz, effective_sample_rate);
    let mut luma_highpass = 0.0_f32;
    let mut chroma_delay = 0.0_f32;
    let mut pll = PhasePll::new(phase_offset, config.precision.pll_lock_slew);
    let mut rng = SimpleRng::new(0x1a2b3c4d);
    let mut i_box = BoxFilter::new(config.demodulation.box_kernel);
    let mut q_box = BoxFilter::new(config.demodulation.box_kernel);
    let mut previous_line = vec![0.0_f32; frame.width];
    let mut previous_line_2 = vec![0.0_f32; frame.width];
    let mut dropout_remaining = 0usize;
    let resample_taps = if config.precision.fix_vertical_stripes {
        config.precision.resample_taps
    } else {
        4
    };
    let resampler = SincResampler::new(resample_taps as usize);
    let mut yiq_line = vec![Yiq { y: 0.0, i: 0.0, q: 0.0 }; frame.width];
    let mut i_line = vec![0.0_f32; frame.width];
    let mut q_line = vec![0.0_f32; frame.width];

    for y in 0..frame.height {
        for x in 0..frame.width {
            let idx = (y * frame.width + x) * 3;
            let r = srgb_to_linear(frame.data[idx].clamp(0.0, 1.0));
            let g = srgb_to_linear(frame.data[idx + 1].clamp(0.0, 1.0));
            let b = srgb_to_linear(frame.data[idx + 2].clamp(0.0, 1.0));
            let mut yiq = rgb_to_yiq(r, g, b);
            yiq.i = encoder_i_filter.process(yiq.i);
            yiq.q = encoder_q_filter.process(yiq.q);
            yiq_line[x] = yiq;
        }

        let samples_per_line = frame.width * oversample;
        let mut composite_line = vec![0.0_f32; samples_per_line];
        let mut cos_line = vec![0.0_f32; samples_per_line];
        let mut sin_line = vec![0.0_f32; samples_per_line];

        for s in 0..samples_per_line {
            let pixel = s / oversample;
            let yiq = yiq_line[pixel];
            let sample_index = (y * samples_per_line + s) as f32;
            let base_phase = phase_offset + phase_step * sample_index;
            let jitter_phase = apply_timebase_jitter(
                y,
                frame.height,
                base_phase,
                &mut rng,
                &config.artifacts,
            );
            let drift_phase = apply_chroma_phase_drift(
                sample_index,
                jitter_phase,
                &config.artifacts,
            );
            let pll_phase = pll.update(
                drift_phase,
                config.precision.pll_phase_noise,
                rng.next_signed(),
            );
            let mut composite = encode_composite_with_phase(yiq, pll_phase);
            let degraded = apply_channel(composite, &config.channel);
            composite = apply_tape(degraded, &config.tape);
            apply_head_switching(
                &mut composite,
                y,
                frame.height,
                &mut rng,
                &config.artifacts,
            );
            if config.artifacts.dropout_enabled {
                apply_dropout(&mut composite, &mut rng, &mut dropout_remaining, &config.artifacts);
            }
            composite.voltage = apply_saturation(composite.voltage, &config.artifacts);
            composite_line[s] = composite.voltage;
            cos_line[s] = pll_phase.cos();
            sin_line[s] = pll_phase.sin();
        }

        for x in 0..frame.width {
            let idx = (y * frame.width + x) * 3;
            let mut decoded = Yiq { y: 0.0, i: 0.0, q: 0.0 };
            let sample_pos = (x as f32 + 0.5) * oversample as f32;
            let composite = resampler.sample(&composite_line, sample_pos);
            let cos_phase = resampler.sample(&cos_line, sample_pos);
            let sin_phase = resampler.sample(&sin_line, sample_pos);
            let sample = CompositeSample {
                voltage: composite,
                phase_rad: 0.0,
            };
            decoded = decode_composite_stateful(
                sample,
                cos_phase,
                sin_phase,
                &mut y_filter,
                &mut i_filter,
                &mut q_filter,
                &mut i_box,
                &mut q_box,
                &mut previous_line,
                &mut previous_line_2,
                x,
                &mut luma_highpass,
                &mut chroma_delay,
                &config.demodulation,
                &config.artifacts,
                config.precision.chroma_delay_variation,
            );
            i_line[x] = decoded.i;
            q_line[x] = decoded.q;

            let (out_r, out_g, out_b) = yiq_to_rgb(decoded);
            out.data[idx] = linear_to_srgb(out_r).clamp(0.0, 1.0);
            out.data[idx + 1] = linear_to_srgb(out_g).clamp(0.0, 1.0);
            out.data[idx + 2] = linear_to_srgb(out_b).clamp(0.0, 1.0);
        }

        apply_chroma_blur(&mut i_line, &mut q_line, config.channel.chroma_bandwidth_mhz);
        if config.debug.diagnostic_mode {
            apply_diagnostics(&mut out, y, &composite_line, &i_line, &q_line, &resampler, config);
        }
    }

    out
}

pub fn process_frame_with_progress<F>(
    frame: &Frame,
    config: &PipelineConfig,
    sample_rate_hz: f32,
    mut on_progress: F,
) -> Frame
where
    F: FnMut(f32),
{
    let subcarrier_hz = 3_579_545.0;
    let mut out = Frame::new(frame.width, frame.height);
    let phase_deg = config.composite.subcarrier_phase_deg;
    let phase_offset = phase_deg.to_radians();
    let oversample = config.precision.oversample_factor.max(1) as usize;
    let effective_sample_rate = sample_rate_hz.max(1.0) * oversample as f32;
    let phase_step = 2.0 * std::f32::consts::PI * subcarrier_hz / effective_sample_rate;
    let luma_cutoff_hz = config.channel.luma_bandwidth_mhz.max(0.1) * 1_000_000.0;
    let chroma_cutoff_hz = config.channel.chroma_bandwidth_mhz.max(0.1) * 1_000_000.0;
    let vhs_chroma_cutoff_hz = config.precision.vhs_chroma_bandwidth_mhz.max(0.1) * 1_000_000.0;
    let i_cutoff_hz = chroma_cutoff_hz.min(1_300_000.0).min(vhs_chroma_cutoff_hz);
    let q_cutoff_hz = chroma_cutoff_hz.min(500_000.0).min(vhs_chroma_cutoff_hz);
    let mut encoder_i_filter = LowpassFilter::new(i_cutoff_hz, effective_sample_rate);
    let mut encoder_q_filter = LowpassFilter::new(q_cutoff_hz, effective_sample_rate);
    let mut y_filter = LowpassFilter::new(luma_cutoff_hz, effective_sample_rate);
    let mut i_filter = LowpassFilter::new(i_cutoff_hz, effective_sample_rate);
    let mut q_filter = LowpassFilter::new(q_cutoff_hz, effective_sample_rate);
    let mut luma_highpass = 0.0_f32;
    let mut chroma_delay = 0.0_f32;
    let mut pll = PhasePll::new(phase_offset, config.precision.pll_lock_slew);
    let mut rng = SimpleRng::new(0x1234abcd);
    let mut i_box = BoxFilter::new(config.demodulation.box_kernel);
    let mut q_box = BoxFilter::new(config.demodulation.box_kernel);
    let mut previous_line = vec![0.0_f32; frame.width];
    let mut previous_line_2 = vec![0.0_f32; frame.width];
    let mut dropout_remaining = 0usize;
    let resample_taps = if config.precision.fix_vertical_stripes {
        config.precision.resample_taps
    } else {
        4
    };
    let resampler = SincResampler::new(resample_taps as usize);
    let mut yiq_line = vec![Yiq { y: 0.0, i: 0.0, q: 0.0 }; frame.width];
    let mut i_line = vec![0.0_f32; frame.width];
    let mut q_line = vec![0.0_f32; frame.width];

    for y in 0..frame.height {
        for x in 0..frame.width {
            let idx = (y * frame.width + x) * 3;
            let r = srgb_to_linear(frame.data[idx].clamp(0.0, 1.0));
            let g = srgb_to_linear(frame.data[idx + 1].clamp(0.0, 1.0));
            let b = srgb_to_linear(frame.data[idx + 2].clamp(0.0, 1.0));
            let mut yiq = rgb_to_yiq(r, g, b);
            yiq.i = encoder_i_filter.process(yiq.i);
            yiq.q = encoder_q_filter.process(yiq.q);
            yiq_line[x] = yiq;
        }

        let samples_per_line = frame.width * oversample;
        let mut composite_line = vec![0.0_f32; samples_per_line];
        let mut cos_line = vec![0.0_f32; samples_per_line];
        let mut sin_line = vec![0.0_f32; samples_per_line];

        for s in 0..samples_per_line {
            let pixel = s / oversample;
            let yiq = yiq_line[pixel];
            let sample_index = (y * samples_per_line + s) as f32;
            let base_phase = phase_offset + phase_step * sample_index;
            let jitter_phase = apply_timebase_jitter(
                y,
                frame.height,
                base_phase,
                &mut rng,
                &config.artifacts,
            );
            let drift_phase = apply_chroma_phase_drift(
                sample_index,
                jitter_phase,
                &config.artifacts,
            );
            let pll_phase = pll.update(
                drift_phase,
                config.precision.pll_phase_noise,
                rng.next_signed(),
            );
            let mut composite = encode_composite_with_phase(yiq, pll_phase);
            let degraded = apply_channel(composite, &config.channel);
            composite = apply_tape(degraded, &config.tape);
            apply_head_switching(
                &mut composite,
                y,
                frame.height,
                &mut rng,
                &config.artifacts,
            );
            if config.artifacts.dropout_enabled {
                apply_dropout(&mut composite, &mut rng, &mut dropout_remaining, &config.artifacts);
            }
            composite.voltage = apply_saturation(composite.voltage, &config.artifacts);
            composite_line[s] = composite.voltage;
            cos_line[s] = pll_phase.cos();
            sin_line[s] = pll_phase.sin();
        }

        for x in 0..frame.width {
            let idx = (y * frame.width + x) * 3;
            let mut decoded = Yiq { y: 0.0, i: 0.0, q: 0.0 };
            let sample_pos = (x as f32 + 0.5) * oversample as f32;
            let composite = resampler.sample(&composite_line, sample_pos);
            let cos_phase = resampler.sample(&cos_line, sample_pos);
            let sin_phase = resampler.sample(&sin_line, sample_pos);
            let sample = CompositeSample {
                voltage: composite,
                phase_rad: 0.0,
            };
            decoded = decode_composite_stateful(
                sample,
                cos_phase,
                sin_phase,
                &mut y_filter,
                &mut i_filter,
                &mut q_filter,
                &mut i_box,
                &mut q_box,
                &mut previous_line,
                &mut previous_line_2,
                x,
                &mut luma_highpass,
                &mut chroma_delay,
                &config.demodulation,
                &config.artifacts,
                config.precision.chroma_delay_variation,
            );
            i_line[x] = decoded.i;
            q_line[x] = decoded.q;

            let (out_r, out_g, out_b) = yiq_to_rgb(decoded);
            out.data[idx] = linear_to_srgb(out_r).clamp(0.0, 1.0);
            out.data[idx + 1] = linear_to_srgb(out_g).clamp(0.0, 1.0);
            out.data[idx + 2] = linear_to_srgb(out_b).clamp(0.0, 1.0);
        }
        apply_chroma_blur(&mut i_line, &mut q_line, config.channel.chroma_bandwidth_mhz);
        if config.debug.diagnostic_mode {
            apply_diagnostics(&mut out, y, &composite_line, &i_line, &q_line, &resampler, config);
        }
        on_progress((y + 1) as f32 / frame.height as f32);
    }

    out
}

fn encode_composite_with_phase(yiq: Yiq, phase_rad: f32) -> CompositeSample {
    let chroma = yiq.i * phase_rad.cos() + yiq.q * phase_rad.sin();
    CompositeSample {
        voltage: yiq.y + chroma,
        phase_rad,
    }
}

fn decode_composite_stateful(
    sample: CompositeSample,
    cos_phase: f32,
    sin_phase: f32,
    y_filter: &mut LowpassFilter,
    i_filter: &mut LowpassFilter,
    q_filter: &mut LowpassFilter,
    i_box: &mut BoxFilter,
    q_box: &mut BoxFilter,
    previous_line: &mut [f32],
    previous_line_2: &mut [f32],
    x: usize,
    luma_highpass: &mut f32,
    chroma_delay: &mut f32,
    demodulation: &crate::config::DemodulationConfig,
    artifacts: &crate::config::ArtifactConfig,
    chroma_delay_variation: f32,
) -> Yiq {
    let raw_i = sample.voltage * cos_phase;
    let raw_q = sample.voltage * sin_phase;

    let (mut chroma_i, mut chroma_q, mut y) = match demodulation.filter {
        DemodulationFilter::Box => {
            let i = i_box.process(raw_i);
            let q = q_box.process(raw_q);
            let y = y_filter.process(sample.voltage);
            (i, q, y)
        }
        DemodulationFilter::Notch => {
            let i = i_filter.process(raw_i);
            let q = q_filter.process(raw_q);
            let chroma_signal = i * cos_phase + q * sin_phase;
            let notch_scale = (demodulation.notch_bandwidth_mhz / 1.5).clamp(0.1, 1.0);
            let y = y_filter.process(sample.voltage) - demodulation.notch_depth * notch_scale * chroma_signal;
            (i, q, y)
        }
        DemodulationFilter::Comb1D => {
            let prev = previous_line[x];
            let comb_y = 0.5 * (sample.voltage + prev);
            let comb_c = 0.5 * (sample.voltage - prev) * demodulation.comb_strength;
            previous_line[x] = sample.voltage;
            let i = i_filter.process(comb_c * cos_phase);
            let q = q_filter.process(comb_c * sin_phase);
            (i, q, y_filter.process(comb_y))
        }
        DemodulationFilter::Comb2D => {
            let prev = previous_line[x];
            let prev2 = previous_line_2[x];
            let comb_y = (sample.voltage + prev + prev2) / 3.0;
            let comb_c = (sample.voltage - prev2) * 0.5 * demodulation.comb_strength;
            previous_line_2[x] = prev;
            previous_line[x] = sample.voltage;
            let i = i_filter.process(comb_c * cos_phase);
            let q = q_filter.process(comb_c * sin_phase);
            (i, q, y_filter.process(comb_y))
        }
        DemodulationFilter::Lowpass => {
            let i = i_filter.process(raw_i);
            let q = q_filter.process(raw_q);
            let y = y_filter.process(sample.voltage);
            (i, q, y)
        }
    };

    if artifacts.crosstalk_dynamic {
        let chroma_signal = chroma_i * cos_phase + chroma_q * sin_phase;
        y += 0.03 * chroma_signal;
        let high = sample.voltage - *luma_highpass;
        *luma_highpass = sample.voltage;
        chroma_i += 0.02 * high;
        chroma_q += 0.02 * high;
    }

    if artifacts.chroma_phase_drift_enabled {
        *chroma_delay += chroma_delay_variation;
        let drift = *chroma_delay;
        let drift_cos = drift.cos();
        let drift_sin = drift.sin();
        let i = chroma_i * drift_cos - chroma_q * drift_sin;
        let q = chroma_i * drift_sin + chroma_q * drift_cos;
        chroma_i = i;
        chroma_q = q;
    }

    let i = chroma_i;
    let q = chroma_q;
    Yiq { y, i, q }
}

fn apply_chroma_blur(i_line: &mut [f32], q_line: &mut [f32], chroma_bandwidth_mhz: f32) {
    let strength = (1.5 - chroma_bandwidth_mhz).clamp(0.0, 1.0) / 1.5;
    if strength <= 0.0 {
        return;
    }
    let mut i_blur = i_line.to_vec();
    let mut q_blur = q_line.to_vec();
    for x in 0..i_line.len() {
        let prev = if x == 0 { x } else { x - 1 };
        let next = if x + 1 >= i_line.len() { x } else { x + 1 };
        i_blur[x] = 0.25 * i_line[prev] + 0.5 * i_line[x] + 0.25 * i_line[next];
        q_blur[x] = 0.25 * q_line[prev] + 0.5 * q_line[x] + 0.25 * q_line[next];
    }
    for x in 0..i_line.len() {
        i_line[x] = i_line[x] * (1.0 - strength) + i_blur[x] * strength;
        q_line[x] = q_line[x] * (1.0 - strength) + q_blur[x] * strength;
    }
}

fn apply_diagnostics(
    frame: &mut Frame,
    y: usize,
    composite_line: &[f32],
    i_line: &[f32],
    q_line: &[f32],
    resampler: &SincResampler,
    config: &PipelineConfig,
) {
    let width = frame.width;
    for x in 0..width {
        let idx = (y * width + x) * 3;
        if config.debug.show_composite {
            let sample_pos = (x as f32 + 0.5) * config.precision.oversample_factor.max(1) as f32;
            let composite = resampler.sample(composite_line, sample_pos);
            let value = (composite * 0.5 + 0.5).clamp(0.0, 1.0);
            frame.data[idx] = value;
            frame.data[idx + 1] = value;
            frame.data[idx + 2] = value;
        }
        if config.debug.show_iq {
            frame.data[idx] = (i_line[x] * 0.5 + 0.5).clamp(0.0, 1.0);
            frame.data[idx + 1] = (q_line[x] * 0.5 + 0.5).clamp(0.0, 1.0);
            frame.data[idx + 2] = 0.5;
        }
        if config.debug.show_grid && (x % 16 == 0 || y % 16 == 0) {
            frame.data[idx] = 1.0;
            frame.data[idx + 1] = 0.1;
            frame.data[idx + 2] = 0.1;
        }
    }
}

fn apply_head_switching(
    sample: &mut CompositeSample,
    y: usize,
    height: usize,
    rng: &mut SimpleRng,
    artifacts: &crate::config::ArtifactConfig,
) {
    if !artifacts.head_switch_enabled || height == 0 {
        return;
    }
    let band_start = ((1.0 - artifacts.head_switch_height.clamp(0.0, 1.0)) * height as f32) as usize;
    if y >= band_start {
        let noise = rng.next_signed() * artifacts.head_switch_randomness;
        sample.voltage += artifacts.head_switch_intensity * (0.1 + noise);
        sample.phase_rad += artifacts.head_switch_phase_distortion * noise;
    }
}

fn apply_timebase_jitter(
    y: usize,
    height: usize,
    phase_rad: f32,
    rng: &mut SimpleRng,
    artifacts: &crate::config::ArtifactConfig,
) -> f32 {
    if !artifacts.horizontal_tbc_enabled && !artifacts.vertical_jitter_enabled {
        return phase_rad;
    }
    let line_norm = if height > 0 {
        y as f32 / height as f32
    } else {
        0.0
    };
    let jitter = if artifacts.vertical_jitter_enabled {
        (line_norm * std::f32::consts::TAU * artifacts.vertical_jitter_frequency).sin()
            * artifacts.vertical_jitter_amplitude
    } else {
        0.0
    };
    let tbc = if artifacts.horizontal_tbc_enabled {
        let noise = rng.next_signed() * 0.5;
        (line_norm * std::f32::consts::TAU * artifacts.horizontal_tbc_frequency + noise).sin()
            * artifacts.horizontal_tbc_amplitude
    } else {
        0.0
    };
    phase_rad + jitter + tbc
}

fn apply_chroma_phase_drift(
    sample_index: f32,
    phase_rad: f32,
    artifacts: &crate::config::ArtifactConfig,
) -> f32 {
    if !artifacts.chroma_phase_drift_enabled {
        return phase_rad;
    }
    let drift = (sample_index * artifacts.chroma_phase_drift_rate * 0.0001).sin()
        * artifacts.chroma_phase_drift_depth;
    phase_rad + drift
}

fn apply_dropout(
    sample: &mut CompositeSample,
    rng: &mut SimpleRng,
    remaining: &mut usize,
    artifacts: &crate::config::ArtifactConfig,
) {
    if *remaining == 0 && rng.next_f32() < artifacts.dropout_rate {
        let length = (artifacts.dropout_length.max(0.0) * 100.0) as usize + 1;
        *remaining = length;
    }
    if *remaining > 0 {
        sample.voltage += rng.next_signed() * 0.4;
        *remaining -= 1;
    }
}

fn apply_saturation(voltage: f32, artifacts: &crate::config::ArtifactConfig) -> f32 {
    if artifacts.saturation_enabled {
        soft_clip(voltage, artifacts.saturation_strength)
    } else {
        voltage
    }
}

fn apply_channel(sample: CompositeSample, config: &crate::config::ChannelConfig) -> CompositeSample {
    let ringing = config.luma_ringing * (sample.phase_rad * 0.5).sin();
    let noise = config.luma_noise * (sample.phase_rad * 13.37).cos();
    CompositeSample {
        voltage: sample.voltage + ringing + noise,
        phase_rad: sample.phase_rad,
    }
}

fn apply_tape(sample: CompositeSample, config: &crate::config::TapeConfig) -> CompositeSample {
    let flutter = config.flutter_depth * (sample.phase_rad * config.flutter_rate_hz).sin();
    let dropout = if (sample.phase_rad * 0.1).sin() > 0.995 { -0.2 } else { 0.0 };
    CompositeSample {
        voltage: sample.voltage * (1.0 - config.tracking_error) + flutter + dropout,
        phase_rad: sample.phase_rad + config.head_switch_jitter * 0.01,
    }
}
