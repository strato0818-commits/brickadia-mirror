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

## Web Prototype (GitHub Pages)

This repo now includes a static web prototype in `docs/` and a wasm crate in `web/`.

- Upload `.brz` in the browser
- Validate and process entirely client-side (no backend)
- Download the output `.brz`

Current processor is a BRZ roundtrip pipeline with entity guard (same limitation as desktop):
- If entities are present, it returns an error.
- Full in-browser symmetry transform is the next step.

Build wasm package into `docs/pkg`:

```powershell
$env:CC_wasm32_unknown_unknown='C:\Program Files\LLVM\bin\clang.exe'
$env:AR_wasm32_unknown_unknown='C:\Program Files\LLVM\bin\llvm-ar.exe'
wasm-pack build .\web --target web --release --out-dir ..\docs\pkg
```

Preview locally (from repo root):

```powershell
python -m http.server 8080 -d docs
```

Then open `http://localhost:8080`.
