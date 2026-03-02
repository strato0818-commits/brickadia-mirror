# brz-symmetry

Standalone Rust desktop tool for Brickadia `.brz` symmetry.

## Features

- Open an input `.brz`
- Apply symmetry across `X`, `Y`, or `Z`
- Save the mirrored result as another `.brz`
- Uses your existing mirror orientation rules from `omegga.plugin.js`

## Known Issues
- prefabs with multiple entities not supported. If you want to mirror multitple entites, break it up and perform in multiple passes

## Run

```powershell
cd symmetry-rust
cargo run --release
```
