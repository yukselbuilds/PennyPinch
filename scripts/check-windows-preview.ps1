[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [ValidateSet('Windows10', 'Windows11')][string]$Target = 'Windows11',
    [ValidateSet('Current', 'SignedUpdate')][string]$Scenario = 'Current'
)

$ErrorActionPreference = 'Stop'
$report = [ordered]@{
    schema_version = 1
    scope = 'preflight'
    target = $Target
    scenario = $Scenario
    powershell = $PSVersionTable.PSVersion.ToString()
}
$status = 'windows_facts_unavailable'

try {
    $system = Get-CimInstance -ClassName Win32_OperatingSystem -Property Caption, BuildNumber, ProductType
    $architectures = @(Get-CimInstance -ClassName Win32_Processor -Property Architecture |
        Select-Object -ExpandProperty Architecture -Unique)
    $build = [int]$system.BuildNumber
    $client = $system.ProductType -eq 1
    $x64 = [Environment]::Is64BitOperatingSystem -and $architectures.Count -eq 1 -and $architectures[0] -eq 9
    $report.windows = [ordered]@{
        build = $build
        workstation = $client
        processor_architecture_codes = $architectures
        x64 = $x64
    }
    $matchesTarget = if ($Target -eq 'Windows10') {
        $build -eq 19045 -and $system.Caption -match '\bWindows 10\b'
    } else {
        $build -ge 22000 -and $system.Caption -match '\bWindows 11\b'
    }
    if (!$client -or !$x64 -or !$matchesTarget) {
        $status = 'target_mismatch'
        throw 'Use the selected Windows desktop version on x64.'
    }

    $status = 'release_evidence_unavailable'
    $proofs = Join-Path $PSScriptRoot '../docs/proofs'
    $evidence = if ($Scenario -eq 'Current') {
        Join-Path $proofs 'pennypinch-windows10-preview8-local.json'
    } else {
        Join-Path $proofs 'pennypinch-windows10-installed-update-and-release.json'
    }
    $package = (Get-Content -LiteralPath $evidence -Raw -Encoding UTF8 | ConvertFrom-Json).delivery
    if ($package.archive.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        $package.installer.sha256 -cnotmatch '^[0-9a-f]{64}$' -or $package.archive.bytes -le 0) {
        throw 'Use the release evidence from the PennyPinch repository.'
    }

    $status = 'archive_unavailable'
    $file = Get-Item -LiteralPath $Archive
    $status = 'archive_mismatch'
    if ($file -isnot [System.IO.FileInfo] -or $file.Length -ne $package.archive.bytes -or
        (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash -ne $package.archive.sha256) {
        throw 'Use the ZIP for the selected test.'
    }
    $report.archive = $package.archive.name
    $report.archive_sha256 = $package.archive.sha256
    $report.installer_sha256 = $package.installer.sha256

    $status = 'webview2_registry_unavailable'
    # Choose the registry views explicitly, including from a 32-bit PowerShell process.
    $key = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $locations = @(
        @{ Hive = [Microsoft.Win32.RegistryHive]::LocalMachine; View = [Microsoft.Win32.RegistryView]::Registry32 }
        @{ Hive = [Microsoft.Win32.RegistryHive]::CurrentUser; View = [Microsoft.Win32.RegistryView]::Registry64 }
    )
    $versions = @(foreach ($location in $locations) {
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($location.Hive, $location.View)
        try {
            $entry = $base.OpenSubKey($key)
            if ($null -eq $entry) { continue }
            try {
                $value = $entry.GetValue('pv')
            } finally {
                $entry.Dispose()
            }
            if ([string]::IsNullOrWhiteSpace($value)) { continue }
            if ($value -cnotmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid WebView2 version.' }
            $version = [version]$value
            if ($version -gt [version]'0.0.0.0') { $version.ToString() }
        } finally {
            $base.Dispose()
        }
    })
    $report.webview2_registry_versions = @($versions | Sort-Object -Unique)
    $status = 'ready'
}
catch {
    # The report uses fixed failure codes and excludes paths and exception text.
}

$report.status = $status
$report | ConvertTo-Json -Depth 3 -Compress
if ($status -ne 'ready') { exit 1 }
