# NTSCloom Architecture

## Pipeline overview

1. **RGB → Linear → YIQ**
   - Linearize RGB before conversion.
   - Convert to YIQ using NTSC coefficients to preserve luma accuracy.
2. **Composite encoding (virtual voltages)**
   - Modulate chroma onto a 3.579545 MHz subcarrier using sin/cos.
   - Inject colorburst per scanline and apply phase offset/jitter.
   - Sample at ≥ 4× subcarrier (14.31818 MHz) and low-pass/anti-alias.
   - Resample composite back to pixel grid using windowed-sinc FIR to avoid aliasing.
3. **Analog channel + tape**
   - Front-end RC filters (luma/chroma low-pass, chroma band-pass).
   - Head/tape response (frequency roll-off, nonlinear saturation).
   - RF multipath (ghosting), phase noise, flutter/wow, dropouts.
4. **Decode composite → YIQ**
   - Use imperfect PLL, burst-based phase recovery.
   - Selectable demodulation filters (lowpass, box, notch, comb).
   - Apply chroma bleed, dot crawl (luma/chroma crosstalk), noise.
5. **YIQ → RGB + Output**
   - Convert with Rec.601 matrix, linear → sRGB, clamp/soft clip, dither.

## Block-based processing

- Frames are processed as scanline blocks for streaming large files.
- Composite waveform is generated per line with a time base that preserves subcarrier phase.

## GPU acceleration

- Use compute kernels for modulation/demodulation and filtering.
- CPU fallback uses SIMD for scanline processing.

## Optional RF path

- Composite → RF AM/FM modulation → channel multipath → downconvert.

## Core module layout

- `dsp.rs`: color space conversions, modulation/demod helpers.
- `pipeline.rs`: signal flow stages and artifact injection.
- `config.rs`: parameter structs with defaults.

See `docs/artifacts.md` for artifact equations and simplifications.
