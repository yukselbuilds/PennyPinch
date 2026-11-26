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
        $socket.SendAsync([ArraySegment[byte]]::new($bytes), [Net.WebSockets.WebSocketMessageType]::Text,
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

function Find-Element($parent, [string]$name, [string]$type) {
    $control = switch ($type) {
        button { [Windows.Automation.ControlType]::Button }
        menu { [Windows.Automation.ControlType]::MenuItem }
    }
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, $control)
    try {
        $parent.FindAll([Windows.Automation.TreeScope]::Descendants, $condition) |
            Where-Object { $_.Current.Name -match $name -and !$_.Current.IsOffscreen } |
            Select-Object -First 1
    } catch [Windows.Automation.ElementNotAvailableException] {}
}

function Find-TopLevel([string]$class) {
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty, $class)
    $root.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
}

function Click-Native($element, [bool]$right) {
    $bounds = $element.Current.BoundingRectangle
    if ($bounds.IsEmpty -or $bounds.Width -lt 1 -or $bounds.Height -lt 1) { throw 'empty_native_bounds' }
    if (![Pointer]::SetCursorPos([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))) {
        throw 'cannot_move_guest_pointer'
    }
    if ($right) {
        [Pointer]::mouse_event(8, 0, 0, 0, [UIntPtr]::Zero)
        [Pointer]::mouse_event(16, 0, 0, 0, [UIntPtr]::Zero)
    } else {
        [Pointer]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        [Pointer]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    }
}

function Find-TrayIcon {
    $pattern = '^PennyPinch(?:$| .+)'
    $taskbar = Find-TopLevel 'Shell_TrayWnd'
    if (!$taskbar) { return }
    $icon = Find-Element $taskbar $pattern button
    if ($icon) { return $icon }
    $overflow = Find-TopLevel 'NotifyIconOverflowWindow'
    if ($overflow) {
        $icon = Find-Element $overflow $pattern button
        if ($icon) { return $icon }
    }
    $chevron = Find-Element $taskbar 'Show hidden icons|Notification Chevron' button
    if ($chevron) {
        Click-Native $chevron $false
        Wait-For 4 {
            $popup = Find-TopLevel 'NotifyIconOverflowWindow'
            if ($popup) { Find-Element $popup $pattern button }
        }
    }
}

function Invoke-Tray([string]$label) {
    $icon = Wait-For 12 { Find-TrayIcon }
    if (!$icon) { throw 'tray_icon_not_found' }
    Click-Native $icon $true
    $item = Wait-For 8 {
        $menu = Find-TopLevel '#32768'
        if ($menu) { Find-Element $menu $label menu }
    }
    if (!$item) { throw 'tray_item_not_found' }
    Click-Native $item $false
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

    $exe = Join-Path $destination 'headroom-desktop.exe'
    if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'installed_executable_not_found' }
    $result.installed_exe_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
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
        try { $socket.ConnectAsync([uri]$target.webSocketDebuggerUrl, $stop.Token).GetAwaiter().GetResult() }
        finally { $stop.Dispose() }
        $script:cdpSequence = 0
        if (!(Wait-For 20 { $app.Refresh(); $app.MainWindowHandle -ne [IntPtr]::Zero -and (Has-Engine stopped) })) {
            throw 'desktop_did_not_start_stopped'
        }
        $result.phase = 'engine_start'
        if (!(Wait-For 18 { Invoke-Page "(() => { const b = document.querySelector('.optimization-control__primary'); if (!b || b.disabled) return false; b.click(); return true; })()" })) {
            throw 'engine_start_control_missing'
        }
        if (!(Wait-For 155 { Has-Engine ready })) { throw 'engine_not_ready' }
        $result.engine_version = Invoke-Page "window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot', {requestId: crypto.randomUUID()}).then(v => v.engine_version)"
        if ($result.engine_version -cne '0.45.0') { throw 'engine_version_mismatch' }
        $result.checks.engine_ready = $true
        $endpointFile = Join-Path $profile 'runtime/connection-endpoint.json'
        $enginePort = (Get-Content -LiteralPath $endpointFile -Raw -Encoding UTF8 | ConvertFrom-Json).port
        if ($enginePort -lt 1 -or $enginePort -gt 65535) { throw 'engine_port_invalid' }
        $listener = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $enginePort -State Listen -ErrorAction SilentlyContinue)
        if (!$listener -or $listener[0].OwningProcess -ne $app.Id) { throw 'owned_route_not_listening' }
        $result.checks.owned_loopback_listener = $true

        $result.phase = 'engine_stop'
        if (!(Invoke-Page "(() => {const d = document.querySelector('.optimization-control__more'); if (!d) return false; d.open = true; const b = d.querySelector('button'); if (!b || b.disabled) return false; b.click(); return true;})()")) {
            throw 'engine_stop_control_missing'
        }
        if (!(Wait-For 50 {
            (Has-Engine stopped) -and (Invoke-Page "window.__TAURI_INTERNALS__.invoke('get_ingress_snapshot', {requestId: crypto.randomUUID()}).then(v => v.backend.status === 'stopped' && v.endpoint.status === 'ready')")
        })) {
            throw 'engine_backend_not_stopped'
        }
        $result.checks.engine_stopped_with_stable_route = $true
        $result.phase = 'tray_close_and_reopen'
        Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Pointer {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(int flags, int x, int y, int data, UIntPtr extra);
}
'@
        $root = [Windows.Automation.AutomationElement]::RootElement
        [void]$app.CloseMainWindow()
        if (!(Wait-For 8 { $app.Refresh(); !$app.HasExited -and $app.MainWindowHandle -eq [IntPtr]::Zero })) {
            throw 'window_did_not_hide'
        }
        Invoke-Tray '^(Open PennyPinch|PennyPinch.i a.)$'
        if (!(Wait-For 8 { $app.Refresh(); $app.MainWindowHandle -ne [IntPtr]::Zero })) {
            throw 'window_did_not_reopen'
        }
        $result.checks.tray_reopen = $true

        $result.phase = 'tray_exit'
        Invoke-Tray '^(Quit.*|Tamamen .+)$'
        if (!(Wait-For 12 { Invoke-Page "(() => { const b = [...document.querySelectorAll('button')].find(x => /^(Confirm exit|Quit|.{1}.* onayla)$/i.test(x.textContent.trim()) && x.getClientRects().length && !x.disabled); if (!b) return false; setTimeout(() => b.click(), 100); return true; })()" })) {
            throw 'exit_confirmation_missing'
        }
        if (!$app.WaitForExit(30000)) { throw 'desktop_exit_timeout' }
        $result.desktop_exit = $app.ExitCode
        if ($app.ExitCode -ne 0) { throw 'desktop_failed_exit' }
        $result.checks.tray_exit = $true
        if (Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $enginePort -State Listen -ErrorAction SilentlyContinue) {
            throw 'stable_listener_survived_exit'
        }
        $result.checks.stable_listener_closed_on_exit = $true
    } finally {
        $socket.Dispose()
    }
}

$ErrorActionPreference = 'Stop'
$destination = 'C:\PP-Verification'
$profile = Join-Path $env:LOCALAPPDATA 'dev.headroom.desktop'
$stateFile = Join-Path $env:LOCALAPPDATA 'pp-clean-desktop.json'
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
} finally {
    $tmp = "$stateFile.tmp"
    $encoded = ConvertTo-Json -InputObject $result -Compress -Depth 7
    [IO.File]::WriteAllText($tmp, $encoded, [Text.UTF8Encoding]::new($false))
    [IO.File]::Move($tmp, $stateFile)
}
