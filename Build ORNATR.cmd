@echo off
cd /d "%~dp0"
set CARGO=%USERPROFILE%\.cargo\bin\cargo.exe
if not exist "%CARGO%" (echo Rust is not installed. Install it from https://rustup.rs > build-log.txt & type build-log.txt & timeout /t 15 & exit /b 1)
echo Building ORNATR. The first build downloads libraries and takes several minutes...
"%CARGO%" build --release -p ornatr > build-log.txt 2>&1
if errorlevel 1 (echo BUILD FAILED>> build-log.txt & echo Build failed. Details are in build-log.txt & timeout /t 15 & exit /b 1)
copy /y target\release\ornatr.exe ORNATR.exe > nul
if errorlevel 1 (echo COPY FAILED: ORNATR is still open. Close it and build again.>> build-log.txt & echo ORNATR is still open - close it and run this again. & timeout /t 15 & exit /b 1)
echo BUILD OK>> build-log.txt
echo Done: ORNATR.exe
timeout /t 5
