# NTSCloom

NTSCloom is an experimental cross-platform desktop program/project for emulating real NTSC composite and VHS chains in the digital domain. The goal is not a simple shader or filter, but a signal-accurate pipeline that encodes RGB into a composite waveform (virtual voltages), tries simulates analog channel and tape behaviors, then decodes back to RGB with realistic artifacts.

## Repo structure

- `crates/ntscloom-core`: Signal pipeline, DSP, and test vectors.
- `apps/cli`: Batch renderer prototype (headless).
- `apps/gui`: Desktop GUI application (egui/eframe).
- `docs/`: Architecture, UI/UX specification, parameters, and testing.

## Quick start

```bash
cargo test
```

To run the GUI:

```bash
cargo run -p ntscloom-gui
```

## Build plan (high level)

- **Native UI (recommended)**: Qt 6 + C++/Rust core via C ABI (FFI). Rendering via GPU compute (Vulkan/Metal/D3D12) with CPU fallback.
- **Electron prototype**: Electron + WebGL2/WebGPU preview, core DSP in Rust/WASM or native module.

See `docs/architecture.md` for pipeline details, `docs/ui.md` for UI/UX layout, and `docs/artifacts.md` for artifact equations.

## Reuse & Contributions

Feel free to use, modify, or integrate parts of this project into your own projects! Credit is always appreciated, but completely optional.
