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

    $result.phase = 'offline_runtime_setup'
    $netBefore = @(Get-NetIPConfiguration | Where-Object IPv4DefaultGateway)
    if ($netBefore.Count -ne 0) { throw 'network_connected_before_baseline' }
    Send-Receipt before @{preflight = $before; windows = $result.windows; already_online = $false}
    if ($before.webview2_registry_versions.Count -ne 0) { throw 'webview2_present_before_test' }
    $runtimeInstaller = Join-Path $PSScriptRoot 'webview2.exe'
    $result.webview2_installer_sha256 = (Get-FileHash -LiteralPath $runtimeInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($result.webview2_installer_sha256 -cne '45769d66809a9652050f8a91ab5329881b65cc4123fe7c733841deefbef91bf6') { throw 'runtime_installer_hash_mismatch' }
    $runtimeSetup = Start-Process -FilePath $runtimeInstaller -ArgumentList '/silent /install' -PassThru
    if (!$runtimeSetup.WaitForExit(150000)) { throw 'offline_runtime_setup_timeout' }
    $result.webview2_setup_exit = $runtimeSetup.ExitCode
    if ($runtimeSetup.ExitCode -ne 0) { throw 'offline_runtime_setup_failed' }
    $immediate = Get-Preflight
    if ($immediate.status -ne 'ready' -or !$immediate.webview2_registry_versions) { throw 'runtime_not_registered' }
    $result.webview2_before_pennypinch = @($immediate.webview2_registry_versions)
    Send-Receipt runtime @{versions = $result.webview2_before_pennypinch; exit = $runtimeSetup.ExitCode}

    $result.phase = 'nsis_installation'
    $setup = Start-Process -FilePath $installer -ArgumentList "/S /D=$destination" -PassThru
    if (!$setup.WaitForExit(150000)) { throw 'setup_timeout' }
    $result.setup_exit = $setup.ExitCode
    if ($setup.ExitCode -ne 0) { throw 'setup_failed' }
    $result.checks.nsis_installation = $true

    $installedExe = Join-Path $destination 'headroom-desktop.exe'
    $result.installed_exe_sha256 = (Get-FileHash -LiteralPath $installedExe -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($result.installed_exe_sha256 -cne '3b9e27afc01d07efd13d0aa7602209da5ab25dadbd659a09849dc3b92c0f5cd2') {
        throw 'installed_executable_hash_mismatch'
    }
    Send-Receipt installed @{checks = $result.checks; exe_sha256 = $result.installed_exe_sha256}

    $result.phase = 'verified_candidate'
    $candidateHashes = @(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'candidate.sha256'))
    if ($candidateHashes.Count -ne 2 -or @($candidateHashes | Where-Object { $_ -cnotmatch '^[a-f0-9]{64}$' }).Count) {
        throw 'invalid_candidate_digests'
    }
    foreach ($file in @(
        @{ name = 'candidate.exe'; destination = $installedExe; sha256 = $candidateHashes[0] },
        @{ name = 'desktop_helper.py'; destination = (Join-Path $destination 'runtime/desktop_helper.py'); sha256 = $candidateHashes[1] }
    )) {
        $inputFile = Join-Path $PSScriptRoot $file.name
        if ((Get-FileHash -LiteralPath $inputFile -Algorithm SHA256).Hash -ine $file.sha256) { throw 'candidate_input_mismatch' }
        Copy-Item -LiteralPath $inputFile -Destination $file.destination -Force
        if ((Get-FileHash -LiteralPath $file.destination -Algorithm SHA256).Hash -ine $file.sha256) { throw 'candidate_install_mismatch' }
    }
    $result.candidate_exe_sha256 = $candidateHashes[0]
    $result.candidate_helper_sha256 = $candidateHashes[1]
    $result.checks.candidate_files_verified = $true
    Send-Receipt candidate @{ executable = $candidateHashes[0]; helper = $candidateHashes[1] }

    $result.phase = 'interactive_desktop'
    $desktopScript = Join-Path $PSScriptRoot 'desktop.ps1'
    $stateFile = Join-Path $env:LOCALAPPDATA 'pp-clean-desktop.json'
    if (Test-Path -LiteralPath $stateFile) { throw 'existing_desktop_result' }
    $command = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File $desktopScript"
    $principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
    $taskName = 'PennyPinchCleanDesktopVerification'
    Register-ScheduledTask -TaskName $taskName -Action $command -Principal $principal | Out-Null
    Start-ScheduledTask -TaskName $taskName
    $deadline = [DateTime]::UtcNow.AddSeconds(390)
    $progressFile = Join-Path $env:LOCALAPPDATA 'pp-clean-progress.jsonl'
    $sent = 0
    do {
        if (Test-Path -LiteralPath $progressFile) {
            $observed = @(Get-Content -LiteralPath $progressFile -Encoding UTF8)
            while ($sent -lt $observed.Count) {
                try { $item = $observed[$sent] | ConvertFrom-Json -ErrorAction Stop }
                catch { break }
                if ($item.event -cnotmatch '^(window|start|ready|stopped|exited)$') { throw 'invalid_desktop_progress' }
                Send-Receipt ('desktop_' + $item.event) $item
                $sent += 1
            }
        }
        if (Test-Path -LiteralPath $stateFile) {
            $result.desktop = Get-Content -LiteralPath $stateFile -Raw -Encoding UTF8 | ConvertFrom-Json
            break
        }
        Start-Sleep -Seconds 2
    } while ([DateTime]::UtcNow -lt $deadline)
    if (!$result.desktop) {
        $result.task_exit = (Get-ScheduledTaskInfo -TaskName $taskName).LastTaskResult
        throw 'desktop_receipt_missing'
    }
    if ($result.desktop.status -ne 'pass') { throw 'desktop_check_failed' }
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
