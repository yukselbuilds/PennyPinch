$ErrorActionPreference = 'Stop'
$serial = [IO.Ports.SerialPort]::new('COM1', 115200)
$serial.NewLine = [string][char]10
$serial.WriteTimeout = 5000
$serial.Open()
$timer = [Diagnostics.Stopwatch]::StartNew()
$result = [ordered]@{schema_version = 1; status = 'running'; phase = 'preflight'; checks = [ordered]@{}}

function Send-Receipt([string]$kind, $data) {
    $json = ConvertTo-Json -InputObject $data -Compress -Depth 8
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($json))
    $serial.WriteLine("PPAUDIT $kind $encoded")
}

function Get-WebViewVersion {
    $key = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    foreach ($path in "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}", "HKCU:\$key") {
        if (Test-Path -LiteralPath $path) {
            $observed = (Get-ItemProperty -LiteralPath $path -Name pv -ErrorAction SilentlyContinue).pv
            if ($observed -match '^\d+\.\d+\.\d+\.\d+$' -and $observed -ne '0.0.0.0') { $observed }
        }
    }
}

try {
    Send-Receipt guest_entry @{started = $true}
    $system = Get-CimInstance Win32_OperatingSystem
    $build = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $architecture = @(Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Architecture -Unique)
    if ($system.BuildNumber -ne '26100' -or $system.ProductType -ne 1 -or
        $build.DisplayVersion -ne '24H2' -or !$architecture.Count -or $architecture[0] -ne 9) { throw 'incorrect_windows_target' }
    $guestAdapters = @(Get-NetAdapter -Physical)
    if ($guestAdapters.Count -ne 1) { throw 'unexpected_guest_network_adapter_count' }
    $guestAdapter = $guestAdapters[0]
    Disable-NetAdapter -InputObject $guestAdapter -Confirm:$false
    if ((Get-NetAdapter -InterfaceIndex $guestAdapter.ifIndex).Status -ne 'Disabled') { throw 'guest_network_not_disabled' }
    $networkReady = Join-Path $env:LOCALAPPDATA 'pp-clean-network-ready'
    if (Test-Path -LiteralPath $networkReady) { throw 'existing_network_receipt' }
    if (@(Get-Command cargo,rustc,node,pnpm,py,cl,msbuild -ErrorAction SilentlyContinue).Count) { throw 'development_tools_present' }
    $priorWebView = @(Get-WebViewVersion | Sort-Object -Unique)
    $secureBoot = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State' -Name UEFISecureBootEnabled).UEFISecureBootEnabled
    $tpm = Get-CimInstance -Namespace 'root/cimv2/security/microsofttpm' -ClassName Win32_Tpm
    if ($secureBoot -ne 1 -or $tpm.SpecVersion -notmatch '^2\.0') { throw 'secure_boot_or_tpm_missing' }
    $media = @(Get-Volume | Where-Object { $_.DriveType -eq 'CD-ROM' -and $_.DriveLetter -and $_.FileSystemLabel -ne 'PPAUDIT' } |
        ForEach-Object { '{0}:\setup.exe' -f $_.DriveLetter } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($media.Count -ne 1) { throw 'unexpected_iso_setup_source' }
    $signature = Get-AuthenticodeSignature -LiteralPath $media[0]
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Microsoft') { throw 'iso_setup_signature_failed' }
    $result.windows = @{build = 26100; display_version = '24H2'; architecture = 9; ubr = [int]$build.UBR; edition = $build.EditionID; secure_boot = $true; tpm2 = $true}
    $result.checks.microsoft_setup_signature_valid = $true
    $result.checks.no_dev_tools_in_path = $true
    $result.checks.network_disconnected = $true
    $result.webview2_before = $priorWebView
    Send-Receipt before @{preflight = @{status = 'ready'; windows = $result.windows}; already_online = $false}

    $result.phase = 'offline_webview2'
    $runtimeInstaller = Join-Path $PSScriptRoot 'webview2.exe'
    if ((Get-FileHash -LiteralPath $runtimeInstaller -Algorithm SHA256).Hash -ine '45769d66809a9652050f8a91ab5329881b65cc4123fe7c733841deefbef91bf6') {
        throw 'webview2_hash_mismatch'
    }
    $runtimeExit = $null
    if (!$priorWebView.Count) {
        $setup = Start-Process -FilePath $runtimeInstaller -ArgumentList '/silent /install' -WindowStyle Hidden -PassThru
        if (!$setup.WaitForExit(160000)) { throw 'webview2_setup_timeout' }
        $runtimeExit = $setup.ExitCode
    }
    $result.webview2 = @(Get-WebViewVersion | Sort-Object -Unique)
    if (($null -ne $runtimeExit -and $runtimeExit -ne 0) -or !$result.webview2.Count) { throw 'webview2_setup_failed' }
    Send-Receipt runtime @{exit = $runtimeExit; versions = $result.webview2}

    $result.phase = 'interactive_test'
    $report = Join-Path $env:LOCALAPPDATA 'pp-clean-desktop.json'
    $progress = Join-Path $env:LOCALAPPDATA 'pp-clean-progress.jsonl'
    if ((Test-Path -LiteralPath $report) -or (Test-Path -LiteralPath $progress)) { throw 'existing_desktop_receipt' }
    $desktop = Join-Path $PSScriptRoot 'desktop.ps1'
    $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument ('-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "{0}"' -f $desktop)
    $principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
    $taskName = 'PennyPinchCleanDesktopVerification'
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal | Out-Null
    Start-ScheduledTask -TaskName $taskName
    $deadline = [DateTime]::UtcNow.AddSeconds(1100)
    $sent = 0
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $progress) {
            $rows = @(Get-Content -LiteralPath $progress -Encoding UTF8)
            while ($sent -lt $rows.Count) {
                try { $item = $rows[$sent] | ConvertFrom-Json -ErrorAction Stop }
                catch { break }
                if ($item.event -cnotmatch '^(installed|window|start|ready|stopped|exited)$') { throw 'invalid_desktop_progress' }
                Send-Receipt $item.event $item
                if ($item.event -ceq 'exited') {
                    Enable-NetAdapter -InputObject (Get-NetAdapter -InterfaceIndex $guestAdapter.ifIndex) -Confirm:$false
                    $waitForNetwork = [DateTime]::UtcNow.AddSeconds(95)
                    do {
                        $network = Get-NetIPConfiguration -InterfaceIndex $guestAdapter.ifIndex -ErrorAction SilentlyContinue
                        if ($network.NetAdapter.Status -eq 'Up' -and $network.IPv4DefaultGateway) { break }
                        Start-Sleep -Seconds 1
                    } while ([DateTime]::UtcNow -lt $waitForNetwork)
                    if ($network.NetAdapter.Status -ne 'Up' -or !$network.IPv4DefaultGateway) { throw 'signed_update_network_unavailable' }
                    [IO.File]::WriteAllText($networkReady, 'ready')
                    $result.checks.network_ready_for_signed_update = $true
                }
                $sent += 1
            }
        }
        if (Test-Path -LiteralPath $report) { break }
        Start-Sleep -Seconds 1
    }
    if (!(Test-Path -LiteralPath $report)) {
        $result.task_exit = (Get-ScheduledTaskInfo -TaskName $taskName).LastTaskResult
        throw 'desktop_receipt_missing'
    }
    $result.desktop = Get-Content -LiteralPath $report -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($result.desktop.status -ne 'pass') { throw 'desktop_lifecycle_failed' }
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
