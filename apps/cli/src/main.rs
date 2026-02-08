use clap::Parser;
use ntscloom_core::{process_frame, DemodulationFilter, Frame, PipelineConfig};

#[derive(Parser, Debug)]
#[command(author, version, about = "NTSCloom CLI batch renderer prototype")]
struct Args {
    #[arg(long, default_value_t = 1920)]
    width: usize,
    #[arg(long, default_value_t = 1080)]
    height: usize,
    #[arg(long, default_value = "consumer-vhs")]
    preset: String,
    #[arg(long, default_value = "lowpass")]
    demod: String,
    #[arg(long, default_value_t = 2)]
    oversample: u8,
}

fn main() {
    let args = Args::parse();
    let frame = Frame::new(args.width, args.height);
    let mut config = preset_config(&args.preset);
    config.demodulation.filter = parse_demod(&args.demod);
    config.precision.oversample_factor = args.oversample;
    let _out = process_frame(&frame, &config, 14_318_180.0);
    println!("Rendered {}x{} frame through NTSCloom pipeline.", args.width, args.height);
}

fn parse_demod(value: &str) -> DemodulationFilter {
    match value.to_lowercase().as_str() {
        "box" => DemodulationFilter::Box,
        "notch" => DemodulationFilter::Notch,
        "comb1d" => DemodulationFilter::Comb1D,
        "comb2d" => DemodulationFilter::Comb2D,
        _ => DemodulationFilter::Lowpass,
    }
}

fn preset_config(name: &str) -> PipelineConfig {
    let mut config = PipelineConfig::default();
    match name.to_lowercase().as_str() {
        "clean-broadcast" => {
            config.channel.luma_noise = 0.0;
            config.tape.flutter_depth = 0.02;
            config.artifacts.head_switch_intensity = 0.1;
            config.artifacts.dropout_rate = 0.0;
            config.demodulation.filter = DemodulationFilter::Comb2D;
        }
        "damaged-tape" => {
            config.tape.dropout_rate = 0.08;
            config.artifacts.dropout_rate = 0.08;
            config.artifacts.head_switch_intensity = 0.6;
            config.artifacts.saturation_strength = 0.5;
        }
        "severe-tracking" => {
            config.tape.tracking_error = 0.4;
            config.artifacts.horizontal_tbc_amplitude = 0.008;
            config.artifacts.vertical_jitter_amplitude = 0.006;
            config.demodulation.filter = DemodulationFilter::Box;
        }
        "vintage-camcorder" => {
            config.channel.chroma_bandwidth_mhz = 0.8;
            config.precision.vhs_chroma_bandwidth_mhz = 0.6;
            config.artifacts.chroma_phase_drift_depth = 0.4;
            config.demodulation.filter = DemodulationFilter::Notch;
        }
        _ => {
            config.tape.flutter_depth = 0.2;
            config.artifacts.chroma_phase_drift_depth = 0.3;
            config.demodulation.filter = DemodulationFilter::Comb1D;
        }
    }
    config
}
