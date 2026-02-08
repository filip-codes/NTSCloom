# Artifact Modeling Notes

## Head Switching
- Model a band near the bottom of the frame where head switching occurs.
- Apply additive luma discontinuity and chroma phase perturbation.
- Parameters: band height, intensity, randomness, phase distortion.

## Vertical Jitter
- Apply a vertical sync timing offset modeled as a low-frequency sine.
- This modulates carrier phase per scanline to emulate transport drift.

## Horizontal Timebase Error (TBC Instability)
- Per-scanline phase offset with low-frequency flutter + noise term.
- Luma and chroma share the same carrier phase error in the prototype.

## Chroma Phase Drift
- Slow phase offset added to chroma carrier over time.
- Approximates VHS color instability and aging tape.

## Dropout Clusters
- Stochastic bursts that add noise spikes to the composite waveform.
- Cluster length controls how long a dropout persists.

## Luma/Chroma Crosstalk
- Dynamic leakage: `Y += 0.03 * chroma_signal`.
- High-frequency luma leaks into chroma via `I/Q += 0.02 * high_luma`.

## Tape Saturation / Nonlinear Amplifier
- Soft clip transfer: `y = x(1+k)/(1+k|x|)` for configurable `k`.

## Demodulation Filters
- **Lowpass**: synchronous demodulation + lowpass integration.
- **Box**: moving average on I/Q (kernel size configurable).
- **Notch**: subtract chroma energy from luma using demodulated chroma.
- **1D Comb**: line-delay add/subtract for Y/C separation.
- **2D Comb**: two-line correlation for improved separation.

## Simplifications
- No full RF modulator path; artifacts are applied in composite domain.
- Line and frame timing use deterministic oscillators instead of full PLL sync recovery.
- Composite resampling uses windowed-sinc FIR with configurable taps for alias suppression.
