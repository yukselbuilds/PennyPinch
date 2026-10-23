[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [switch]$IncludeLocalStats
)

$ErrorActionPreference = 'Stop'
Import-Module -Name (Join-Path $PSScriptRoot 'manual-codex-core.psm1') -Force
try { [void][type]::GetType('System.Net.Http.HttpClient, System.Net.Http', $true) }
catch { Add-Type -AssemblyName System.Net.Http }

function Stop-Safe([string]$Code) {
    $messages = @{
        invalid_options = 'CheckOnly and IncludeLocalStats cannot be combined.'
        project_root_mismatch = 'Run this launcher from the Headroom project checkout.'
        app_not_built = 'The expected local Headroom desktop build is unavailable.'
        app_path_unsafe = 'The expected Headroom executable path is unsafe or redirected.'
        listener_query_failed = 'Could not safely inspect the local Headroom listener.'
        listener_missing = 'Headroom is not listening on the expected local endpoint.'
        listener_ambiguous = 'The expected endpoint has more than one listener.'
        listener_not_loopback = 'The endpoint is not bound only to IPv4 loopback.'
        listener_owner_mismatch = 'The endpoint is not owned by the expected Headroom process.'
        process_unavailable = 'Could not verify the process that owns the endpoint.'
        process_image_mismatch = 'The endpoint owner is not the expected project Headroom executable.'
        readiness_unavailable = 'The local engine readiness check did not complete safely.'
        readiness_invalid = 'The local engine returned an invalid readiness response.'
        engine_not_ready = 'The owned Headroom engine is not ready at version 0.41.0.'
        codex_unavailable = 'The installed Codex CLI could not be located safely.'
        codex_path_unsafe = 'Codex is not at the expected user-local installation path.'
        codex_version_unverified = 'The installed Codex CLI version is not in the verified local allowlist.'
        codex_install_ambiguous = 'More than one verified Codex CLI installation was found.'
        run_root_unsafe = 'The ignored local run directory failed its project-path checks.'
        workspace_create_failed = 'Could not create a new isolated local workspace.'
        codex_start_failed = 'Codex could not be started; no raw error was retained.'
        report_write_failed = 'Could not write the bounded local launcher report.'
        report_exists = 'The report already exists; it was kept unchanged.'
    }
    if ($messages.ContainsKey($Code)) { Write-Error -Message $messages[$Code] -ErrorAction Continue }
    else { Write-Error -Message 'The launcher stopped on a fixed safety check.' -ErrorAction Continue }
    exit 2
}

function Get-HeadroomOwnerSnapshot([string]$ExpectedImagePath) {
    try { $rows = @(Get-NetTCPConnection -LocalPort 51778 -State Listen -ErrorAction Stop) }
    catch { return [pscustomobject]@{ ok=$false; code='listener_query_failed'; rows=@(); pid=0; image=$null; creation=$null } }
    if ($rows.Count -eq 0) { return [pscustomobject]@{ ok=$false; code='listener_missing'; rows=@(); pid=0; image=$null; creation=$null } }
    if ($rows.Count -ne 1) { return [pscustomobject]@{ ok=$false; code='listener_ambiguous'; rows=$rows; pid=0; image=$null; creation=$null } }
    if ([string]$rows[0].LocalAddress -cne '127.0.0.1') {
        return [pscustomobject]@{ ok=$false; code='listener_not_loopback'; rows=$rows; pid=0; image=$null; creation=$null }
    }
    $pidValue = 0
    try { $pidValue = [int]$rows[0].OwningProcess } catch { $pidValue = 0 }
    if ($pidValue -le 0) { return [pscustomobject]@{ ok=$false; code='listener_owner_mismatch'; rows=$rows; pid=0; image=$null; creation=$null } }
    try { $processes = @(Get-CimInstance -ClassName Win32_Process -Filter ("ProcessId = {0}" -f $pidValue) -ErrorAction Stop) }
    catch { return [pscustomobject]@{ ok=$false; code='process_unavailable'; rows=$rows; pid=$pidValue; image=$null; creation=$null } }
    if ($processes.Count -ne 1 -or [string]::IsNullOrWhiteSpace([string]$processes[0].ExecutablePath)) {
        return [pscustomobject]@{ ok=$false; code='process_unavailable'; rows=$rows; pid=$pidValue; image=$null; creation=$null }
    }
    $imagePath = [string]$processes[0].ExecutablePath
    $creation = [string]$processes[0].CreationDate
    if ([string]::IsNullOrWhiteSpace($creation)) {
        return [pscustomobject]@{ ok=$false; code='process_unavailable'; rows=$rows; pid=$pidValue; image=$imagePath; creation=$null }
    }
    if (-not (Test-ManualPathChainNoReparse -Path $ExpectedImagePath -RequireLeaf)) {
        return [pscustomobject]@{ ok=$false; code='app_path_unsafe'; rows=$rows; pid=$pidValue; image=$imagePath; creation=$creation }
    }
    try {
        $sameImage = [string]::Equals([IO.Path]::GetFullPath($imagePath), [IO.Path]::GetFullPath($ExpectedImagePath), [StringComparison]::OrdinalIgnoreCase)
    } catch { $sameImage = $false }
    if (-not $sameImage) {
        return [pscustomobject]@{ ok=$false; code='process_image_mismatch'; rows=$rows; pid=$pidValue; image=$imagePath; creation=$creation }
    }
    return [pscustomobject]@{ ok=$true; code='owned'; rows=$rows; pid=$pidValue; image=$imagePath; creation=$creation }
}

function Invoke-LocalJsonGet([string]$Path, [int]$MaximumBytes) {
    $handler = $null
    $client = $null
    $request = $null
    $response = $null
    $stream = $null
    $memory = $null
    $cancellation = $null
    try {
        if ($Path -cnotin @('/readyz', '/stats') -or $MaximumBytes -lt 1 -or $MaximumBytes -gt 1048576) {
            return [pscustomobject]@{ ok=$false; code='http_unavailable'; status=0; body=$null }
        }
        $handler = New-Object System.Net.Http.HttpClientHandler
        $handler.UseProxy = $false
        $handler.AllowAutoRedirect = $false
        $handler.UseDefaultCredentials = $false
        $client = New-Object System.Net.Http.HttpClient($handler)
        $client.Timeout = [Threading.Timeout]::InfiniteTimeSpan
        $cancellation = New-Object System.Threading.CancellationTokenSource([TimeSpan]::FromSeconds(3))
        $token = $cancellation.Token
        $uri = New-Object System.Uri(('http://127.0.0.1:51778' + $Path))
        $request = New-Object System.Net.Http.HttpRequestMessage([System.Net.Http.HttpMethod]::Get, $uri)
        $response = $client.SendAsync($request, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead, $token).GetAwaiter().GetResult()
        if ($response.Content.Headers.ContentLength -gt $MaximumBytes) {
            return [pscustomobject]@{ ok=$false; code='response_too_large'; status=0; body=$null }
        }
        $stream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $memory = New-Object IO.MemoryStream
        $buffer = New-Object byte[] 8192
        while (($read = $stream.ReadAsync($buffer, 0, $buffer.Length, $token).GetAwaiter().GetResult()) -gt 0) {
            if (($memory.Length + $read) -gt $MaximumBytes) {
                return [pscustomobject]@{ ok=$false; code='response_too_large'; status=0; body=$null }
            }
            $memory.Write($buffer, 0, $read)
        }
        $encoding = New-Object System.Text.UTF8Encoding($false, $true)
        $body = $encoding.GetString($memory.ToArray())
        return [pscustomobject]@{ ok=$true; code='ok'; status=[int]$response.StatusCode; body=$body }
    } catch {
        return [pscustomobject]@{ ok=$false; code='http_unavailable'; status=0; body=$null }
    } finally {
        if ($memory) { $memory.Dispose() }
        if ($stream) { $stream.Dispose() }
        if ($response) { $response.Dispose() }
        if ($request) { $request.Dispose() }
        if ($client) { $client.Dispose() }
        if ($handler) { $handler.Dispose() }
        if ($cancellation) { $cancellation.Dispose() }
    }
}

function Get-VerifiedCodexSelection([string]$InstallRoot) {
    $commands = @(Get-Command codex.exe -CommandType Application -ErrorAction SilentlyContinue)
    if ($commands.Count -gt 1) { return [pscustomobject]@{ ok=$false; code='codex_install_ambiguous'; path=$null; version=$null } }
    $command = if ($commands.Count -eq 1) { $commands[0] } else { $null }
    $candidatePaths = New-Object 'System.Collections.Generic.List[string]'
    if ($null -ne $command -and -not [string]::IsNullOrWhiteSpace([string]$command.Source)) {
        $candidate = [IO.Path]::GetFullPath([string]$command.Source)
        if (-not (Test-ManualCodexInstallPath -Path $candidate -InstallRoot $InstallRoot) -or
            -not (Test-ManualPathChainNoReparse -Path $candidate -RequireLeaf)) {
            return [pscustomobject]@{ ok=$false; code='codex_path_unsafe'; path=$null; version=$null }
        }
        $candidatePaths.Add($candidate)
    } else {
        if (-not [IO.Directory]::Exists($InstallRoot) -or -not (Test-ManualPathChainNoReparse -Path $InstallRoot -RequireLeaf)) {
            return [pscustomobject]@{ ok=$false; code='codex_unavailable'; path=$null; version=$null }
        }
        $directories = New-Object 'System.Collections.Generic.List[string]'
        try {
            foreach ($directory in [IO.Directory]::EnumerateDirectories($InstallRoot)) {
                $directories.Add($directory)
                if ($directories.Count -gt 16) { return [pscustomobject]@{ ok=$false; code='codex_install_ambiguous'; path=$null; version=$null } }
            }
        } catch { return [pscustomobject]@{ ok=$false; code='codex_unavailable'; path=$null; version=$null } }
        foreach ($directory in $directories) {
            $leaf = [IO.Path]::GetFileName($directory)
            if ($leaf -notmatch '^[0-9a-f]{16,64}$') { continue }
            $candidate = Join-Path $directory 'codex.exe'
            if (-not [IO.File]::Exists($candidate)) { continue }
            if (-not (Test-ManualCodexInstallPath -Path $candidate -InstallRoot $InstallRoot) -or
                -not (Test-ManualPathChainNoReparse -Path $candidate -RequireLeaf)) {
                return [pscustomobject]@{ ok=$false; code='codex_path_unsafe'; path=$null; version=$null }
            }
            $candidatePaths.Add([IO.Path]::GetFullPath($candidate))
        }
        if ($candidatePaths.Count -gt 1) { return [pscustomobject]@{ ok=$false; code='codex_install_ambiguous'; path=$null; version=$null } }
    }
    $verified = New-Object 'System.Collections.Generic.List[string]'
    foreach ($candidate in $candidatePaths) {
        try {
            $length = ([IO.FileInfo]$candidate).Length
            if ($length -ne 336134960) { continue }
            $digest = Get-ManualFileSha256 -Path $candidate -MaximumBytes 336134960
            if (-not $digest.ok -or $digest.length -ne 336134960) {
                return [pscustomobject]@{ ok=$false; code='codex_version_unverified'; path=$null; version=$null }
            }
            if (-not (Test-ManualPathChainNoReparse -Path $candidate -RequireLeaf)) {
                return [pscustomobject]@{ ok=$false; code='codex_path_unsafe'; path=$null; version=$null }
            }
            if (Test-ManualCodexBinaryFingerprint -Length $digest.length -Sha256 $digest.sha256) { $verified.Add($candidate) }
        } catch { return [pscustomobject]@{ ok=$false; code='codex_version_unverified'; path=$null; version=$null } }
    }
    if ($verified.Count -eq 0) { return [pscustomobject]@{ ok=$false; code='codex_version_unverified'; path=$null; version=$null } }
    if ($verified.Count -ne 1) { return [pscustomobject]@{ ok=$false; code='codex_install_ambiguous'; path=$null; version=$null } }
    return [pscustomobject]@{ ok=$true; code='ok'; path=$verified[0]; version='0.163.0-alpha.11.2' }
}

if ($CheckOnly -and $IncludeLocalStats) { Stop-Safe 'invalid_options' }
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not [string]::Equals($projectRoot.TrimEnd('\'), 'D:\Projeler\Headroom', [StringComparison]::OrdinalIgnoreCase)) { Stop-Safe 'project_root_mismatch' }
$expectedImage = Join-Path $projectRoot 'work\caches\cargo-target\debug\headroom-desktop.exe'
if (-not (Test-Path -LiteralPath $expectedImage -PathType Leaf)) { Stop-Safe 'app_not_built' }
if (-not (Test-ManualPathChainNoReparse -Path $expectedImage -RequireLeaf)) { Stop-Safe 'app_path_unsafe' }

$localAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
if ([string]::IsNullOrWhiteSpace($localAppData)) { Stop-Safe 'codex_unavailable' }
$codexInstallRoot = Join-Path $localAppData 'OpenAI\Codex\bin'
$codexSelection = Get-VerifiedCodexSelection -InstallRoot $codexInstallRoot
if (-not $codexSelection.ok) { Stop-Safe $codexSelection.code }
$codexPath = $codexSelection.path

$before = Get-HeadroomOwnerSnapshot -ExpectedImagePath $expectedImage
if (-not $before.ok) { Stop-Safe $before.code }
$readyReply = Invoke-LocalJsonGet -Path '/readyz' -MaximumBytes 16384
if (-not $readyReply.ok) { Stop-Safe 'readiness_unavailable' }
$after = Get-HeadroomOwnerSnapshot -ExpectedImagePath $expectedImage
if (-not $after.ok -or $after.pid -ne $before.pid -or $after.creation -cne $before.creation) {
    Stop-Safe 'listener_owner_mismatch'
}
$preflight = Get-ManualCodexPreflightResult -ListenerRows $after.rows -ProcessId $after.pid `
    -ProcessImagePath $after.image -ExpectedImagePath $expectedImage -HttpStatusCode $readyReply.status -ReadyJson $readyReply.body
if (-not $preflight.ok) { Stop-Safe $preflight.code }

if ($CheckOnly) {
    Write-Output 'Check passed: the expected Headroom desktop process owns 127.0.0.1:51778 and its 0.41.0 engine is ready.'
    Write-Output ('Codex CLI selection: ' + $codexSelection.version + ' (' + $codexPath + ')')
    Write-Output 'Codex was not launched; login files and client configuration were not inspected.'
    exit 0
}

$run = New-ManualRunWorkspace -ProjectRoot $projectRoot
if (-not $run.ok) { Stop-Safe $run.code }
$baselineIds = $null
$statsStatus = 'not_requested'
if ($IncludeLocalStats) {
    $baselineOwner = Get-HeadroomOwnerSnapshot -ExpectedImagePath $expectedImage
    if ($baselineOwner.ok -and $baselineOwner.pid -eq $before.pid -and $baselineOwner.creation -ceq $before.creation) {
        $baselineReply = Invoke-LocalJsonGet -Path '/stats' -MaximumBytes 1048576
        if ($baselineReply.ok -and $baselineReply.status -eq 200) {
            $baseline = Get-ManualCodexRequestIds -StatsJson $baselineReply.body
            if ($baseline.ok) { $baselineIds = [string[]]@($baseline.ids); $statsStatus = 'pending' }
            else { $statsStatus = 'unavailable' }
        } else { $statsStatus = 'unavailable' }
    } else { $statsStatus = 'unavailable' }
}

Write-Output ('Starting Codex CLI ' + $codexSelection.version + ' with its own process, read-only sandbox, isolated workspace, and one endpoint override.')
Write-Output ('Codex CLI: ' + $codexPath)
Write-Output ('Workspace: ' + $run.workspace)
$cliArguments = @(New-ManualCodexArguments -Workspace $run.workspace)
$cliExitCode = $null
$launcherStatus = 'started'
try {
    & $codexPath @cliArguments
    $cliExitCode = [int]$LASTEXITCODE
} catch {
    $launcherStatus = 'codex_start_failed'
}

$statsResult = [pscustomobject]@{ status=$statsStatus; code='not_requested'; rows=@(); aggregation='none' }
if ($IncludeLocalStats -and $null -ne $baselineIds) {
    $finalOwner = Get-HeadroomOwnerSnapshot -ExpectedImagePath $expectedImage
    if ($finalOwner.ok -and $finalOwner.pid -eq $before.pid -and $finalOwner.creation -ceq $before.creation) {
        $finalReply = Invoke-LocalJsonGet -Path '/stats' -MaximumBytes 1048576
        if ($finalReply.ok -and $finalReply.status -eq 200) {
            $statsResult = Get-ManualCodexStatsDelta -StatsJson $finalReply.body -BaselineRequestIds $baselineIds
        } else { $statsResult = [pscustomobject]@{ status='unavailable'; code='stats_unavailable'; rows=@(); aggregation='none' } }
    } else { $statsResult = [pscustomobject]@{ status='unavailable'; code='headroom_owner_changed'; rows=@(); aggregation='none' } }
}
if ($launcherStatus -eq 'codex_start_failed') { $cliExitCode = $null }
$report = [ordered]@{
    schema_version = 1
    run_id = $run.run_id
    recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    launcher_status = $launcherStatus
    cli_exit_code = $cliExitCode
    local_stats_status = $statsResult.status
    per_request_rows = @($statsResult.rows)
    aggregation = 'none'
    main_subagent_identity = 'unknown'
    user_turn_completion = 'unknown'
}
$reportWrite = Write-ManualLauncherReport -ProjectRoot $projectRoot -RunRoot $run.root -ReportObject $report
if (-not $reportWrite.ok) { Stop-Safe $reportWrite.code }
$reportPath = $reportWrite.path

if ($launcherStatus -eq 'codex_start_failed') { Stop-Safe 'codex_start_failed' }
Write-Output ('Launcher report: ' + $reportPath)
Write-Output ('Codex process exit code: ' + $cliExitCode)
if ($IncludeLocalStats) {
    Write-Output 'Per-request Codex rows (not totals; warmup/user turn and agent identity remain unknown):'
    Write-Output (Format-ManualCodexSavingsRows -Rows @($statsResult.rows))
}
exit 0
