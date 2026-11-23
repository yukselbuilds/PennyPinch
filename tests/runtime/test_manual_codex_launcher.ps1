param(
    [switch]$EmitArgumentVector,
    [string]$ArgumentVectorJson
)

if ($EmitArgumentVector) {
    [Console]::Out.WriteLine($ArgumentVectorJson)
    exit 0
}

$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$modulePath = Join-Path $projectRoot 'scripts\manual-codex-core.psm1'
Import-Module -Name $modulePath -Force

$script:assertions = 0
function Assert-True([bool]$Condition, [string]$Name) {
    $script:assertions++
    if (-not $Condition) { throw "ASSERT:$Name" }
}

function Assert-Equal($Expected, $Actual, [string]$Name) {
    $script:assertions++
    if ($Expected -cne $Actual) { throw "ASSERT:$Name" }
}

$expectedExe = 'D:\Projeler\Headroom\work\caches\cargo-target\debug\headroom-desktop.exe'
$listener = [pscustomobject]@{
    LocalAddress = '127.0.0.1'
    LocalPort = 51778
    State = 'Listen'
    OwningProcess = 412
}
$ready = '{"ready":true,"version":"0.45.0"}'
$good = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson $ready
Assert-True $good.ok 'valid-owned-ready'
Assert-Equal '0.45.0' $good.engine_version 'ready-version'

$foreign = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 999 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson $ready
Assert-Equal 'listener_owner_mismatch' $foreign.code 'foreign-owner-rejected'

$wildcard = $listener.PSObject.Copy()
$wildcard.LocalAddress = '0.0.0.0'
$notLoopback = Get-ManualCodexPreflightResult -ListenerRows @($wildcard) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson $ready
Assert-Equal 'listener_not_loopback' $notLoopback.code 'wildcard-rejected'

$duplicates = Get-ManualCodexPreflightResult -ListenerRows @($listener, $listener) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson $ready
Assert-Equal 'listener_ambiguous' $duplicates.code 'ambiguous-listener-rejected'

$wrongImage = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 412 `
    -ProcessImagePath 'D:\Other\headroom-desktop.exe' -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson $ready
Assert-Equal 'process_image_mismatch' $wrongImage.code 'other-image-rejected'

$notReady = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 503 `
    -ReadyJson '{"ready":false,"version":"0.45.0"}'
Assert-Equal 'engine_not_ready' $notReady.code 'not-ready-rejected'
$missingReady = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson '{}'
Assert-Equal 'engine_not_ready' $missingReady.code 'missing-ready-and-version-rejected'
$missingVersion = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson '{"ready":true}'
Assert-Equal 'engine_not_ready' $missingVersion.code 'missing-version-rejected'
$wrongReadyType = Get-ManualCodexPreflightResult -ListenerRows @($listener) -ProcessId 412 `
    -ProcessImagePath $expectedExe -ExpectedImagePath $expectedExe -HttpStatusCode 200 -ReadyJson '{"ready":"true","version":"0.45.0"}'
Assert-Equal 'engine_not_ready' $wrongReadyType.code 'wrong-ready-type-rejected'

$cliRoot = 'C:\Users\test\AppData\Local\OpenAI\Codex\bin'
Assert-True (Test-ManualCodexInstallPath -Path "$cliRoot\0123456789abcdef\codex.exe" -InstallRoot $cliRoot) 'official-cli-layout'
Assert-True (-not (Test-ManualCodexInstallPath -Path 'C:\Tools\codex.exe' -InstallRoot $cliRoot)) 'external-cli-rejected'
Assert-True (Test-ManualCodexBinaryFingerprint -Length 336134960 -Sha256 'c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a') 'pinned-cli-fingerprint'
Assert-True (-not (Test-ManualCodexBinaryFingerprint -Length 336134959 -Sha256 'c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a')) 'wrong-cli-size-rejected'
Assert-True (-not (Test-ManualCodexBinaryFingerprint -Length 336134960 -Sha256 ('0' * 64))) 'wrong-cli-digest-rejected'

$unicodeModel = 'custom model/' + [string][char]0x6a21 + [string][char]0x578b
$unicodeSuffix = [string][char]0x6a21 + [string][char]0x578b
$workspace = 'D:\Projeler\Headroom\work\proofs\manual-codex\id\workspace with spaces-' + $unicodeSuffix
Assert-True (Test-ManualProjectChildPath -Path $workspace -ProjectRoot 'D:\Projeler\Headroom') 'workspace-contained'
Assert-True (-not (Test-ManualProjectChildPath -Path 'D:\Projeler\Headroom-old\escape' -ProjectRoot 'D:\Projeler\Headroom')) 'prefix-confusion-rejected'

$arguments = @(New-ManualCodexArguments -Workspace $workspace)
Assert-Equal '--no-daemon' $arguments[0] 'isolated-cli-process'
Assert-Equal '-s' $arguments[1] 'sandbox-flag'
Assert-Equal 'read-only' $arguments[2] 'read-only-sandbox'
Assert-Equal '-C' $arguments[3] 'cd-flag'
Assert-Equal $workspace $arguments[4] 'workspace-unicode-and-spaces-preserved'
Assert-Equal '-c' $arguments[5] 'config-flag'
Assert-Equal 'openai_base_url=http://127.0.0.1:51778/v1' $arguments[6] 'only-endpoint-override'
Assert-Equal 7 $arguments.Count 'no-model-auth-reasoning-agent-or-log-override'
Assert-True (-not (@($arguments) -contains 'log_dir=')) 'no-plaintext-tui-log-override'

$python = 'C:\Python314\python.exe'
if (-not [IO.File]::Exists($python)) { throw 'ASSERT:native-argv-python-unavailable' }
$pythonCode = 'import json,sys; print(json.dumps(sys.argv[1:], ensure_ascii=True))'
$nativeJson = & $python -c $pythonCode @arguments
if ($LASTEXITCODE -ne 0) { throw 'ASSERT:native-argv-child-failed' }
$nativeParsed = ConvertFrom-Json -InputObject ($nativeJson -join "`n") -ErrorAction Stop
if ($nativeParsed -is [System.Array]) { $nativeArgs = $nativeParsed }
else { $nativeArgs = @($nativeParsed) }
Assert-Equal $arguments.Count $nativeArgs.Count 'native-argv-count'
for ($index = 0; $index -lt $arguments.Count; $index++) {
    Assert-Equal $arguments[$index] $nativeArgs[$index] "native-argv-$index"
}

$createdRun = New-ManualRunWorkspace -ProjectRoot $projectRoot
try {
    Assert-True $createdRun.ok 'uuid-run-workspace-created'
    Assert-True ($createdRun.run_id -match '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$') 'run-id-is-uuid'
    Assert-True (Test-ManualProjectChildPath -Path $createdRun.root -ProjectRoot (Join-Path $projectRoot 'work\proofs\manual-codex')) 'run-root-contained'
    Assert-True (Test-ManualProjectChildPath -Path $createdRun.workspace -ProjectRoot $projectRoot) 'created-workspace-contained'
    Assert-True (Test-ManualPathChainNoReparse -Path $createdRun.workspace -RequireLeaf) 'created-workspace-no-reparse'
    Assert-True ([IO.Directory]::Exists($createdRun.workspace)) 'workspace-directory-exists'
    Assert-True (-not [IO.Directory]::Exists((Join-Path $createdRun.root 'logs'))) 'no-extra-log-capture-directory'
    $hashFixturePath = Join-Path $createdRun.workspace 'hash-fixture.bin'
    $hashFixtureBytes = [Text.Encoding]::ASCII.GetBytes('manual-codex-hash-fixture')
    if ([IO.File]::Exists($hashFixturePath)) { throw 'ASSERT:hash-fixture-already-exists' }
    [IO.File]::WriteAllBytes($hashFixturePath, $hashFixtureBytes)
    try {
        $hashResult = Get-ManualFileSha256 -Path $hashFixturePath -MaximumBytes 1024
        Assert-True $hashResult.ok 'dotnet-sha256-readable'
        Assert-Equal 25 $hashResult.length 'dotnet-sha256-length'
        Assert-Equal '65080312c5f59c21972149f5fcd4120eda859c986813db24df52430d67f1399c' $hashResult.sha256 'dotnet-sha256-digest'
        $oversizeHash = Get-ManualFileSha256 -Path $hashFixturePath -MaximumBytes 24
        Assert-Equal 'file_unavailable' $oversizeHash.code 'dotnet-sha256-size-bound'
    } finally {
        if ([IO.File]::Exists($hashFixturePath)) {
            if (-not (Test-ManualProjectChildPath -Path $hashFixturePath -ProjectRoot $createdRun.workspace) -or
                -not (Test-ManualPathChainNoReparse -Path $hashFixturePath -RequireLeaf)) {
                throw 'ASSERT:hash-fixture-cleanup-refused'
            }
            [IO.File]::Delete($hashFixturePath)
        }
    }
    $fixtureReport = [ordered]@{
        schema_version = 1
        run_id = $createdRun.run_id
        launcher_status = 'fixture'
        local_stats_status = 'not_requested'
        per_request_rows = @()
        aggregation = 'none'
    }
    $firstReport = Write-ManualLauncherReport -ProjectRoot $projectRoot -RunRoot $createdRun.root -ReportObject $fixtureReport
    Assert-True $firstReport.ok 'report-created-once'
    $reportBefore = [IO.File]::ReadAllText($firstReport.path)
    $secondReport = Write-ManualLauncherReport -ProjectRoot $projectRoot -RunRoot $createdRun.root -ReportObject $fixtureReport
    Assert-Equal 'report_exists' $secondReport.code 'report-create-new-does-not-overwrite'
    Assert-Equal $reportBefore ([IO.File]::ReadAllText($firstReport.path)) 'existing-report-remains-unchanged'
} finally {
    if ($createdRun.ok) {
        $manualRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot 'work\proofs\manual-codex'))
        $expectedRunRoot = [IO.Path]::GetFullPath((Join-Path $manualRoot $createdRun.run_id))
        $childDirectories = @([IO.Directory]::GetDirectories($expectedRunRoot))
        $childFiles = @([IO.Directory]::GetFiles($expectedRunRoot))
        $workspaceDirectories = @($childDirectories | Where-Object { [string]::Equals([IO.Path]::GetFullPath($_), [IO.Path]::GetFullPath($createdRun.workspace), [StringComparison]::OrdinalIgnoreCase) })
        $workspaceFiles = if ([IO.Directory]::Exists($createdRun.workspace)) { @([IO.Directory]::GetFiles($createdRun.workspace)) } else { @() }
        $workspaceChildren = if ([IO.Directory]::Exists($createdRun.workspace)) { @([IO.Directory]::GetDirectories($createdRun.workspace)) } else { @() }
        $expectedReport = [IO.Path]::GetFullPath((Join-Path $expectedRunRoot 'launcher-report.json'))
        $reportFiles = @($childFiles | Where-Object { [string]::Equals([IO.Path]::GetFullPath($_), $expectedReport, [StringComparison]::OrdinalIgnoreCase) })
        $reportChainSafe = if ($reportFiles.Count -eq 1) { Test-ManualPathChainNoReparse -Path $expectedReport -RequireLeaf } else { $false }
        $safeOwnedTree = [string]::Equals([IO.Path]::GetFullPath($createdRun.root), $expectedRunRoot, [StringComparison]::OrdinalIgnoreCase) -and
            (Test-ManualProjectChildPath -Path $expectedRunRoot -ProjectRoot $manualRoot) -and
            (Test-ManualPathChainNoReparse -Path $expectedRunRoot -RequireLeaf) -and
            (Test-ManualPathChainNoReparse -Path $createdRun.workspace -RequireLeaf) -and
            $childDirectories.Count -eq 1 -and $workspaceDirectories.Count -eq 1 -and $childFiles.Count -eq 1 -and $reportFiles.Count -eq 1 -and $reportChainSafe -and
            $workspaceFiles.Count -eq 0 -and $workspaceChildren.Count -eq 0
        if (-not $safeOwnedTree) { throw 'ASSERT:workspace-fixture-cleanup-refused' }
        Remove-Item -LiteralPath $expectedRunRoot -Recurse -Force -ErrorAction Stop
    }
}
Assert-True (-not [IO.Directory]::Exists($createdRun.root)) 'own-uuid-fixture-cleaned'

$stats = @{
    config = @{ api_key = 'SECRET_SENTINEL'; upstream_url = 'https://secret.invalid/key' }
    request_logs = @(
        @{ request_id='old-0000000000000001'; tags=@{ client='codex'; private='TAG_SENTINEL' }; provider='openai'; model='gpt-custom'; input_tokens_original=100; input_tokens_optimized=60; output_tokens=20; tokens_saved=40; savings_percent=40; request_messages='BODY_SENTINEL'; headers='HEADER_SENTINEL'; error='ERROR_SENTINEL' },
        @{ request_id='hr_1792746278_000003'; tags=@{ client='codex'; private='TAG_SENTINEL' }; provider='openai'; model=$unicodeModel; input_tokens_original=200; input_tokens_optimized=125; output_tokens=31; tokens_saved=75; savings_percent=37.5; request_messages='BODY_SENTINEL'; headers='HEADER_SENTINEL'; error='ERROR_SENTINEL' },
        @{ request_id='invalid/id-0000000000000004'; tags=@{ client='codex' }; model='invalid-row'; input_tokens_original=22; input_tokens_optimized=11; tokens_saved=11; savings_percent=50 },
        @{ request_id='other-0000000000000003'; tags=@{ client='other' }; provider='openai'; model='not-codex'; input_tokens_original=999; input_tokens_optimized=1; output_tokens=1; tokens_saved=998; savings_percent=99.8 }
    )
} | ConvertTo-Json -Depth 8 -Compress
$snapshot = Get-ManualCodexRequestIds -StatsJson $stats
Assert-True $snapshot.ok 'stats-baseline-parse'
Assert-True (@($snapshot.ids) -contains 'hr_1792746278_000003') 'actual-request-id-in-baseline'
Assert-True (-not (@($snapshot.ids) -contains 'invalid/id-0000000000000004')) 'invalid-request-id-excluded-from-baseline'
$delta = Get-ManualCodexStatsDelta -StatsJson $stats -BaselineRequestIds @('old-0000000000000001')
Assert-Equal 'available' $delta.status 'stats-available'
Assert-Equal 1 $delta.rows.Count 'only-new-codex-row'
Assert-Equal $unicodeModel $delta.rows[0].model 'arbitrary-model-unicode-metadata-preserved'
Assert-Equal 'hr_1792746278_000003' $delta.rows[0].request_id 'real-headroom-request-id-preserved'
Assert-Equal 75 $delta.rows[0].tokens_saved 'per-request-savings-only'
Assert-Equal 'unknown' $delta.rows[0].main_subagent_identity 'agent-attribution-unknown'
$safeJson = ConvertTo-Json -InputObject $delta -Depth 6 -Compress
foreach ($secret in @('SECRET_SENTINEL', 'secret.invalid', 'TAG_SENTINEL', 'BODY_SENTINEL', 'HEADER_SENTINEL', 'ERROR_SENTINEL', 'not-codex')) {
    Assert-True (-not $safeJson.Contains($secret)) "stats-redaction-$secret"
}
Assert-True (-not $safeJson.Contains('total_tokens_saved')) 'no-aggregate-savings'
$emptyBaseline = Get-ManualCodexStatsDelta -StatsJson $stats -BaselineRequestIds @()
Assert-Equal 2 $emptyBaseline.rows.Count 'empty-baseline-keeps-valid-codex-rows-only'
$noBaseline = Get-ManualCodexStatsDelta -StatsJson $stats -BaselineRequestIds $null
Assert-Equal 'baseline_unavailable' $noBaseline.code 'unavailable-baseline-suppresses-attribution'

$badStats = Get-ManualCodexRequestIds -StatsJson '{bad-json'
Assert-True (-not $badStats.ok) 'bad-stats-fail-closed'
Assert-Equal 'stats_invalid' $badStats.code 'fixed-stats-error'

foreach ($malformed in @('{"request_logs":{}}','{"request_logs":null}','{}','{"request_logs":[null]}')) {
    $invalidEnvelope = Get-ManualCodexRequestIds -StatsJson $malformed
    Assert-True (-not $invalidEnvelope.ok) 'malformed-request-logs-fixed-unavailable'
    Assert-Equal 'stats_invalid' $invalidEnvelope.code 'malformed-stats-fixed-code'
}

$partialStats = '{"request_logs":[{"request_id":"hr_1792746278_000004","tags":{"client":"codex"}}]}'
$partialDelta = Get-ManualCodexStatsDelta -StatsJson $partialStats -BaselineRequestIds @()
Assert-Equal 'partial' $partialDelta.status 'missing-model-and-metrics-not-success'
Assert-Equal 'metadata_incomplete' $partialDelta.code 'partial-fixed-code'
Assert-Equal $null $partialDelta.rows[0].model 'missing-model-remains-null'
Assert-Equal $null $partialDelta.rows[0].input_tokens_original 'missing-original-remains-null'
Assert-Equal $null $partialDelta.rows[0].input_tokens_optimized 'missing-optimized-remains-null'
Assert-Equal $null $partialDelta.rows[0].tokens_saved 'missing-savings-remains-null'
Assert-Equal $null $partialDelta.rows[0].savings_percent 'missing-percent-remains-null'
$missingTags = '{"request_logs":[{"request_id":"hr_1792746278_000005","model":"ignored"}]}'
$missingTagsDelta = Get-ManualCodexStatsDelta -StatsJson $missingTags -BaselineRequestIds @()
Assert-Equal 'unavailable' $missingTagsDelta.status 'missing-tags-do-not-attribute-client'
Assert-Equal 'metadata_incomplete' $missingTagsDelta.code 'missing-tags-fixed-incomplete-code'

$table = Format-ManualCodexSavingsRows -Rows @($delta.rows)
Assert-True ($table.Contains($unicodeModel) -and $table.Contains('200') -and $table.Contains('125') -and $table.Contains('75') -and $table.Contains('37.5%')) 'sanitized-per-request-table'

Write-Output "PASS manual-codex focused assertions=$script:assertions"
