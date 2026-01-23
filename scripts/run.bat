@echo off
setlocal

:: Change to project directory
cd /d E:\Source\BoilR

:: Set verbose logging
set RUST_LOG=boilr=debug,info

echo === Running BoilR ===
echo Log file will be at: %APPDATA%\boilr\boilr.log
echo.

:: Run the application
target\release\boilr.exe %*
