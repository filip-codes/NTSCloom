# Parameters

## Composite
- Subcarrier phase offset: −180°..+180°
- Burst amplitude: 0..2.0
- Chroma level: 0..2.0

## Chroma
- Chroma bandwidth: 0.1..6 MHz
- Lowpass slope
- I/Q phase noise: 0..100°
- Dot crawl intensity: 0..1

## Luma
- Luma bandwidth: 0.1..8 MHz
- Luma ringing: 0..1
- Luma noise: 0..1

## Tape / VHS
- Flutter rate: 0.1..20 Hz
- Flutter depth: 0..1
- Tracking error frequency/amplitude
- Dropout frequency/length
- Tape hiss: −60..0 dB

## Artifacts
- Head switching band height/intensity/randomness/phase distortion
- Vertical jitter frequency/amplitude
- Horizontal timebase error frequency/amplitude
- Chroma phase drift rate/depth
- Dropout clusters rate/length
- Dynamic luma/chroma crosstalk
- Saturation strength

## Demodulation
- Lowpass, Box, Notch, 1D Comb, 2D Comb
- Box kernel size
- Notch bandwidth/depth
- Comb strength

## Precision
- Oversample factor (preview/full)
- Resample taps (preview/full)
- Fix vertical stripes (sinc resampling + AA)
- PLL phase noise and lock slew
- VHS chroma bandwidth
- Chroma delay variation

## Debug
- Diagnostic mode
- Show composite waveform
- Show demodulated I/Q
- Show diagnostic grid

## Noise & RF
- AWGN level: dB
- Color noise texture scale
- RF interference amplitude/frequency

## Temporal
- Motion blur: 0..1 frames
- Frame jitter
- Interlace combing intensity

## Output
- Export bit depth
- Dithering
- Color space conversion (Rec.709, sRGB)
- Wet/Dry mix
