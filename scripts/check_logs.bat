@echo off
echo === Checking BoilR Config Folder ===
echo Location: %APPDATA%\boilr
echo.

if exist "%APPDATA%\boilr" (
    echo Folder exists. Contents:
    dir "%APPDATA%\boilr"
    echo.

    if exist "%APPDATA%\boilr\boilr.log" (
        echo === Last 50 lines of log file ===
        powershell -Command "Get-Content '%APPDATA%\boilr\boilr.log' -Tail 50"
    ) else (
        echo Log file not found.
    )
) else (
    echo Config folder does not exist.
)

echo.
echo === Checking if BoilR is running ===
tasklist /FI "IMAGENAME eq boilr.exe" 2>NUL
