Set-StrictMode -Version 2.0

function Get-ManualCodexPreflightResult {
    param(
        [object[]]$ListenerRows,
        [int]$ProcessId,
        [string]$ProcessImagePath,
        [string]$ExpectedImagePath,
        [int]$HttpStatusCode,
        [string]$ReadyJson
    )

    $listeners = @($ListenerRows | Where-Object {
        [int]$_.LocalPort -eq 51778 -and [string]$_.State -eq 'Listen'
    })
    if ($listeners.Count -eq 0) { return [pscustomobject]@{ ok=$false; code='listener_missing'; engine_version=$null } }
    if ($listeners.Count -ne 1) { return [pscustomobject]@{ ok=$false; code='listener_ambiguous'; engine_version=$null } }
    if ([string]$listeners[0].LocalAddress -cne '127.0.0.1') {
        return [pscustomobject]@{ ok=$false; code='listener_not_loopback'; engine_version=$null }
    }
    if ([int]$listeners[0].OwningProcess -ne $ProcessId) {
        return [pscustomobject]@{ ok=$false; code='listener_owner_mismatch'; engine_version=$null }
    }
    try {
        $image = [IO.Path]::GetFullPath($ProcessImagePath)
        $expected = [IO.Path]::GetFullPath($ExpectedImagePath)
    } catch {
        return [pscustomobject]@{ ok=$false; code='process_image_mismatch'; engine_version=$null }
    }
    if (-not [string]::Equals($image, $expected, [StringComparison]::OrdinalIgnoreCase)) {
        return [pscustomobject]@{ ok=$false; code='process_image_mismatch'; engine_version=$null }
    }
    if ($HttpStatusCode -ne 200) { return [pscustomobject]@{ ok=$false; code='engine_not_ready'; engine_version=$null } }
    try { $ready = ConvertFrom-Json -InputObject $ReadyJson -ErrorAction Stop } catch {
        return [pscustomobject]@{ ok=$false; code='readiness_invalid'; engine_version=$null }
    }
    $readyField = Get-ManualObjectProperty -InputObject $ready -Name 'ready'
    $versionField = Get-ManualObjectProperty -InputObject $ready -Name 'version'
    if (-not $readyField.exists -or -not $versionField.exists -or
        ($readyField.value -isnot [bool]) -or -not $readyField.value -or
        ($versionField.value -isnot [string]) -or $versionField.value -cne '0.45.0') {
        return [pscustomobject]@{ ok=$false; code='engine_not_ready'; engine_version=$null }
    }
    return [pscustomobject]@{ ok=$true; code='ready'; engine_version='0.45.0' }
}

function Test-ManualCodexInstallPath {
    param([string]$Path, [string]$InstallRoot)
    try {
        $fullPath = [IO.Path]::GetFullPath($Path)
        $fullRoot = [IO.Path]::GetFullPath($InstallRoot).TrimEnd('\') + '\'
    } catch { return $false }
    if (-not $fullPath.StartsWith($fullRoot, [StringComparison]::OrdinalIgnoreCase)) { return $false }
    $relative = $fullPath.Substring($fullRoot.Length)
    return ($relative -match '^[0-9a-f]{16,64}\\codex\.exe$')
}

function Test-ManualCodexBinaryFingerprint {
    param([long]$Length, [string]$Sha256)
    return ($Length -eq 336134960 -and $Sha256 -cmatch '^c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a$')
}

function Get-ManualFileSha256 {
    param([string]$Path, [long]$MaximumBytes)
    $stream = $null
    $sha = $null
    try {
        if ([string]::IsNullOrWhiteSpace($Path) -or $MaximumBytes -lt 1) {
            return [pscustomobject]@{ ok=$false; code='file_unavailable'; length=$null; sha256=$null }
        }
        $length = ([IO.FileInfo]$Path).Length
        if ($length -lt 0 -or $length -gt $MaximumBytes) {
            return [pscustomobject]@{ ok=$false; code='file_unavailable'; length=$null; sha256=$null }
        }
        $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ($stream.Length -ne $length) {
            return [pscustomobject]@{ ok=$false; code='file_unavailable'; length=$null; sha256=$null }
        }
        $sha = [Security.Cryptography.SHA256]::Create()
        $digestBytes = $sha.ComputeHash($stream)
        $digest = [BitConverter]::ToString($digestBytes).Replace('-', '').ToLowerInvariant()
        return [pscustomobject]@{ ok=$true; code='ok'; length=$length; sha256=$digest }
    } catch {
        return [pscustomobject]@{ ok=$false; code='file_unavailable'; length=$null; sha256=$null }
    } finally {
        if ($sha) { $sha.Dispose() }
        if ($stream) { $stream.Dispose() }
    }
}

function Test-ManualProjectChildPath {
    param([string]$Path, [string]$ProjectRoot)
    try {
        $fullPath = [IO.Path]::GetFullPath($Path)
        $fullRoot = [IO.Path]::GetFullPath($ProjectRoot).TrimEnd('\') + '\'
    } catch { return $false }
    return $fullPath.StartsWith($fullRoot, [StringComparison]::OrdinalIgnoreCase)
}

function Test-ManualPathChainNoReparse {
    param([string]$Path, [switch]$RequireLeaf)
    try {
        $full = [IO.Path]::GetFullPath($Path)
        $volume = [IO.Path]::GetPathRoot($full)
        if ([string]::IsNullOrEmpty($volume)) { return $false }
        $segments = $full.Substring($volume.Length) -split '[\\/]'
        $cursor = $volume
        foreach ($segment in $segments) {
            if ([string]::IsNullOrEmpty($segment)) { continue }
            $cursor = Join-Path $cursor $segment
            try { $attributes = [IO.File]::GetAttributes($cursor) }
            catch {
                if ($RequireLeaf) { return $false }
                return $true
            }
            if (($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $false }
        }
        return $true
    } catch { return $false }
}

function New-ManualRunWorkspace {
    param([string]$ProjectRoot)
    $runRoot = $null
    try {
        $root = [IO.Path]::GetFullPath($ProjectRoot).TrimEnd('\\')
        $proofsRoot = [IO.Path]::GetFullPath((Join-Path $root 'work\proofs'))
        $manualRoot = [IO.Path]::GetFullPath((Join-Path $proofsRoot 'manual-codex'))
        if (-not (Test-ManualProjectChildPath -Path $manualRoot -ProjectRoot $root) -or
            -not (Test-ManualPathChainNoReparse -Path $proofsRoot -RequireLeaf)) {
            return [pscustomobject]@{ ok=$false; code='run_root_unsafe'; run_id=$null; root=$null; workspace=$null }
        }
        if ([IO.Directory]::Exists($manualRoot)) {
            if (-not (Test-ManualPathChainNoReparse -Path $manualRoot -RequireLeaf)) {
                return [pscustomobject]@{ ok=$false; code='run_root_unsafe'; run_id=$null; root=$null; workspace=$null }
            }
        } else {
            [void][IO.Directory]::CreateDirectory($manualRoot)
            if (-not (Test-ManualPathChainNoReparse -Path $manualRoot -RequireLeaf)) {
                return [pscustomobject]@{ ok=$false; code='run_root_unsafe'; run_id=$null; root=$null; workspace=$null }
            }
        }

        $runId = [Guid]::NewGuid().ToString('D')
        $runRoot = [IO.Path]::GetFullPath((Join-Path $manualRoot $runId))
        $workspace = [IO.Path]::GetFullPath((Join-Path $runRoot 'workspace'))
        if (-not (Test-ManualProjectChildPath -Path $runRoot -ProjectRoot $manualRoot) -or
            [IO.Directory]::Exists($runRoot) -or [IO.File]::Exists($runRoot)) {
            return [pscustomobject]@{ ok=$false; code='run_root_unsafe'; run_id=$null; root=$null; workspace=$null }
        }
        [void][IO.Directory]::CreateDirectory($runRoot)
        if (-not (Test-ManualPathChainNoReparse -Path $runRoot -RequireLeaf)) {
            return [pscustomobject]@{ ok=$false; code='run_root_unsafe'; run_id=$runId; root=$runRoot; workspace=$null }
        }
        [void][IO.Directory]::CreateDirectory($workspace)
        if (-not (Test-ManualProjectChildPath -Path $workspace -ProjectRoot $root) -or
            -not (Test-ManualPathChainNoReparse -Path $workspace -RequireLeaf)) {
            return [pscustomobject]@{ ok=$false; code='run_root_unsafe'; run_id=$runId; root=$runRoot; workspace=$null }
        }
        return [pscustomobject]@{ ok=$true; code='ok'; run_id=$runId; root=$runRoot; workspace=$workspace }
    } catch {
        return [pscustomobject]@{ ok=$false; code='workspace_create_failed'; run_id=$null; root=$runRoot; workspace=$null }
    }
}

function New-ManualCodexArguments {
    param([string]$Workspace)
    return @('--no-daemon', '-s', 'read-only', '-C', $Workspace, '-c', 'openai_base_url=http://127.0.0.1:51778/v1')
}

function Write-ManualLauncherReport {
    param([string]$ProjectRoot, [string]$RunRoot, $ReportObject)
    $stream = $null
    try {
        $project = [IO.Path]::GetFullPath($ProjectRoot).TrimEnd('\\')
        $manualRoot = [IO.Path]::GetFullPath((Join-Path $project 'work\proofs\manual-codex'))
        $run = [IO.Path]::GetFullPath($RunRoot)
        $runParent = [IO.Path]::GetFullPath([IO.Directory]::GetParent($run).FullName)
        $runId = [IO.Path]::GetFileName($run)
        if (-not [guid]::TryParse($runId, [ref]([guid]::Empty)) -or
            -not [string]::Equals($runParent, $manualRoot, [StringComparison]::OrdinalIgnoreCase) -or
            -not (Test-ManualPathChainNoReparse -Path $run -RequireLeaf)) {
            return [pscustomobject]@{ ok=$false; code='report_write_failed'; path=$null }
        }
        $reportPath = [IO.Path]::GetFullPath((Join-Path $run 'launcher-report.json'))
        if (-not (Test-ManualProjectChildPath -Path $reportPath -ProjectRoot $manualRoot)) {
            return [pscustomobject]@{ ok=$false; code='report_write_failed'; path=$null }
        }
        $json = ConvertTo-Json -InputObject $ReportObject -Depth 6
        $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($json)
        if ($bytes.Length -gt 16384) { return [pscustomobject]@{ ok=$false; code='report_write_failed'; path=$null } }
        $stream = [IO.File]::Open($reportPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
        return [pscustomobject]@{ ok=$true; code='ok'; path=$reportPath }
    } catch [IO.IOException] {
        if ($RunRoot -and [IO.File]::Exists((Join-Path $RunRoot 'launcher-report.json'))) {
            return [pscustomobject]@{ ok=$false; code='report_exists'; path=$null }
        }
        return [pscustomobject]@{ ok=$false; code='report_write_failed'; path=$null }
    } catch {
        return [pscustomobject]@{ ok=$false; code='report_write_failed'; path=$null }
    } finally {
        if ($stream) { $stream.Dispose() }
    }
}

function Get-ManualObjectProperty {
    param($InputObject, [string]$Name)
    if ($null -eq $InputObject -or $InputObject -is [string] -or $InputObject -is [System.Array]) {
        return [pscustomobject]@{ exists=$false; value=$null }
    }
    try {
        $property = $InputObject.PSObject.Properties[$Name]
        if ($null -eq $property) { return [pscustomobject]@{ exists=$false; value=$null } }
        return [pscustomobject]@{ exists=$true; value=$property.Value }
    } catch { return [pscustomobject]@{ exists=$false; value=$null } }
}

function Read-ManualStatsEnvelope {
    param([string]$StatsJson)
    if ([string]::IsNullOrEmpty($StatsJson) -or $StatsJson.Length -gt 1048576) {
        return [pscustomobject]@{ ok=$false; code='stats_invalid'; entries=@() }
    }
    try { $payload = ConvertFrom-Json -InputObject $StatsJson -ErrorAction Stop } catch {
        return [pscustomobject]@{ ok=$false; code='stats_invalid'; entries=@() }
    }
    $logsField = Get-ManualObjectProperty -InputObject $payload -Name 'request_logs'
    if (-not $logsField.exists -or $logsField.value -isnot [System.Array] -or $logsField.value.Count -gt 10) {
        return [pscustomobject]@{ ok=$false; code='stats_invalid'; entries=@() }
    }
    foreach ($entry in $logsField.value) {
        if ($null -eq $entry -or $entry -is [string] -or $entry -is [System.Array]) {
            return [pscustomobject]@{ ok=$false; code='stats_invalid'; entries=@() }
        }
    }
    return [pscustomobject]@{ ok=$true; code='ok'; entries=@($logsField.value) }
}

function Get-ManualCodexRequestIds {
    param([string]$StatsJson)
    $parsed = Read-ManualStatsEnvelope -StatsJson $StatsJson
    if (-not $parsed.ok) { return [pscustomobject]@{ ok=$false; code=$parsed.code; ids=@() } }
    $ids = New-Object 'System.Collections.Generic.List[string]'
    foreach ($entry in $parsed.entries) {
        $idField = Get-ManualObjectProperty -InputObject $entry -Name 'request_id'
        if (-not $idField.exists -or $idField.value -isnot [string]) { continue }
        $id = [string]$idField.value
        if ($id -match '^[A-Za-z0-9_-]{8,128}$' -and -not $ids.Contains($id)) { $ids.Add($id) }
    }
    return [pscustomobject]@{ ok=$true; code='ok'; ids=@($ids.ToArray()) }
}

function ConvertTo-ManualSafeInteger {
    param($Value, [bool]$AllowNegative = $false)
    if ($null -eq $Value -or $Value -is [bool]) { return $null }
    try {
        if ($Value -is [string]) {
            if ($Value -notmatch '^-?[0-9]{1,10}$') { return $null }
            $number = [long]::Parse($Value, [Globalization.CultureInfo]::InvariantCulture)
        } elseif ($Value -is [int] -or $Value -is [long] -or $Value -is [short]) {
            $number = [long]$Value
        } else {
            $floating = [double]$Value
            if ([double]::IsNaN($floating) -or [double]::IsInfinity($floating) -or [Math]::Truncate($floating) -ne $floating) { return $null }
            $number = [long]$floating
        }
    } catch { return $null }
    if ($AllowNegative) {
        if ($number -lt -1000000000 -or $number -gt 1000000000) { return $null }
    } elseif ($number -lt 0 -or $number -gt 1000000000) { return $null }
    return $number
}

function Get-ManualCodexStatsDelta {
    param([string]$StatsJson, [string[]]$BaselineRequestIds)
    $parsed = Get-ManualCodexRequestIds -StatsJson $StatsJson
    if (-not $parsed.ok) { return [pscustomobject]@{ status='unavailable'; code=$parsed.code; rows=@() } }
    if ($null -eq $BaselineRequestIds) { return [pscustomobject]@{ status='unavailable'; code='baseline_unavailable'; rows=@() } }
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($id in $BaselineRequestIds) { [void]$seen.Add([string]$id) }
    $envelope = Read-ManualStatsEnvelope -StatsJson $StatsJson
    if (-not $envelope.ok) { return [pscustomobject]@{ status='unavailable'; code=$envelope.code; rows=@() } }
    $rows = New-Object 'System.Collections.Generic.List[object]'
    $incomplete = $false
    foreach ($entry in $envelope.entries) {
        $idField = Get-ManualObjectProperty -InputObject $entry -Name 'request_id'
        if (-not $idField.exists -or $idField.value -isnot [string]) { continue }
        $id = [string]$idField.value
        if ($id -notmatch '^[A-Za-z0-9_-]{8,128}$') { continue }
        if ($seen.Contains($id)) { continue }
        $tagsField = Get-ManualObjectProperty -InputObject $entry -Name 'tags'
        if (-not $tagsField.exists) { $incomplete = $true; continue }
        $clientField = Get-ManualObjectProperty -InputObject $tagsField.value -Name 'client'
        if (-not $clientField.exists -or $clientField.value -isnot [string]) { $incomplete = $true; continue }
        if ($clientField.value -cne 'codex') { continue }
        $modelField = Get-ManualObjectProperty -InputObject $entry -Name 'model'
        $model = $null
        if ($modelField.exists -and $modelField.value -is [string] -and
            $modelField.value.Length -ge 1 -and $modelField.value.Length -le 160 -and
            $modelField.value -notmatch '[\p{Cc}\p{Cs}]') { $model = [string]$modelField.value }
        $savingsPercent = $null
        try {
            $savingsField = Get-ManualObjectProperty -InputObject $entry -Name 'savings_percent'
            if ($savingsField.exists -and $null -ne $savingsField.value) {
                $candidate = [double]$savingsField.value
                if (-not [double]::IsNaN($candidate) -and -not [double]::IsInfinity($candidate) -and $candidate -ge -10000 -and $candidate -le 10000) { $savingsPercent = $candidate }
            }
        } catch { $savingsPercent = $null }
        $originalField = Get-ManualObjectProperty -InputObject $entry -Name 'input_tokens_original'
        $optimizedField = Get-ManualObjectProperty -InputObject $entry -Name 'input_tokens_optimized'
        $outputField = Get-ManualObjectProperty -InputObject $entry -Name 'output_tokens'
        $savedField = Get-ManualObjectProperty -InputObject $entry -Name 'tokens_saved'
        $original = if ($originalField.exists) { ConvertTo-ManualSafeInteger $originalField.value } else { $null }
        $optimized = if ($optimizedField.exists) { ConvertTo-ManualSafeInteger $optimizedField.value } else { $null }
        $output = if ($outputField.exists) { ConvertTo-ManualSafeInteger $outputField.value } else { $null }
        $saved = if ($savedField.exists) { ConvertTo-ManualSafeInteger $savedField.value $true } else { $null }
        if ($null -eq $model -or $null -eq $original -or $null -eq $optimized -or $null -eq $saved -or $null -eq $savingsPercent) { $incomplete = $true }
        $rows.Add([pscustomobject]@{
            request_id = $id
            model = $model
            input_tokens_original = $original
            input_tokens_optimized = $optimized
            output_tokens = $output
            tokens_saved = $saved
            savings_percent = $savingsPercent
            main_subagent_identity = 'unknown'
        })
        if ($rows.Count -ge 10) { break }
    }
    $status = if ($incomplete) { 'partial' } else { 'available' }
    if ($rows.Count -eq 0 -and $incomplete) { $status = 'unavailable' }
    elseif ($rows.Count -eq 0) { $status = 'no_new_codex_requests' }
    $code = if ($incomplete) { 'metadata_incomplete' } else { 'ok' }
    return [pscustomobject]@{ status=$status; code=$code; rows=@($rows.ToArray()); aggregation='none' }
}

function Format-ManualCodexSavingsRows {
    param([object[]]$Rows)
    $safeRows = @($Rows | ForEach-Object {
        [pscustomobject]@{
            Request = [string]$_.request_id
            Model = if ($null -eq $_.model) { '—' } else { [string]$_.model }
            Original = if ($null -eq $_.input_tokens_original) { '—' } else { [string]$_.input_tokens_original }
            Optimized = if ($null -eq $_.input_tokens_optimized) { '—' } else { [string]$_.input_tokens_optimized }
            Saved = if ($null -eq $_.tokens_saved) { '—' } else { [string]$_.tokens_saved }
            Percent = if ($null -eq $_.savings_percent) { '—' } else { ([double]$_.savings_percent).ToString('0.##', [Globalization.CultureInfo]::InvariantCulture) + '%' }
        }
    })
    if ($safeRows.Count -eq 0) { return 'No new Codex request rows were available.' }
    return ($safeRows | Format-Table -AutoSize | Out-String -Width 160).TrimEnd()
}

Export-ModuleMember -Function Get-ManualCodexPreflightResult,Test-ManualCodexInstallPath,Test-ManualCodexBinaryFingerprint,Get-ManualFileSha256,Test-ManualProjectChildPath,Test-ManualPathChainNoReparse,New-ManualRunWorkspace,New-ManualCodexArguments,Write-ManualLauncherReport,Get-ManualObjectProperty,Read-ManualStatsEnvelope,Get-ManualCodexRequestIds,Get-ManualCodexStatsDelta,Format-ManualCodexSavingsRows
