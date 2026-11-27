function Initialize-NativeTray {
    Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
public static class TrayProbe {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindow(string className, string windowName);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string className, string windowName);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr window, out Bounds rectangle);
    [DllImport("user32.dll")]
    public static extern int GetSystemMetrics(int metric);
    [DllImport("user32.dll")]
    public static extern bool SetProcessDPIAware();

    [StructLayout(LayoutKind.Sequential)]
    public struct Bounds {
        public int Left, Top, Right, Bottom;

        public int X { get { return Left + (Right - Left) / 2; } }
        public int Y { get { return Top + (Bottom - Top) / 2; } }
        public bool HasArea { get { return Right > Left && Bottom > Top; } }
        public bool Contains(int x, int y) { return HasArea && x >= Left && x < Right && y >= Top && y < Bottom; }
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct IconIdentity {
        public uint Size;
        public IntPtr Window;
        public uint Id;
        public Guid Guid;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct MenuBarInfo {
        public uint Size;
        public Bounds Bounds;
        public IntPtr Menu;
        public IntPtr MenuWindow;
        public uint FocusFlags;
    }

    [DllImport("shell32.dll")]
    private static extern int Shell_NotifyIconGetRect(ref IconIdentity icon, out Bounds rectangle);
    [DllImport("user32.dll")]
    private static extern bool GetMenuBarInfo(IntPtr window, int objectId, int item, ref MenuBarInfo info);
    [DllImport("user32.dll")]
    private static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll")]
    private static extern bool GetMenuItemRect(IntPtr window, IntPtr menu, uint item, out Bounds rectangle);
    [DllImport("user32.dll")]
    private static extern uint GetMenuState(IntPtr menu, uint item, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetMenuString(IntPtr menu, uint item, StringBuilder text, int capacity, uint flags);

    public static IntPtr FindOwnedWindow(string windowClass, int processId) {
        IntPtr window = IntPtr.Zero;
        for (int i = 0; i < 64; i++) {
            window = FindWindowEx(IntPtr.Zero, window, windowClass, null);
            if (window == IntPtr.Zero) break;
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == processId) return window;
        }
        return IntPtr.Zero;
    }

    public static Bounds GetIconBounds(IntPtr owner) {
        if (owner == IntPtr.Zero) return new Bounds();
        IconIdentity icon = new IconIdentity();
        icon.Window = owner;
        icon.Size = (uint)Marshal.SizeOf(typeof(IconIdentity));
        // The pinned tray-icon crate assigns a small sequential ID.
        for (icon.Id = 0; icon.Id < 32; icon.Id++) {
            Bounds bounds;
            if (Shell_NotifyIconGetRect(ref icon, out bounds) == 0 && bounds.HasArea) return bounds;
        }
        return new Bounds();
    }

    public static Bounds GetMenuItemBounds(IntPtr popup, string pattern) {
        if (popup == IntPtr.Zero || !IsWindowVisible(popup)) return new Bounds();
        MenuBarInfo info = new MenuBarInfo();
        info.Size = (uint)Marshal.SizeOf(typeof(MenuBarInfo));
        if (!GetMenuBarInfo(popup, -4, 0, ref info) || info.Menu == IntPtr.Zero) return new Bounds();
        const uint byPosition = 0x0400;
        int count = GetMenuItemCount(info.Menu);
        for (uint item = 0; count > 0 && count <= 12 && item < count; item++) {
            StringBuilder text = new StringBuilder(128);
            if (GetMenuString(info.Menu, item, text, text.Capacity, byPosition) == 0) continue;
            if (!Regex.IsMatch(text.ToString(), pattern)) continue;
            if ((GetMenuState(info.Menu, item, byPosition) & 3) != 0) continue;
            Bounds bounds;
            if (GetMenuItemRect(popup, info.Menu, item, out bounds)) return bounds;
        }
        return new Bounds();
    }
}
'@
    [void][TrayProbe]::SetProcessDPIAware()
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

function Click-TrayPoint([int]$x, [int]$y, [bool]$right) {
    $row = ConvertTo-Json -Compress -InputObject @{
        event = 'pointer'
        phase = $result.phase
        x = $x
        y = $y
        right = $right
        width = [TrayProbe]::GetSystemMetrics(0)
        height = [TrayProbe]::GetSystemMetrics(1)
    }
    [IO.File]::AppendAllText($progressFile, $row + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
}

function Get-TrayBounds([IntPtr]$handle) {
    $rectangle = New-Object TrayProbe+Bounds
    if ([TrayProbe]::IsWindowVisible($handle) -and [TrayProbe]::GetWindowRect($handle, [ref]$rectangle)) { return $rectangle }
}

function Invoke-TrayItem([int]$processId, [string]$label) {
    $owner = [TrayProbe]::FindOwnedWindow('tray_icon_app', $processId)
    $taskbar = [TrayProbe]::FindWindow('Shell_TrayWnd', [NullString]::Value)
    if ($owner -eq [IntPtr]::Zero -or $taskbar -eq [IntPtr]::Zero) { throw 'tray_owner_or_shell_missing' }
    $area = [TrayProbe]::FindWindowEx($taskbar, [IntPtr]::Zero, 'TrayNotifyWnd', [NullString]::Value)
    $chevron = Get-TrayBounds ([TrayProbe]::FindWindowEx($area, [IntPtr]::Zero, 'Button', [NullString]::Value))
    $notification = Get-TrayBounds $area
    $icon = [TrayProbe]::GetIconBounds($owner)
    if (!$icon.HasArea -or !$notification -or !$notification.Contains($icon.X, $icon.Y) -or
        ($chevron -and $chevron.Contains($icon.X, $icon.Y))) {
        $overflow = Get-TrayBounds ([TrayProbe]::FindWindow('NotifyIconOverflowWindow', [NullString]::Value))
        if (!$overflow) {
            if (!$chevron) { throw 'tray_overflow_button_missing' }
            Click-TrayPoint $chevron.X $chevron.Y $false
        }
        $icon = Wait-For 12 {
            $popupBounds = Get-TrayBounds ([TrayProbe]::FindWindow('NotifyIconOverflowWindow', [NullString]::Value))
            $bounds = [TrayProbe]::GetIconBounds($owner)
            if ($popupBounds -and $bounds.HasArea -and $popupBounds.Contains($bounds.X, $bounds.Y)) { $bounds }
        }
        if (!$icon) { throw 'tray_icon_bounds_not_found' }
        $result.checks.used_notification_overflow = $true
    }
    Click-TrayPoint $icon.X $icon.Y $true
    $item = Wait-For 12 {
        $popup = [TrayProbe]::FindOwnedWindow('#32768', $processId)
        $bounds = [TrayProbe]::GetMenuItemBounds($popup, $label)
        if ($bounds.HasArea) { return @{x = $bounds.X; y = $bounds.Y} }
        $control = Find-NativeControl $popup $label 'menu'
        if ($control -and $control.Current.IsEnabled) {
            $rectangle = $control.Current.BoundingRectangle
            if (!$rectangle.IsEmpty -and $rectangle.Width -ge 1 -and $rectangle.Height -ge 1) {
                return @{x = [int]($rectangle.X + $rectangle.Width / 2); y = [int]($rectangle.Y + $rectangle.Height / 2)}
            }
        }
    }
    if (!$item) { throw 'tray_item_not_found' }
    Click-TrayPoint $item.x $item.y $false
    $result.checks.tray_icon_owned_by_app = $true
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
    Invoke-TrayItem $app.Id '^(Open PennyPinch|PennyPinch.i a.)$'
    if (!(Wait-For 12 { !$app.HasExited -and [TrayProbe]::IsWindowVisible($window) })) { throw 'window_did_not_reopen' }
    $result.checks.reopened_from_native_tray = $true

    $result.phase = 'tray_pause'
    Invoke-TrayItem $app.Id '^(Pause|Duraklat)$'
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
    Invoke-TrayItem $app.Id '^(Resume|Devam et)$'
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
    Invoke-TrayItem $app.Id '^(Quit.*|Tamamen .+)$'
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
