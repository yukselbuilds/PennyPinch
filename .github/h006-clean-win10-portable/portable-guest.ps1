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

try {
    $system = Get-CimInstance Win32_OperatingSystem
    $build = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $architecture = @(Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Architecture -Unique)
    if ($system.BuildNumber -ne '19045' -or $system.ProductType -ne 1 -or
        $build.DisplayVersion -ne '22H2' -or !$architecture.Count -or $architecture[0] -ne 9) {
        throw 'incorrect_windows_target'
    }
    if (@(Get-NetIPConfiguration | Where-Object IPv4DefaultGateway).Count) { throw 'guest_network_not_disconnected' }
    if (@(Get-Command cargo,rustc,node,pnpm,py,cl,msbuild -ErrorAction SilentlyContinue).Count) {
        throw 'development_tools_present'
    }
    $result.windows = @{ build = 19045; edition = $system.Caption; display_version = '22H2'; architecture = 9 }
    $result.checks.no_dev_tools_in_path = $true
    $result.checks.network_disconnected = $true
    Send-Receipt before @{preflight = @{status = 'ready'; windows = $result.windows}; already_online = $false}

    $result.phase = 'portable_bundle'
    $stage = Join-Path $PSScriptRoot 'prepare.ps1'
    $prep = Start-Process -FilePath 'powershell.exe' -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File $stage" -WindowStyle Hidden -PassThru
    if (!$prep.WaitForExit(170000)) { throw 'portable_preparation_timeout' }
    $result.package = Get-Content -LiteralPath 'C:\PP-Portable-Preparation.json' -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($prep.ExitCode -ne 0 -or $result.package.status -ne 'pass') { throw 'portable_preparation_failed' }
    Send-Receipt portable $result.package

    $result.phase = 'packaged_controller'
    $bundle = 'C:\PP-Verification\runtime'
    $python = Join-Path $bundle 'payload\python.exe'
    $script = Join-Path $PSScriptRoot 'controller.py'
    $stateFile = Join-Path $env:LOCALAPPDATA 'pp-clean-controller.json'
    if (Test-Path -LiteralPath $stateFile) { throw 'existing_controller_receipt' }
    $action = New-ScheduledTaskAction -Execute $python -Argument ('-I -B -u "{0}" "{1}" "{2}"' -f $script, $bundle, $stateFile)
    $principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
    $taskName = 'PennyPinchCleanControllerVerification'
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal | Out-Null
    Start-ScheduledTask -TaskName $taskName
    $deadline = [DateTime]::UtcNow.AddSeconds(210)
    while (!(Test-Path -LiteralPath $stateFile) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Seconds 1
    }
    if (!(Test-Path -LiteralPath $stateFile)) {
        $result.task_exit = (Get-ScheduledTaskInfo -TaskName $taskName).LastTaskResult
        throw 'controller_receipt_missing'
    }
    $result.controller = Get-Content -LiteralPath $stateFile -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($result.controller.status -ne 'pass') { throw 'packaged_controller_failed' }
    $result.status = 'pass'
    $result.phase = 'complete'
} catch {
    $result.status = 'fail'
    $result.error_type = $_.Exception.GetType().Name
    if ($_.Exception.Message -cmatch '^[a-z_]{1,70}$') { $result.error_code = $_.Exception.Message }
    $result.error_line = $_.InvocationInfo.ScriptLineNumber
} finally {
    $result.guest_seconds = [math]::Round($timer.Elapsed.TotalSeconds, 3)
    Send-Receipt complete $result
    $serial.Dispose()
}
