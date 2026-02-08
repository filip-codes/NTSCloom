use approx::assert_relative_eq;
use ntscloom_core::{process_frame, rgb_to_yiq, yiq_to_rgb, Frame, PipelineConfig};

#[test]
fn yiq_roundtrip_preserves_luma() {
    let yiq = rgb_to_yiq(0.8, 0.2, 0.1);
    let (r, g, b) = yiq_to_rgb(yiq);
    let yiq_out = rgb_to_yiq(r, g, b);
    assert_relative_eq!(yiq.y, yiq_out.y, epsilon = 0.01);
}

#[test]
fn pipeline_outputs_frame() {
    let mut frame = Frame::new(2, 2);
    frame.data = vec![
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
        0.0, 0.0, 1.0,
        1.0, 1.0, 1.0,
    ];

    let config = PipelineConfig::default();
    let out = process_frame(&frame, &config, 14_318_180.0);
    assert_eq!(out.data.len(), frame.data.len());
}

#[test]
fn uniform_frame_has_low_column_variance() {
    let mut frame = Frame::new(64, 64);
    frame.data.fill(0.5);

    let mut config = PipelineConfig::default();
    config.channel.luma_noise = 0.0;
    config.tape.flutter_depth = 0.0;
    config.tape.tracking_error = 0.0;
    config.artifacts.head_switch_enabled = false;
    config.artifacts.vertical_jitter_enabled = false;
    config.artifacts.horizontal_tbc_enabled = false;
    config.artifacts.chroma_phase_drift_enabled = false;
    config.artifacts.dropout_enabled = false;
    config.artifacts.saturation_enabled = false;
    let out = process_frame(&frame, &config, 14_318_180.0);

    let mut column_means = Vec::with_capacity(frame.width);
    for x in 0..frame.width {
        let mut sum = 0.0;
        for y in 0..frame.height {
            let idx = (y * frame.width + x) * 3;
            let luma = (out.data[idx] + out.data[idx + 1] + out.data[idx + 2]) / 3.0;
            sum += luma;
        }
        column_means.push(sum / frame.height as f32);
    }
    let mut sorted = column_means.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let max_dev = column_means
        .iter()
        .map(|v| (v - median).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_dev < 0.02, "column variance too high: {max_dev}");
}
