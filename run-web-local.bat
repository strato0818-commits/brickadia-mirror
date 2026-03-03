@echo off
setlocal

REM Build wasm package into docs/pkg
where wasm-pack >nul 2>nul
if errorlevel 1 (
  echo [ERROR] wasm-pack is not installed or not on PATH.
  echo Install it with: cargo install wasm-pack
  exit /b 1
)

echo [1/2] Building wasm package...
wasm-pack build .\web --target web --release --out-dir ..\docs\pkg
if errorlevel 1 (
  echo [ERROR] wasm build failed.
  exit /b 1
)

REM Serve docs directory on localhost
where python >nul 2>nul
if errorlevel 1 (
  echo [ERROR] python is not installed or not on PATH.
  exit /b 1
)

echo [2/2] Starting local server at http://localhost:8080
echo Press Ctrl+C to stop.
python -m http.server 8080 -d docs

endlocal
