# BoilR Development Environment Setup Script
# Run this script in PowerShell as Administrator

param(
    [switch]$SkipRust,
    [switch]$SkipBuildTools
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "BoilR Development Environment Setup" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if running as administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "WARNING: Not running as Administrator. Some installations may fail." -ForegroundColor Yellow
    Write-Host "Consider re-running this script as Administrator." -ForegroundColor Yellow
    Write-Host ""
}

# Function to check if a command exists
function Test-Command {
    param($Command)
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = 'stop'
    try {
        if (Get-Command $Command) { return $true }
    }
    catch { return $false }
    finally { $ErrorActionPreference = $oldPreference }
}

# Function to refresh environment variables
function Update-Environment {
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")

    # Also refresh CARGO_HOME and RUSTUP_HOME if they exist
    $cargoHome = [System.Environment]::GetEnvironmentVariable("CARGO_HOME", "User")
    if ($cargoHome) { $env:CARGO_HOME = $cargoHome }

    $rustupHome = [System.Environment]::GetEnvironmentVariable("RUSTUP_HOME", "User")
    if ($rustupHome) { $env:RUSTUP_HOME = $rustupHome }
}

# Check for winget (Windows Package Manager)
Write-Host "Checking for Windows Package Manager (winget)..." -ForegroundColor Yellow
if (Test-Command "winget") {
    Write-Host "  [OK] winget is available" -ForegroundColor Green
    $hasWinget = $true
} else {
    Write-Host "  [MISSING] winget not found - will use direct downloads" -ForegroundColor Yellow
    $hasWinget = $false
}
Write-Host ""

# Check/Install Visual Studio Build Tools
if (-not $SkipBuildTools) {
    Write-Host "Checking for Visual Studio Build Tools..." -ForegroundColor Yellow

    # Check for cl.exe (MSVC compiler)
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $hasBuildTools = $false

    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsPath) {
            Write-Host "  [OK] Visual Studio Build Tools found at: $vsPath" -ForegroundColor Green
            $hasBuildTools = $true
        }
    }

    if (-not $hasBuildTools) {
        Write-Host "  [MISSING] Visual Studio Build Tools not found" -ForegroundColor Yellow
        Write-Host "  Installing Visual Studio Build Tools..." -ForegroundColor Yellow

        if ($hasWinget) {
            Write-Host "  Using winget to install..." -ForegroundColor Cyan
            winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
        } else {
            Write-Host "  Downloading Visual Studio Build Tools installer..." -ForegroundColor Cyan
            $vsInstaller = "$env:TEMP\vs_buildtools.exe"
            Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile $vsInstaller
            Write-Host "  Running installer (this may take several minutes)..." -ForegroundColor Cyan
            Start-Process -FilePath $vsInstaller -ArgumentList "--wait", "--passive", "--add", "Microsoft.VisualStudio.Workload.VCTools", "--includeRecommended" -Wait
            Remove-Item $vsInstaller -Force -ErrorAction SilentlyContinue
        }
        Write-Host "  [OK] Visual Studio Build Tools installed" -ForegroundColor Green
    }
} else {
    Write-Host "Skipping Visual Studio Build Tools check (--SkipBuildTools)" -ForegroundColor Yellow
}
Write-Host ""

# Check/Install Rust
if (-not $SkipRust) {
    Write-Host "Checking for Rust toolchain..." -ForegroundColor Yellow

    # Refresh PATH in case it was just installed
    Update-Environment

    if (Test-Command "rustc") {
        $rustVersion = rustc --version
        Write-Host "  [OK] Rust is installed: $rustVersion" -ForegroundColor Green

        # Check cargo too
        if (Test-Command "cargo") {
            $cargoVersion = cargo --version
            Write-Host "  [OK] Cargo is installed: $cargoVersion" -ForegroundColor Green
        }
    } else {
        Write-Host "  [MISSING] Rust not found" -ForegroundColor Yellow
        Write-Host "  Installing Rust via rustup..." -ForegroundColor Yellow

        if ($hasWinget) {
            Write-Host "  Using winget to install rustup..." -ForegroundColor Cyan
            winget install Rustlang.Rustup
        } else {
            Write-Host "  Downloading rustup-init.exe..." -ForegroundColor Cyan
            $rustupInit = "$env:TEMP\rustup-init.exe"
            Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupInit
            Write-Host "  Running rustup installer..." -ForegroundColor Cyan
            Start-Process -FilePath $rustupInit -ArgumentList "-y", "--default-toolchain", "stable" -Wait
            Remove-Item $rustupInit -Force -ErrorAction SilentlyContinue
        }

        # Refresh environment after installation
        Update-Environment

        # Add cargo bin to current session path
        $cargobin = "$env:USERPROFILE\.cargo\bin"
        if (Test-Path $cargobin) {
            $env:Path = "$cargobin;$env:Path"
        }

        if (Test-Command "rustc") {
            $rustVersion = rustc --version
            Write-Host "  [OK] Rust installed successfully: $rustVersion" -ForegroundColor Green
        } else {
            Write-Host "  [WARNING] Rust installed but not in PATH. You may need to restart your terminal." -ForegroundColor Yellow
        }
    }
} else {
    Write-Host "Skipping Rust check (--SkipRust)" -ForegroundColor Yellow
}
Write-Host ""

# Update Rust toolchain
Write-Host "Updating Rust toolchain..." -ForegroundColor Yellow
if (Test-Command "rustup") {
    rustup update stable 2>&1 | Out-Host
    Write-Host "  [OK] Rust toolchain updated" -ForegroundColor Green
} else {
    Write-Host "  [SKIP] rustup not available" -ForegroundColor Yellow
}
Write-Host ""

# Summary
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Setup Complete!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Final verification
Write-Host "Final verification:" -ForegroundColor Yellow
Update-Environment

$allGood = $true

if (Test-Command "rustc") {
    Write-Host "  [OK] rustc: $(rustc --version)" -ForegroundColor Green
} else {
    Write-Host "  [FAIL] rustc not found" -ForegroundColor Red
    $allGood = $false
}

if (Test-Command "cargo") {
    Write-Host "  [OK] cargo: $(cargo --version)" -ForegroundColor Green
} else {
    Write-Host "  [FAIL] cargo not found" -ForegroundColor Red
    $allGood = $false
}

Write-Host ""

if ($allGood) {
    Write-Host "All tools are installed! You can now build the project with:" -ForegroundColor Green
    Write-Host "  cd E:\Source\BoilR" -ForegroundColor White
    Write-Host "  cargo build --release" -ForegroundColor White
    Write-Host ""
    Write-Host "To run the project:" -ForegroundColor Green
    Write-Host "  cargo run --release" -ForegroundColor White
} else {
    Write-Host "Some tools are missing. Please restart your terminal and try again," -ForegroundColor Yellow
    Write-Host "or manually install the missing components." -ForegroundColor Yellow
}

Write-Host ""
