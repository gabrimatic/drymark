$ErrorActionPreference = "Stop"

$RepositoryRoot = Split-Path -Parent $PSScriptRoot
$TestRoot = Join-Path ([System.IO.Path]::GetTempPath()) "drymark-powershell-setup-$PID"
$FakeBin = Join-Path $TestRoot "bin"
$Caller = Join-Path $TestRoot "caller"
$OriginalPath = $env:Path
$OriginalOs = $env:OS

try {
    New-Item -ItemType Directory -Force -Path $FakeBin, $Caller | Out-Null
    Set-Content -LiteralPath (Join-Path $FakeBin "node.cmd") -Value "@echo v22.0.0"
    Set-Content -LiteralPath (Join-Path $FakeBin "npm.cmd") -Value "@exit /b 99"
    Set-Content -LiteralPath (Join-Path $FakeBin "cargo.cmd") -Value "@echo cargo 1.97.1 (test fixture)"

    $env:Path = "$FakeBin;$env:Path"
    $env:OS = "Windows_NT"
    Push-Location -LiteralPath $Caller
    try {
        $Before = (Get-Location).Path
        & (Join-Path $RepositoryRoot "setup.ps1") -Check | Out-Null
        $After = (Get-Location).Path
        if ($After -ne $Before) {
            throw "setup.ps1 changed the caller directory from $Before to $After"
        }
    } finally {
        Pop-Location
    }

    Write-Host "PowerShell setup preserves the caller directory"
} finally {
    $env:Path = $OriginalPath
    $env:OS = $OriginalOs
    Remove-Item -LiteralPath $TestRoot -Recurse -Force -ErrorAction SilentlyContinue
}
