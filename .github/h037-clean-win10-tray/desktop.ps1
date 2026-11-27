function Wait-For([int]$seconds, [scriptblock]$probe) {
    $deadline = [DateTime]::UtcNow.AddSeconds($seconds)
    do {
        $value = & $probe
        if ($value) { return $value }
        Start-Sleep -Milliseconds 350
    } while ([DateTime]::UtcNow -lt $deadline)
    return $null
}

function Invoke-Page([string]$expression) {
    $sequence = ++$script:cdpSequence
    $request = @{id = $sequence; method = 'Runtime.evaluate'; params = @{
        expression = $expression; returnByValue = $true; awaitPromise = $true
    }} | ConvertTo-Json -Depth 5 -Compress
    $bytes = [Text.Encoding]::UTF8.GetBytes($request)
    $stop = [Threading.CancellationTokenSource]::new(6000)
    try {
        [void]$socket.SendAsync([ArraySegment[byte]]::new($bytes), [Net.WebSockets.WebSocketMessageType]::Text,
            $true, $stop.Token).GetAwaiter().GetResult()
        $buffer = New-Object byte[] 16384
        do {
            $message = [IO.MemoryStream]::new()
            try {
                do {
                    $fragment = $socket.ReceiveAsync([ArraySegment[byte]]::new($buffer), $stop.Token).GetAwaiter().GetResult()
                    if ($fragment.MessageType -eq [Net.WebSockets.WebSocketMessageType]::Close) {
                        throw 'closed_debug_socket'
                    }
                    $message.Write($buffer, 0, $fragment.Count)
                    if ($message.Length -gt 131072) { throw 'large_debug_response' }
                } until ($fragment.EndOfMessage)
                $reply = [Text.Encoding]::UTF8.GetString($message.ToArray()) | ConvertFrom-Json
            } finally {
                $message.Dispose()
            }
        } until ($reply.id -eq $sequence)
        if ($reply.error -or $reply.result.exceptionDetails) { throw 'page_evaluation_failed' }
        return $reply.result.result.value
    } finally {
        $stop.Dispose()
    }
}

function Has-Engine([string]$expected) {
    $state = Invoke-Page "window.__TAURI_INTERNALS__?.invoke('get_runtime_snapshot', {requestId: crypto.randomUUID()}).then(v => v.engine.status)"
    return $state -ceq $expected
}

function Test-BundledPython([string]$installRoot) {
    $python = Join-Path $installRoot 'runtime\payload\python.exe'
    if (!(Test-Path -LiteralPath $python -PathType Leaf)) { return @{ present = $false } }
    $start = [Diagnostics.ProcessStartInfo]::new($python, '-I -B --version')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $probe = [Diagnostics.Process]::Start($start)
    try {
        if (!$probe.WaitForExit(8000)) {
            $probe.Kill()
            return @{ present = $true; timed_out = $true }
        }
        $version = $probe.StandardOutput.ReadToEnd().Trim()
        $facts = @{ present = $true; exit = $probe.ExitCode }
        if ($version -cmatch '^Python \d+\.\d+\.\d+$') { $facts.version = $version }
        return $facts
    } finally {
        $probe.Dispose()
    }
}

function Publish-Progress([string]$event) {
    $row = ConvertTo-Json -Compress -Depth 6 -InputObject @{
        event = $event
        checks = $result.checks
        ready_seconds = $result.engine_ready_after_seconds
        setup_exit = $result.setup_exit
        installer_sha256 = $result.installer_sha256
    }
    [IO.File]::AppendAllText($progressFile, $row + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
}

function Get-EngineProcesses([int]$owner) {
    $python = Join-Path $destination 'runtime\payload\python.exe'
    $scriptPath = Join-Path $PSScriptRoot 'engine-processes.py'
    $snapshot = & $python -I -B $scriptPath $owner
    if ($LASTEXITCODE -ne 0) { throw 'engine_process_snapshot_failed' }
    return ($snapshot | ConvertFrom-Json)
}

function Test-Desktop {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $session = (Get-Process -Id $PID).SessionId
    $shell = Wait-For 35 {
        Get-Process -Name explorer -ErrorAction SilentlyContinue |
            Where-Object SessionId -eq $session | Select-Object -First 1
    }
    if (!$shell -or $session -eq 0) { throw 'interactive_session_unavailable' }
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'desktop_probe_elevated' }
    $result.checks.normal_interactive_token = $true

    $result.phase = 'nsis_installation'
    $registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Headroom'
    if ((Test-Path -LiteralPath $destination) -or (Test-Path -LiteralPath $profile) -or
        (Test-Path -LiteralPath $registration)) { throw 'existing_product_state' }
    $result.checks.no_previous_installation = $true
    $candidate = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'candidate.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $installer = Join-Path $PSScriptRoot 'setup.exe'
    $result.installer_sha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($result.installer_sha256 -cne $candidate.installer_sha256) { throw 'wrong_setup_digest' }
    $setup = Start-Process -FilePath $installer -ArgumentList "/S /NS /D=$destination" -WindowStyle Hidden -PassThru
    if (!$setup.WaitForExit(290000)) { throw 'nsis_timeout' }
    $result.setup_exit = $setup.ExitCode
    if ($setup.ExitCode -ne 0) { throw 'nsis_failed' }
    $installLocation = (Get-ItemProperty -LiteralPath $registration -Name InstallLocation).InstallLocation.Trim('"')
    if ($installLocation -ine $destination -or !(Test-Path -LiteralPath (Join-Path $destination 'uninstall.exe') -PathType Leaf)) {
        throw 'current_user_installation_not_registered'
    }
    $result.checks.current_user_nsis = $true
    $files = @{
        'headroom-desktop.exe' = @{ field = 'installed_exe_sha256'; expected = $candidate.executable_sha256 }
        'runtime/desktop_helper.py' = @{ field = 'installed_helper_sha256'; expected = $candidate.helper_sha256 }
        'runtime/manifest.json' = @{ field = 'installed_manifest_sha256'; expected = $candidate.manifest_sha256 }
        'package-inputs.json' = @{ field = 'installed_receipt_sha256'; expected = $candidate.resource_receipt_sha256 }
    }
    foreach ($name in $files.Keys) {
        $definition = $files[$name]
        $digest = (Get-FileHash -LiteralPath (Join-Path $destination $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($digest -cne $definition.expected) { throw 'installed_file_digest_mismatch' }
        $result[$definition.field] = $digest
    }
    $result.checks.installed_file_digests_match = $true
    Publish-Progress installed

    $result.phase = 'installed_window'
    $exe = Join-Path $destination 'headroom-desktop.exe'
    $result.installed_product = (Get-Item -LiteralPath $exe).VersionInfo.ProductName
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=49257 --remote-debugging-address=127.0.0.1'
    $launchTime = Get-Date
    $app = Start-Process -FilePath $exe -WorkingDirectory $destination -WindowStyle Normal -PassThru
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    if (!(Wait-For 55 { $app.Refresh(); $app.HasExited -or $app.MainWindowHandle -ne [IntPtr]::Zero })) {
        $result.window_wait_expired = $true
    }
    $app.Refresh()
    if ($app.HasExited -or $app.MainWindowHandle -eq [IntPtr]::Zero) {
        if ($app.HasExited) { $result.start_process_exit = $app.ExitCode }
        $events = Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = $launchTime; Id = 1000,1001,1002,1026,33 } -ErrorAction SilentlyContinue
        $result.desktop_fault_event_ids = @($events | Where-Object { $_.Message -match 'headroom-desktop\.exe|msedgewebview2\.exe' } |
            Select-Object -ExpandProperty Id -Unique)
        throw 'native_window_missing'
    }
    if ($app.MainWindowTitle -cne 'PennyPinch') {
        $result.window_caption_matches = $false
        throw 'unexpected_window_caption'
    }
    $result.checks.native_window = $true
    Publish-Progress window

    $result.phase = 'webview_control'
    $target = Wait-For 27 {
        try {
            (Invoke-RestMethod 'http://127.0.0.1:49257/json/list' -TimeoutSec 2) |
                Where-Object { $_.type -ceq 'page' -and $_.url -cmatch '^https?://tauri\.localhost/' } |
                Select-Object -First 1
        } catch [System.Net.WebException] {}
    }
    if (!$target) {
        $result.webview_debug_listener = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort 49257 -State Listen -ErrorAction SilentlyContinue).Count -gt 0
        $result.webview_process_count = @(Get-Process -Name msedgewebview2 -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session).Count
        throw 'webview_debug_target_missing'
    }
    if ($target.webSocketDebuggerUrl -cnotmatch '^ws://127\.0\.0\.1:49257/devtools/page/[A-Za-z0-9_-]+$') {
        throw 'webview_debug_address_mismatch'
    }
    $socket = [Net.WebSockets.ClientWebSocket]::new()
    try {
        $stop = [Threading.CancellationTokenSource]::new(8000)
        try { [void]$socket.ConnectAsync([uri]$target.webSocketDebuggerUrl, $stop.Token).GetAwaiter().GetResult() }
        finally { $stop.Dispose() }
        $script:cdpSequence = 0
        if (!(Wait-For 20 { $app.Refresh(); $app.MainWindowHandle -ne [IntPtr]::Zero -and (Has-Engine stopped) })) {
            throw 'desktop_did_not_start_stopped'
        }
        if (@((Get-EngineProcesses $app.Id).controller).Count -ne 0) { throw 'engine_already_running' }
        $result.checks.started_stopped = $true
        $result.phase = 'engine_start'
        $engineTimer = [Diagnostics.Stopwatch]::StartNew()
        if (!(Wait-For 30 { Invoke-Page "(() => { const b = document.querySelector('.optimization-control__primary'); if (!b || b.disabled || !/^(Start|Ba\u015flat)$/.test(b.textContent.trim())) return false; b.click(); return true; })()" })) {
            $result.last_engine = Invoke-Page "window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot', {requestId: crypto.randomUUID()}).then(v => v.engine)"
            throw 'engine_start_action_unavailable'
        }
        $result.checks.start_action_clicked = $true
        Publish-Progress start
        $statuses = [Collections.Generic.List[string]]::new()
        $terminal = Wait-For 155 {
            $engine = Invoke-Page "window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot', {requestId: crypto.randomUUID()}).then(v => v.engine)"
            if ($engine.status -and (!$statuses.Count -or $statuses[$statuses.Count - 1] -cne $engine.status)) {
                $statuses.Add($engine.status)
            }
            $result.last_engine = $engine
            if ($engine.status -in 'ready','failed','unavailable') { $engine }
        }
        $result.engine_statuses = @($statuses.ToArray())
        if (!$terminal) { throw 'engine_not_ready' }
        if ($terminal.status -cne 'ready') { throw 'engine_start_reported_error' }
        $result.engine_ready_after_seconds = [math]::Round($engineTimer.Elapsed.TotalSeconds, 3)
        $result.engine_version = Invoke-Page "window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot', {requestId: crypto.randomUUID()}).then(v => v.engine_version)"
        if ($result.engine_version -cne '0.45.0') { throw 'engine_version_mismatch' }
        $result.checks.engine_ready = $true
        Publish-Progress ready
        $endpointFile = Join-Path $profile 'runtime/connection-endpoint.json'
        $enginePort = (Get-Content -LiteralPath $endpointFile -Raw -Encoding UTF8 | ConvertFrom-Json).port
        if ($enginePort -lt 1 -or $enginePort -gt 65535) { throw 'engine_port_invalid' }
        $listener = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $enginePort -State Listen -ErrorAction SilentlyContinue)
        if (!$listener -or $listener[0].OwningProcess -ne $app.Id) { throw 'owned_route_not_listening' }
        $result.checks.owned_loopback_listener = $true
        $chain = Get-EngineProcesses $app.Id
        $result.process_chain_counts = @{controller = @($chain.controller).Count; helper = @($chain.helper).Count; engine = @($chain.engine).Count}
        if ($result.process_chain_counts.controller -ne 1 -or $result.process_chain_counts.helper -ne 1 -or
            $result.process_chain_counts.engine -ne 1) {
            throw 'installed_controller_chain_missing'
        }
        $ownedIds = @($chain.controller) + @($chain.helper) + @($chain.engine)
        $ownedProcesses = @($ownedIds | ForEach-Object { [Diagnostics.Process]::GetProcessById($_) })
        foreach ($owned in $ownedProcesses) { [void]$owned.Handle }
        $backend = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -State Listen -ErrorAction SilentlyContinue |
            Where-Object { $_.OwningProcess -eq $chain.engine[0] })
        if ($backend.Count -ne 1) { throw 'installed_engine_listener_missing' }
        $readiness = Invoke-RestMethod ("http://127.0.0.1:{0}/readyz" -f $backend[0].LocalPort) -TimeoutSec 4
        if (!$readiness.ready -or $readiness.version -cne '0.45.0' -or $readiness.rust_core -cne 'loaded' -or
            !$readiness.checks.startup.ready -or $readiness.checks.upstream.enabled -ne $false) { throw 'local_readiness_invalid' }
        $result.readiness = @{ready = $true; rust_core = 'loaded'; upstream_probe_enabled = $false}
        $result.installed_python_processes_after_start = $ownedProcesses.Count

        Test-NativeTray $app $enginePort $ownedProcesses $backend[0].LocalPort
    } finally {
        foreach ($owned in $ownedProcesses) { $owned.Dispose() }
        $socket.Dispose()
    }
}

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'tray.ps1')
$destination = Join-Path $env:LOCALAPPDATA 'PP QA'
$profile = Join-Path $env:LOCALAPPDATA 'dev.headroom.desktop'
$stateFile = Join-Path $env:LOCALAPPDATA 'pp-clean-desktop.json'
$progressFile = Join-Path $env:LOCALAPPDATA 'pp-clean-progress.jsonl'
$result = [ordered]@{ status = 'running'; phase = 'installed_window'; checks = [ordered]@{} }
try {
    Test-Desktop
    $result.status = 'pass'
    $result.phase = 'complete'
} catch {
    $result.status = 'fail'
    $result.error_type = $_.Exception.GetType().Name
    if ($_.Exception.Message -cmatch '^[a-z_]{1,70}$') { $result.error_code = $_.Exception.Message }
    $result.error_line = $_.InvocationInfo.ScriptLineNumber
    if ($result.phase -ceq 'engine_start') {
        try { $result.bundled_python = Test-BundledPython $destination }
        catch { $result.bundled_python = @{ error_type = $_.Exception.GetType().Name } }
    }
} finally {
    $tmp = "$stateFile.tmp"
    $encoded = ConvertTo-Json -InputObject $result -Compress -Depth 7
    [IO.File]::WriteAllText($tmp, $encoded, [Text.UTF8Encoding]::new($false))
    [IO.File]::Move($tmp, $stateFile)
}
