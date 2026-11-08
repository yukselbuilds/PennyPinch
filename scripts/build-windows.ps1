[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$buildPython = Join-Path $projectRoot 'work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe'
if (-not (Test-Path -LiteralPath $buildPython -PathType Leaf)) {
    throw 'Pinned build Python is missing. The development payload must be prepared before packaging.'
}
$savedCargoHome = $env:CARGO_HOME
$savedCargoTarget = $env:CARGO_TARGET_DIR
$savedDirectory = Get-Location
try {
    Set-Location -LiteralPath $projectRoot
    $env:CARGO_HOME = Join-Path $projectRoot 'work\caches\cargo-home'
    $env:CARGO_TARGET_DIR = Join-Path $projectRoot 'work\caches\cargo-target'
    & $buildPython -I -B scripts/package-windows.py
    if ($LASTEXITCODE -ne 0) { throw 'Pinned Windows resource preparation failed; no installer build was started.' }
    & pnpm.cmd tauri build --config src-tauri/installer.conf.json --ci --no-sign -- --offline
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer build failed; no release readiness is implied.' }
} finally {
    $env:CARGO_HOME = $savedCargoHome
    $env:CARGO_TARGET_DIR = $savedCargoTarget
    Set-Location -LiteralPath $savedDirectory
}
