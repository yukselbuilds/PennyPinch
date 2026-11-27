function Initialize-NativeTray {
    Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class TrayProbe {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindow(string className, string windowName);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string className, string windowName);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint flags, int x, int y, uint data, UIntPtr extra);
}
'@
}

function Find-NativeControl([IntPtr]$window, [string]$pattern, [string]$kind) {
    if ($window -eq [IntPtr]::Zero -or ![TrayProbe]::IsWindowVisible($window)) { return }
    $type = if ($kind -eq 'menu') { [Windows.Automation.ControlType]::MenuItem } else { [Windows.Automation.ControlType]::Button }
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, $type)
    try {
        $group = [Windows.Automation.AutomationElement]::FromHandle($window)
        $group.FindAll([Windows.Automation.TreeScope]::Descendants, $condition) |
            Where-Object { $_.Current.Name -match $pattern -and !$_.Current.IsOffscreen } |
            Select-Object -First 1
    } catch [Windows.Automation.ElementNotAvailableException] {}
}

function Click-TrayControl($control, [bool]$right) {
    if ($control.Current.IsOffscreen -or !$control.Current.IsEnabled) { throw 'tray_control_disabled' }
    $bounds = $control.Current.BoundingRectangle
    if ($bounds.IsEmpty -or $bounds.Width -lt 1 -or $bounds.Height -lt 1) { throw 'empty_native_bounds' }
    if (![TrayProbe]::SetCursorPos([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))) {
        throw 'cannot_move_guest_pointer'
    }
    if ($right) {
        [TrayProbe]::mouse_event(8, 0, 0, 0, [UIntPtr]::Zero)
        [TrayProbe]::mouse_event(16, 0, 0, 0, [UIntPtr]::Zero)
    } else {
        [TrayProbe]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        [TrayProbe]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    }
}

function Find-TrayButton {
    $taskbar = [TrayProbe]::FindWindow('Shell_TrayWnd', [NullString]::Value)
    if ($taskbar -eq [IntPtr]::Zero) { return }
    $notificationArea = [TrayProbe]::FindWindowEx($taskbar, [IntPtr]::Zero, 'TrayNotifyWnd', [NullString]::Value)
    $pattern = '^PennyPinch(?:$| .+)'
    $icon = Find-NativeControl $notificationArea $pattern 'button'
    if ($icon) { return $icon }

    $overflow = [TrayProbe]::FindWindow('NotifyIconOverflowWindow', [NullString]::Value)
    if (![TrayProbe]::IsWindowVisible($overflow)) {
        $chevron = Find-NativeControl $taskbar 'Show hidden icons|Notification Chevron' 'button'
        if (!$chevron) { return }
        Click-TrayControl $chevron $false
    }
    Wait-For 4 {
        Find-NativeControl ([TrayProbe]::FindWindow('NotifyIconOverflowWindow', [NullString]::Value)) $pattern 'button'
    }
}

function Invoke-TrayItem([string]$label) {
    $icon = Wait-For 15 { Find-TrayButton }
    if (!$icon) { throw 'tray_icon_not_found' }
    Click-TrayControl $icon $true
    $item = Wait-For 12 { Find-NativeControl ([TrayProbe]::FindWindow('#32768', [NullString]::Value)) $label 'menu' }
    if (!$item) { throw 'tray_item_not_found' }
    Click-TrayControl $item $false
}

function Get-TrayRuntime {
    Invoke-Page "window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{requestId:crypto.randomUUID()}).then(v=>({engine:v.engine.status,mode:v.compression.status,revision:v.compression.revision,generation:v.runtime_generation}))"
}

function Test-NativeTray([Diagnostics.Process]$app, [int]$enginePort, $ownedProcesses, [int]$backendPort) {
    $result.phase = 'tray_initialize'
    Initialize-NativeTray
    $window = $app.MainWindowHandle
    $before = Get-TrayRuntime
    if ($window -eq [IntPtr]::Zero -or $before.engine -cne 'ready' -or $before.mode -cne 'enabled' -or
        !$before.generation -or $null -eq $before.revision) { throw 'invalid_tray_start_state' }
    $result.tray = [ordered]@{ modes = @('enabled'); revisions = @($before.revision) }

    $result.phase = 'tray_hide'
    if (!$app.CloseMainWindow() -or !(Wait-For 12 { !$app.HasExited -and ![TrayProbe]::IsWindowVisible($window) })) {
        throw 'window_did_not_hide'
    }
    $result.checks.close_hid_native_window = $true
    $hidden = Get-TrayRuntime
    if ($hidden.engine -cne 'ready' -or $hidden.generation -cne $before.generation -or $hidden.mode -cne 'enabled') {
        throw 'hide_interrupted_engine'
    }
    $result.checks.engine_ready_while_hidden = $true

    $result.phase = 'tray_open'
    Invoke-TrayItem '^(Open PennyPinch|PennyPinch.i a.)$'
    if (!(Wait-For 12 { !$app.HasExited -and [TrayProbe]::IsWindowVisible($window) })) { throw 'window_did_not_reopen' }
    $result.checks.reopened_from_native_tray = $true

    $result.phase = 'tray_pause'
    Invoke-TrayItem '^(Pause|Duraklat)$'
    $paused = Wait-For 22 {
        $snapshot = Get-TrayRuntime
        if ($snapshot.mode -ceq 'paused') { $snapshot }
    }
    if (!$paused -or $paused.engine -cne 'ready' -or $paused.generation -cne $before.generation -or
        $paused.revision -le $before.revision) { throw 'tray_pause_not_acknowledged' }
    $result.tray.modes += 'paused'
    $result.tray.revisions += $paused.revision

    $result.phase = 'tray_resume'
    Start-Sleep -Milliseconds 650
    Invoke-TrayItem '^(Resume|Devam et)$'
    $resumed = Wait-For 22 {
        $snapshot = Get-TrayRuntime
        if ($snapshot.mode -ceq 'enabled') { $snapshot }
    }
    if (!$resumed -or $resumed.engine -cne 'ready' -or $resumed.generation -cne $before.generation -or
        $resumed.revision -le $paused.revision -or @($ownedProcesses | Where-Object HasExited).Count) {
        throw 'tray_resume_not_acknowledged'
    }
    $result.tray.modes += 'enabled'
    $result.tray.revisions += $resumed.revision
    $result.checks.tray_pause_resume_same_generation = $true
    Publish-Progress tray

    $result.phase = 'tray_quit'
    Invoke-TrayItem '^(Quit.*|Tamamen .+)$'
    if (!(Wait-For 15 { Invoke-Page "(() => {const d=document.querySelector('dialog[open][data-desktop-exit-confirmation]');return Boolean(d?.querySelector('.desktop-exit-dialog__confirm:not(:disabled)'));})()" })) {
        throw 'quit_confirmation_missing'
    }
    if ($app.HasExited) { throw 'quit_exited_before_confirmation' }
    $result.checks.tray_quit_required_confirmation = $true
    $confirm = Invoke-Page "(() => {const b=document.querySelector('dialog[open][data-desktop-exit-confirmation] .desktop-exit-dialog__confirm:not(:disabled)');if(!b)return false;setTimeout(()=>b.click(),300);return true;})()"
    if (!$confirm -or !$app.WaitForExit(40000)) { throw 'tray_exit_timeout' }
    $result.desktop_exit = $app.ExitCode
    if ($app.ExitCode -ne 0) { throw 'tray_exit_failed' }
    if (!(Wait-For 14 { @($ownedProcesses | Where-Object { !$_.HasExited }).Count -eq 0 })) {
        throw 'engine_processes_survived_tray_exit'
    }
    foreach ($port in $enginePort, $backendPort) {
        if (Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $port -State Listen -ErrorAction SilentlyContinue) {
            throw 'listener_survived_tray_exit'
        }
    }
    $result.checks.normal_tray_exit = $true
    $result.checks.installed_python_processes_ended = $true
    $result.checks.backend_port_released = $true
    $result.checks.stable_listener_closed_on_exit = $true
    Publish-Progress exited
}
