[CmdletBinding()]
param(
    [switch]$Check,
    [switch]$BuildOnly,
    [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path

function Write-Step([string]$Message) {
    Write-Host "`n> $Message" -ForegroundColor Cyan
}

function Write-Ok([string]$Message) {
    Write-Host "  $([char]0x2713) $Message" -ForegroundColor Green
}

function Stop-Setup([string]$Message) {
    Write-Error $Message
    exit 1
}

if (@($Check, $BuildOnly, $NoLaunch).Where({ $_ }).Count -gt 1) {
    Stop-Setup "Choose only one of -Check, -BuildOnly, or -NoLaunch."
}

Write-Host ""
Write-Host "+--------------------------------------+" -ForegroundColor White
Write-Host "|  DryMark - Setup                     |" -ForegroundColor White
Write-Host "|  Local clipboard watermark removal   |" -ForegroundColor DarkGray
Write-Host "+--------------------------------------+" -ForegroundColor White

Write-Step "Checking prerequisites"
if ($env:OS -ne "Windows_NT") {
    Stop-Setup "Run setup.ps1 from PowerShell on Windows. Use setup.sh on macOS or Linux."
}

$Node = Get-Command node -ErrorAction SilentlyContinue
$Npm = Get-Command npm -ErrorAction SilentlyContinue
if (-not $Node) { Stop-Setup "Node.js 22 or newer is required: https://nodejs.org" }
if (-not $Npm) { Stop-Setup "npm is required and normally ships with Node.js." }

$NodeVersion = (& node --version).TrimStart("v")
$NodeMajor = [int]($NodeVersion.Split(".")[0])
if ($NodeMajor -lt 22) {
    Stop-Setup "Node.js 22 or newer is required; found v$NodeVersion."
}
Write-Ok "Node.js v$NodeVersion"

$Cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $Cargo) {
    $Rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if ($Rustup) {
        $CargoPath = (& rustup which cargo 2>$null)
        if ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $CargoPath -PathType Leaf)) {
            $env:Path = "$(Split-Path -Parent $CargoPath);$env:Path"
            $Cargo = Get-Command cargo -ErrorAction SilentlyContinue
        }
    }
}
if (-not $Cargo) {
    Stop-Setup "Rust 1.97.1 is required. Install rustup from https://rustup.rs, then rerun setup."
}

$CargoVersion = (& cargo --version)
if (-not $CargoVersion.StartsWith("cargo 1.97.1 ")) {
    Stop-Setup "Rust 1.97.1 is required; found $CargoVersion. Run: rustup toolchain install 1.97.1"
}
Write-Ok "Rust 1.97.1"

if ($Check) {
    Write-Host "`n$([char]0x2713) Prerequisites ready.`n" -ForegroundColor Green
    return
}

Push-Location -LiteralPath $Root
try {
    Write-Step "Installing source dependencies"
    & npm ci
    if ($LASTEXITCODE -ne 0) { Stop-Setup "npm ci failed." }
    Write-Ok "Source dependencies ready"

    Write-Step "Building DryMark"
    & npm run tauri -- build --bundles nsis -- --locked
    if ($LASTEXITCODE -ne 0) { Stop-Setup "Native build failed." }

    $Installers = @(Get-ChildItem -LiteralPath "$Root\target\release\bundle\nsis" -Filter "*.exe" -File)
    if ($Installers.Count -ne 1 -or $Installers[0].Length -le 0) {
        Stop-Setup "Expected one non-empty DryMark installer."
    }
    Write-Ok "Native build complete"

    if ($BuildOnly) {
        Write-Host "`n$([char]0x2713) Build complete.`n" -ForegroundColor Green
        return
    }

    Write-Step "Installing DryMark"
    $InstallerProcess = Start-Process -FilePath $Installers[0].FullName -ArgumentList "/S" -Wait -PassThru
    if ($InstallerProcess.ExitCode -ne 0) {
        Stop-Setup "The DryMark installer exited with code $($InstallerProcess.ExitCode)."
    }
    Write-Ok "DryMark installed"

    if (-not $NoLaunch) {
        $InstalledExecutable = Join-Path $env:LOCALAPPDATA "DryMark\drymark-desktop.exe"
        if (-not (Test-Path -LiteralPath $InstalledExecutable -PathType Leaf)) {
            Stop-Setup "DryMark was installed, but its executable was not found at the expected location."
        }
        Start-Process -FilePath $InstalledExecutable
        Write-Ok "DryMark is open"
    }
} finally {
    Pop-Location
}

Write-Host "`n$([char]0x2713) Setup complete.`n" -ForegroundColor Green
