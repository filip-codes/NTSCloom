# Testing & Reference Vectors

## Reference vectors
- SMPTE color bars
- High-contrast edges (luma ringing/overshoot)
- Fine chroma patterns (dot crawl, chroma bleed)
- Moving bars (temporal smear, head switching)

## Automated tests

- YIQ round-trip accuracy
- Composite modulation frequency sanity checks
- Artifact toggles (noise, dropouts)

## Benchmarks

- Target: 1080p @ 30fps realtime preview on mid-tier GPU.
- Provide timing for encode, channel, decode stages.
