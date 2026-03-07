@echo off
setlocal

REM Build wasm package into docs/pkg and serve docs on localhost.
REM Clipboard APIs require a secure context; localhost qualifies.
set MODE=%~1

if /I "%MODE%"=="serve-only" goto serve

where wasm-pack >nul 2>nul
if errorlevel 1 (
  echo [ERROR] wasm-pack is not installed or not on PATH.
  echo Install it with: cargo install wasm-pack
  exit /b 1
)

where clang >nul 2>nul
if errorlevel 1 (
  echo [ERROR] clang was not found on PATH.
  echo This project needs clang to compile zstd for wasm32.
  echo Install LLVM clang, then open a new terminal.
  echo Example: winget install -e --id LLVM.LLVM
  exit /b 1
)

where rustup >nul 2>nul
if not errorlevel 1 (
  rustup target list --installed | findstr /C:"wasm32-unknown-unknown" >nul
  if errorlevel 1 (
    echo [INFO] Adding rust target wasm32-unknown-unknown...
    rustup target add wasm32-unknown-unknown
    if errorlevel 1 (
      echo [ERROR] Failed to add wasm32-unknown-unknown target.
      exit /b 1
    )
  )
)

echo [1/2] Building wasm package...
wasm-pack build .\web --target web --release --out-dir ..\docs\pkg
if errorlevel 1 (
  echo [ERROR] wasm build failed.
  exit /b 1
)

if not exist docs\pkg\brz_symmetry_web.js (
  echo [ERROR] Build did not produce docs\pkg\brz_symmetry_web.js
  exit /b 1
)

:serve
set URL=http://localhost:8080
set PY_CMD=
if exist "%LocalAppData%\Programs\Python\Python313\python.exe" set "PY_CMD="%LocalAppData%\Programs\Python\Python313\python.exe""
if not defined PY_CMD if exist "%LocalAppData%\Programs\Python\Python312\python.exe" set "PY_CMD="%LocalAppData%\Programs\Python\Python312\python.exe""
if not defined PY_CMD if exist "%LocalAppData%\Programs\Python\Python311\python.exe" set "PY_CMD="%LocalAppData%\Programs\Python\Python311\python.exe""
if not defined PY_CMD if exist "%LocalAppData%\Programs\Python\Python310\python.exe" set "PY_CMD="%LocalAppData%\Programs\Python\Python310\python.exe""
if not defined PY_CMD if exist "%LocalAppData%\Programs\Python\Python39\python.exe" set "PY_CMD="%LocalAppData%\Programs\Python\Python39\python.exe""
if not defined PY_CMD (
  where py >nul 2>nul
  if not errorlevel 1 set PY_CMD=py -3
)
if not defined PY_CMD (
  where python >nul 2>nul
  if not errorlevel 1 (
    set PY_VER=
    for /f "tokens=2 delims= " %%V in ('python -V 2^>^&1') do set PY_VER=%%V
    if not "%PY_VER:~0,1%"=="2" set PY_CMD=python
  )
)
if not defined PY_CMD (
  echo [ERROR] Python was not found ^(checked 'python' and 'py -3'^).
  echo [ERROR] Note: Python 2 is not supported; Python 3 is required.
  echo Install Python 3 and ensure it is on PATH.
  exit /b 1
)

if /I "%MODE%"=="serve-only" (
  echo [1/1] Starting local server at %URL%
) else (
  echo [2/2] Starting local server at %URL%
)
echo Using: %PY_CMD%
echo Press Ctrl+C to stop.
start "" %URL% >nul 2>nul
pushd docs
%PY_CMD% -m http.server 8080
set EXITCODE=%ERRORLEVEL%
popd
if not "%EXITCODE%"=="0" (
  echo [ERROR] Local server exited with code %EXITCODE%.
  exit /b %EXITCODE%
)

endlocal
