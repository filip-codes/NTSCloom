pub mod config;
pub mod dsp;
pub mod pipeline;

pub use config::{
    ArtifactConfig, ChannelConfig, CompositeConfig, DebugConfig, DemodulationConfig,
    DemodulationFilter, OutputConfig, PipelineConfig, PrecisionConfig, TapeConfig,
};
pub use dsp::{rgb_to_yiq, yiq_to_rgb, CompositeSample, Yiq};
pub use pipeline::{process_frame, Frame, FrameFormat};
pub use pipeline::process_frame_with_progress;
