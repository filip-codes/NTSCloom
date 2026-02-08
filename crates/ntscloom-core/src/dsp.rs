use std::f32::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct Yiq {
    pub y: f32,
    pub i: f32,
    pub q: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct CompositeSample {
    pub voltage: f32,
    pub phase_rad: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct LowpassFilter {
    pub state: f32,
    pub alpha: f32,
}

impl LowpassFilter {
    pub fn new(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        let rc = 1.0 / (2.0 * PI * cutoff_hz.max(1.0));
        let dt = 1.0 / sample_rate_hz.max(1.0);
        let alpha = dt / (rc + dt);
        Self { state: 0.0, alpha }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.state += self.alpha * (input - self.state);
        self.state
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PhasePll {
    pub phase: f32,
    pub lock_slew: f32,
}

impl PhasePll {
    pub fn new(phase: f32, lock_slew: f32) -> Self {
        Self { phase, lock_slew }
    }

    pub fn update(&mut self, target_phase: f32, phase_noise: f32, noise: f32) -> f32 {
        let delta = target_phase - self.phase;
        self.phase += delta * self.lock_slew + noise * phase_noise;
        self.phase
    }
}

#[derive(Debug, Clone)]
pub struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    pub fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        let value = (self.state >> 9) | 0x3f800000;
        f32::from_bits(value) - 1.0
    }

    pub fn next_signed(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

pub fn soft_clip(value: f32, strength: f32) -> f32 {
    let k = strength.max(0.0);
    (value * (1.0 + k)) / (1.0 + k * value.abs())
}

pub fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub fn rgb_to_yiq(r: f32, g: f32, b: f32) -> Yiq {
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let i = 0.596 * r - 0.274 * g - 0.322 * b;
    let q = 0.211 * r - 0.523 * g + 0.312 * b;
    Yiq { y, i, q }
}

pub fn yiq_to_rgb(yiq: Yiq) -> (f32, f32, f32) {
    let r = yiq.y + 0.956 * yiq.i + 0.621 * yiq.q;
    let g = yiq.y - 0.272 * yiq.i - 0.647 * yiq.q;
    let b = yiq.y - 1.106 * yiq.i + 1.703 * yiq.q;
    (r, g, b)
}

pub fn encode_composite(yiq: Yiq, subcarrier_hz: f32, time_s: f32, phase_deg: f32) -> CompositeSample {
    let phase_rad = phase_deg.to_radians();
    let carrier_phase = 2.0 * PI * subcarrier_hz * time_s + phase_rad;
    let chroma = yiq.i * carrier_phase.cos() + yiq.q * carrier_phase.sin();
    CompositeSample {
        voltage: yiq.y + chroma,
        phase_rad: carrier_phase,
    }
}

pub fn decode_composite(sample: CompositeSample, subcarrier_hz: f32, time_s: f32, phase_deg: f32) -> Yiq {
    let phase_rad = phase_deg.to_radians();
    let carrier_phase = 2.0 * PI * subcarrier_hz * time_s + phase_rad;
    let i = sample.voltage * carrier_phase.cos();
    let q = sample.voltage * carrier_phase.sin();
    let y = sample.voltage - (i * carrier_phase.cos() + q * carrier_phase.sin());
    Yiq { y, i, q }
}
