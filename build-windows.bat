@echo off
setlocal

cd /d "%~dp0"

echo [1/2] Building release binary...
cargo build --release
if errorlevel 1 (
  echo Build failed.
  exit /b 1
)

if not exist dist mkdir dist
copy /Y "target\release\brz-symmetry.exe" "dist\brz-symmetry.exe" >nul
if errorlevel 1 (
  echo Failed to copy executable to dist.
  exit /b 1
)

echo [2/2] Done.
echo Output: %cd%\dist\brz-symmetry.exe
exit /b 0
