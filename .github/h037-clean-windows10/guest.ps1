$ErrorActionPreference = 'Stop'
$serial = [IO.Ports.SerialPort]::new('COM1', 115200)
$serial.NewLine = [string][char]10
$serial.Open()
$timer = [Diagnostics.Stopwatch]::StartNew()
$result = [ordered]@{
    schema_version = 1
    status = 'running'
    phase = 'desktop_preflight'
    checks = [ordered]@{}
}
function Send-Receipt([string]$kind, $data) {
    $json = ConvertTo-Json -InputObject $data -Compress -Depth 9
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($json))
    $serial.WriteLine("PPAUDIT $kind $encoded")
}
function Get-Preflight {
    $out = & (Join-Path $PSScriptRoot 'scripts/check-windows-preview.ps1') -Archive $archive -Target Windows10
    $out | ConvertFrom-Json
}
try {
    $archive = Join-Path $PSScriptRoot 'preview.zip'
    $before = Get-Preflight
    if ($before.status -ne 'ready') { throw 'preflight_failed' }
    $result.before = $before
    $system = Get-CimInstance Win32_OperatingSystem
    $build = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $result.windows = [ordered]@{
        caption = $system.Caption
        version = $system.Version
        ubr = [int]$build.UBR
        display_version = $build.DisplayVersion
    }
    if ($build.DisplayVersion -ne '22H2' -or $system.Caption -notmatch 'Evaluation') {
        throw 'unexpected_edition'
    }
    $devTools = @(Get-Command cargo,rustc,node,pnpm,py,cl,msbuild -ErrorAction SilentlyContinue)
    if ($devTools.Count -ne 0) { throw 'development_tools_present' }
    $result.checks.no_dev_tools_in_path = $true
    $destination = 'C:\PP-Verification'
    if (Test-Path -LiteralPath $destination) { throw 'existing_installation' }
    $profile = Join-Path $env:LOCALAPPDATA 'dev.headroom.desktop'
    if (Test-Path -LiteralPath $profile) { throw 'existing_app_state' }
    $result.checks.fresh_installation = $true

    $result.phase = 'setup_integrity'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $installer = Join-Path $env:TEMP 'PennyPinch_0.1.1-preview.8_x64-setup.exe'
    $bundle = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $entry = $bundle.GetEntry([IO.Path]::GetFileName($installer))
        if ($null -eq $entry) { throw 'setup_not_found' }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $installer)
    } finally {
        $bundle.Dispose()
    }
    if ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ine $before.installer_sha256) {
        throw 'setup_hash_mismatch'
    }
    $result.checks.setup_sha256_verified = $true

    $result.phase = 'setup_network'
    $netBefore = @(Get-NetIPConfiguration | Where-Object IPv4DefaultGateway)
    if ($netBefore.Count -ne 0) { throw 'network_connected_before_baseline' }
    Send-Receipt before @{preflight = $before; windows = $result.windows; already_online = $false}
    $limit = [DateTime]::UtcNow.AddSeconds(70)
    do {
        $ready = @(Get-NetIPConfiguration | Where-Object IPv4DefaultGateway).Count -gt 0
        if ($ready) { break }
        Start-Sleep -Seconds 2
    } while ([DateTime]::UtcNow -lt $limit)
    if (!$ready) { throw 'guest_network_timeout' }

    $result.phase = 'nsis_installation'
    $immediate = Get-Preflight
    if ($immediate.status -ne 'ready') { throw 'pre_setup_recheck_failed' }
    $result.webview2_immediately_before = @($immediate.webview2_registry_versions | Where-Object { $_ })
    Register-CimIndicationEvent -Query 'SELECT * FROM Win32_ProcessStartTrace' -SourceIdentifier 'PPSetupProcess' | Out-Null
    try {
        $setup = Start-Process -FilePath $installer -ArgumentList "/S /D=$destination" -PassThru
        if (!$setup.WaitForExit(240000)) { throw 'setup_timeout' }
        $result.setup_exit = $setup.ExitCode
        $bootstrapEvents = @(Get-Event -SourceIdentifier PPSetupProcess | Where-Object {
            $_.SourceEventArgs.NewEvent.ProcessName -ieq 'MicrosoftEdgeWebview2Setup.exe'
        })
        $result.webview2_bootstrapper_seen = $bootstrapEvents.Count -gt 0
        $result.webview2_bootstrapper_from_setup = @($bootstrapEvents | Where-Object {
            $_.SourceEventArgs.NewEvent.ParentProcessID -eq $setup.Id
        }).Count -gt 0
        if ($setup.ExitCode -ne 0) { throw 'setup_failed' }
    } finally {
        Unregister-Event -SourceIdentifier PPSetupProcess
    }
    $result.checks.nsis_installation = $true

    $result.phase = 'webview2_after_installation'
    $after = Get-Preflight
    if ($after.status -ne 'ready' -or !$after.webview2_registry_versions) { throw 'runtime_not_registered' }
    $result.after = $after
    $result.checks.webview2_installed_by_setup = $result.webview2_immediately_before.Count -eq 0 -and
        $result.webview2_bootstrapper_from_setup
    Send-Receipt installed @{checks = $result.checks; before = $result.webview2_immediately_before;
        after = $after.webview2_registry_versions; bootstrapper_from_setup = $result.webview2_bootstrapper_from_setup}

    $result.phase = 'installed_desktop'
    . (Join-Path $PSScriptRoot 'desktop.ps1')
    Test-Desktop
    $result.status = 'pass'
    $result.phase = 'complete'
} catch {
    $result.status = 'fail'
    $result.error_type = $_.Exception.GetType().Name
    if ($_.Exception.Message -cmatch '^[a-z_]{1,70}$') { $result.error_code = $_.Exception.Message }
    $result.error_line = $_.InvocationInfo.ScriptLineNumber
} finally {
    $result.guest_test_seconds = [math]::Round($timer.Elapsed.TotalSeconds, 3)
    Send-Receipt complete $result
    $serial.Dispose()
}
