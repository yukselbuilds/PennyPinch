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
$savedReleaseCandidate = $env:HEADROOM_RELEASE_EXE_CANDIDATE
$savedEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
$savedDirectory = Get-Location
try {
    Set-Location -LiteralPath $projectRoot
    $env:CARGO_HOME = Join-Path $projectRoot 'work\caches\cargo-home'
    $env:CARGO_TARGET_DIR = Join-Path $projectRoot 'work\caches\cargo-target'
    & $buildPython -I -B scripts/package-windows.py
    if ($LASTEXITCODE -ne 0) { throw 'Pinned Windows resource preparation failed; no installer build was started.' }
    $separator = [char]31
    $flags = @()
    if ($savedEncodedFlags) {
        $flags = @($savedEncodedFlags -split $separator)
    } elseif ($env:RUSTFLAGS) {
        $flags = @($env:RUSTFLAGS -split '\s+' | Where-Object { $_ })
    } elseif ($env:CARGO_BUILD_RUSTFLAGS) {
        $flags = @($env:CARGO_BUILD_RUSTFLAGS -split '\s+' | Where-Object { $_ })
    }
    $flags += "--remap-path-prefix=$projectRoot=/pennypinch"
    if ($env:USERPROFILE) {
        $flags += "--remap-path-prefix=$env:USERPROFILE=/build-user"
    }
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join $separator
    & pnpm.cmd tauri build --config src-tauri/installer.conf.json --ci --no-sign -- --offline
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer build failed; no release readiness is implied.' }
    $env:HEADROOM_RELEASE_EXE_CANDIDATE = Join-Path $env:CARGO_TARGET_DIR 'release\headroom-desktop.exe'
    & $buildPython -I -B -m unittest discover -s tests/runtime -p test_windows_release_executable.py
    if ($LASTEXITCODE -ne 0) { throw 'Windows release executable validation failed; the installer is not ready.' }
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $savedEncodedFlags
    $env:CARGO_HOME = $savedCargoHome
    $env:CARGO_TARGET_DIR = $savedCargoTarget
    $env:HEADROOM_RELEASE_EXE_CANDIDATE = $savedReleaseCandidate
    Set-Location -LiteralPath $savedDirectory
}
