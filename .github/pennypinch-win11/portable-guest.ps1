$ErrorActionPreference = 'Stop'
$serial = [IO.Ports.SerialPort]::new('COM1', 115200)
$serial.NewLine = [string][char]10
$serial.WriteTimeout = 5000
$serial.Open()
$timer = [Diagnostics.Stopwatch]::StartNew()
$result = [ordered]@{ schema_version = 1; status = 'fail'; phase = 'preflight'; checks = [ordered]@{} }

function Send-Receipt([string]$kind, $value) {
    $json = ConvertTo-Json -InputObject $value -Depth 10 -Compress
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($json))
    $serial.WriteLine("PPAUDIT $kind $encoded")
}

try {
    Send-Receipt guest_entry @{ started = $true; phase = 'specialize' }
    $system = Get-CimInstance Win32_OperatingSystem
    $build = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $architecture = @(Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Architecture -Unique)
    if ($system.BuildNumber -ne '26100' -or $system.ProductType -ne 1 -or $build.DisplayVersion -ne '24H2' -or
        $architecture.Count -ne 1 -or $architecture[0] -ne 9) { throw 'incorrect_windows_target' }
    $secureBoot = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State' -Name UEFISecureBootEnabled).UEFISecureBootEnabled
    $tpm = Get-CimInstance -Namespace 'root/cimv2/security/microsofttpm' -ClassName Win32_Tpm
    if ($secureBoot -ne 1 -or $tpm.SpecVersion -notmatch '^2\.0') { throw 'secure_boot_or_tpm_missing' }
    if (@(Get-NetAdapter -Physical).Count) { throw 'unexpected_guest_network_adapter' }
    if (@(Get-Command cargo,rustc,node,pnpm,py,cl,msbuild -ErrorAction SilentlyContinue).Count) { throw 'development_tools_present' }
    $result.windows = @{ build = 26100; display_version = '24H2'; architecture = 9; ubr = [int]$build.UBR;
        edition = $build.EditionID; secure_boot = $true; tpm2 = $true }
    $result.checks.no_dev_tools_in_path = $true
    $result.checks.network_device_absent = $true
    Send-Receipt before @{ preflight = @{ status = 'ready'; windows = $result.windows }; already_online = $false }

    $result.phase = 'portable_preparation'
    $stage = Join-Path $PSScriptRoot 'prepare.ps1'
    $prep = Start-Process -FilePath 'powershell.exe' -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File "{0}"' -f $stage) -WindowStyle Hidden -PassThru
    if (!$prep.WaitForExit(205000)) { throw 'portable_preparation_timeout' }
    $result.package = Get-Content -LiteralPath 'C:\PP-Portable-Preparation.json' -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($prep.ExitCode -ne 0 -or $result.package.status -ne 'pass') { throw 'portable_preparation_failed' }
    Send-Receipt portable $result.package

    $result.phase = 'packaged_controller'
    $bundle = 'C:\PP-Verification\runtime'
    $directory = 'C:\PP-Verification-State'
    if (Test-Path -LiteralPath $directory) { throw 'existing_test_state' }
    $null = New-Item -ItemType Directory -Path $directory
    $localService = [Security.Principal.SecurityIdentifier]::new('S-1-5-19')
    $localServiceName = $localService.Translate([Security.Principal.NTAccount]).Value
    $acl = Get-Acl -LiteralPath $directory
    $inheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($localService, 'Modify', $inheritance, 'None', 'Allow'))
    Set-Acl -LiteralPath $directory -AclObject $acl
    $stateFile = Join-Path $directory 'controller.json'
    $script = Join-Path $PSScriptRoot 'controller.py'
    $python = Join-Path $bundle 'payload\python.exe'
    $action = New-ScheduledTaskAction -Execute $python -Argument ('-I -B -u "{0}" "{1}" "{2}"' -f $script, $bundle, $stateFile)
    $principal = New-ScheduledTaskPrincipal -UserId $localServiceName -LogonType ServiceAccount -RunLevel Limited
    $taskName = 'PennyPinchPortableVerification'
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal | Out-Null
    Start-ScheduledTask -TaskName $taskName
    Send-Receipt controller_task @{ started = $true; identity = 'LocalService'; elevated = $false }
    $deadline = [DateTime]::UtcNow.AddSeconds(235)
    while (!(Test-Path -LiteralPath $stateFile) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Seconds 1 }
    if (!(Test-Path -LiteralPath $stateFile)) {
        $result.task_exit = (Get-ScheduledTaskInfo -TaskName $taskName).LastTaskResult
        throw 'controller_receipt_missing'
    }
    $result.controller = Get-Content -LiteralPath $stateFile -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($result.controller.status -ne 'pass') { throw 'packaged_controller_failed' }
    $result.status = 'pass'
    $result.phase = 'complete'
} catch {
    $result.error_type = $_.Exception.GetType().Name
    if ($_.Exception.Message -cmatch '^[a-z_]{1,70}$') { $result.error_code = $_.Exception.Message }
    $result.error_line = $_.InvocationInfo.ScriptLineNumber
} finally {
    $result.guest_test_seconds = [math]::Round($timer.Elapsed.TotalSeconds, 3)
    Send-Receipt complete $result
    $serial.Dispose()
}
if ($result.status -ne 'pass') { exit 1 }
