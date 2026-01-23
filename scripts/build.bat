@echo off
setlocal EnableDelayedExpansion

:: Add cargo to PATH
set PATH=%USERPROFILE%\.cargo\bin;%PATH%

:: Find and initialize Visual Studio environment
echo === Setting up Visual Studio Environment ===

:: Try VS 2022 Build Tools first
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    echo Found VS 2022 Build Tools
    call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    goto :build
)

:: Try VS 2022 Community
if exist "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" (
    echo Found VS 2022 Community
    call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
    goto :build
)

:: Try VS 2022 Professional
if exist "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat" (
    echo Found VS 2022 Professional
    call "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
    goto :build
)

:: Try VS 2022 Enterprise
if exist "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat" (
    echo Found VS 2022 Enterprise
    call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
    goto :build
)

:: Try VS 2019 Build Tools
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    echo Found VS 2019 Build Tools
    call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    goto :build
)

:: Try using vswhere to find VS
for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do (
    if exist "%%i\VC\Auxiliary\Build\vcvars64.bat" (
        echo Found VS at: %%i
        call "%%i\VC\Auxiliary\Build\vcvars64.bat"
        goto :build
    )
)

echo WARNING: Could not find Visual Studio environment setup script
echo Attempting build anyway...

:build
echo.

:: Change to project directory
cd /d E:\Source\BoilR

:: Show versions
echo === Build Environment ===
cargo --version
rustc --version
echo.

:: Build the project
echo === Building BoilR ===
cargo build --release

:: Check result
if %ERRORLEVEL% EQU 0 (
    echo.
    echo === Build Successful ===
    echo Binary location: E:\Source\BoilR\target\release\boilr.exe
) else (
    echo.
    echo === Build Failed with error code %ERRORLEVEL% ===
    exit /b %ERRORLEVEL%
)
