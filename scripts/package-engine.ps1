[CmdletBinding()]
param(
    [switch]$ReuseExisting
)

$ErrorActionPreference = 'Stop'

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'work\proofs\packaging\artifacts'))
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $artifactRoot 'headroom-engine-0.45.0'))
$runtimeOutput = Join-Path $outputRoot 'runtime'
$proofRoot = Join-Path $repoRoot 'work\proofs\engine-0.45.0'
$sourceRuntime = Join-Path $repoRoot 'work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none'
$sourceSitePackages = Join-Path $proofRoot '.venv\Lib\site-packages'
$pythonExe = Join-Path $sourceRuntime 'python.exe'
$pyproject = Join-Path $proofRoot 'pyproject.toml'
$lockFile = Join-Path $proofRoot 'uv.lock'
$runnerSource = Join-Path $proofRoot 'headroom_runtime.py'
$tokenizerCache = Join-Path $proofRoot 'tiktoken-cache'

$separator = [System.IO.Path]::DirectorySeparatorChar
$repoPrefix = $repoRoot.TrimEnd($separator) + $separator
$artifactPrefix = $artifactRoot.TrimEnd($separator) + $separator
if (-not $artifactRoot.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Artifact root resolves outside the project workspace.'
}
if (-not $outputRoot.StartsWith($artifactPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Package output resolves outside the packaging artifact root.'
}
if (-not (Test-Path -LiteralPath $pythonExe -PathType Leaf) -or
    -not (Test-Path -LiteralPath $sourceSitePackages -PathType Container) -or
    -not (Test-Path -LiteralPath $pyproject -PathType Leaf) -or
    -not (Test-Path -LiteralPath $lockFile -PathType Leaf) -or
    -not (Test-Path -LiteralPath $runnerSource -PathType Leaf) -or
    -not (Test-Path -LiteralPath $tokenizerCache -PathType Container)) {
    throw 'Pinned runtime inputs are incomplete; no package was written.'
}
$projectDefinition = Get-Content -LiteralPath $pyproject -Raw
if ($projectDefinition -notmatch 'headroom-ai\[proxy\]==0\.45\.0') {
    throw 'The proof project no longer pins the expected Headroom runtime.'
}
$sourceLockHash = (Get-FileHash -LiteralPath $lockFile -Algorithm SHA256).Hash.ToLowerInvariant()
$expectedInventory = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
[void]$expectedInventory.Add('runtime')
function Add-ExpectedTree {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Prefix
    )
    $base = [System.IO.Path]::GetFullPath($BasePath).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    Get-ChildItem -LiteralPath $base -Force -Recurse | ForEach-Object {
        $relative = $_.FullName.Substring($base.Length).TrimStart('\', '/')
        [void]$expectedInventory.Add(($Prefix + '\' + $relative))
    }
}
[void]$expectedInventory.Add('runtime\Lib')
[void]$expectedInventory.Add('runtime\Lib\site-packages')
Add-ExpectedTree -BasePath $sourceRuntime -Prefix 'runtime'
Add-ExpectedTree -BasePath $sourceSitePackages -Prefix 'runtime\Lib\site-packages'
[void]$expectedInventory.Add('runtime\tiktoken-cache')
Add-ExpectedTree -BasePath $tokenizerCache -Prefix 'runtime\tiktoken-cache'
[void]$expectedInventory.Add('runtime\headroom_runtime.py')
[void]$expectedInventory.Add('runtime\runtime-manifest.json')

function Get-RelativeInventory {
    param([Parameter(Mandatory = $true)][string]$BasePath)
    $base = [System.IO.Path]::GetFullPath($BasePath).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    Get-ChildItem -LiteralPath $base -Force -Recurse | ForEach-Object {
        $_.FullName.Substring($base.Length).TrimStart('\', '/')
    }
}

if (Test-Path -LiteralPath $outputRoot) {
    if (-not $ReuseExisting) {
        throw 'Package output already exists. Pass -ReuseExisting only for this same pinned lock.'
    }
    $resolvedOutput = (Resolve-Path -LiteralPath $outputRoot).Path
    if (-not $resolvedOutput.StartsWith($artifactPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing reuse: resolved package output is outside the packaging artifact root.'
    }
    $outputItem = Get-Item -LiteralPath $resolvedOutput -Force
    if ($outputItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
        throw 'Refusing reuse: package output is a reparse point.'
    }
    $reparsePoints = @(
        Get-ChildItem -LiteralPath $resolvedOutput -Force -Recurse -ErrorAction Stop |
            Where-Object { $_.Attributes -band [System.IO.FileAttributes]::ReparsePoint }
    )
    if ($reparsePoints.Count -gt 0) {
        throw 'Refusing reuse: package output contains a reparse point.'
    }
    $existingManifestPath = Join-Path $runtimeOutput 'runtime-manifest.json'
    if (-not (Test-Path -LiteralPath $existingManifestPath -PathType Leaf)) {
        throw 'Refusing reuse: previous package manifest is missing.'
    }
    $existingManifest = Get-Content -LiteralPath $existingManifestPath -Raw | ConvertFrom-Json
    if ($existingManifest.engine.lock_sha256 -ne $sourceLockHash) {
        throw 'Refusing reuse: existing package uses a different dependency lock.'
    }
    $existingInventory = @(Get-RelativeInventory -BasePath $outputRoot)
    $unexpected = @($existingInventory | Where-Object { -not $expectedInventory.Contains($_) })
    if ($unexpected.Count -gt 0) {
        throw 'Refusing reuse: package contains stale paths outside the current source inventory.'
    }
}

New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
New-Item -ItemType Directory -Path $runtimeOutput -Force | Out-Null
Get-ChildItem -LiteralPath $sourceRuntime -Force | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $runtimeOutput -Recurse -Force
}
$sitePackagesOutput = Join-Path $runtimeOutput 'Lib\site-packages'
New-Item -ItemType Directory -Path $sitePackagesOutput -Force | Out-Null
Get-ChildItem -LiteralPath $sourceSitePackages -Force | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $sitePackagesOutput -Recurse -Force
}
Copy-Item -LiteralPath $runnerSource -Destination (Join-Path $runtimeOutput 'headroom_runtime.py')
Copy-Item -LiteralPath $tokenizerCache -Destination (Join-Path $runtimeOutput 'tiktoken-cache') -Recurse -Force

$manifestPath = Join-Path $runtimeOutput 'runtime-manifest.json'
$runtimeFiles = @(
    Get-ChildItem -LiteralPath $runtimeOutput -File -Recurse -Force |
        Where-Object { $_.FullName -ne $manifestPath }
)
$runtimeBytes = [long](($runtimeFiles | Measure-Object -Property Length -Sum).Sum)
$lockedPackages = (Select-String -LiteralPath $lockFile -Pattern '^name = ').Count
$installedDistributions = @(
    Get-ChildItem -LiteralPath $sitePackagesOutput -Directory -Filter '*.dist-info' -Force
).Count
$manifest = [ordered]@{
    schema_version = 1
    engine = [ordered]@{
        name = 'headroom-ai'
        distribution = 'headroom-ai[proxy]'
        version = '0.45.0'
        lock_sha256 = $sourceLockHash
    }
    python = [ordered]@{
        implementation = 'CPython'
        version = '3.13.13'
        architecture = 'x64'
        executable = 'python.exe'
        sha256 = (Get-FileHash -LiteralPath (Join-Path $runtimeOutput 'python.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    runtime = [ordered]@{
        runner = 'headroom_runtime.py'
        site_packages = 'Lib/site-packages'
        file_count = $runtimeFiles.Count
        uncompressed_bytes = $runtimeBytes
    }
    dependencies = [ordered]@{
        locked_packages = $lockedPackages
        installed_distributions = $installedDistributions
    }
    source = [ordered]@{
        pyproject_sha256 = (Get-FileHash -LiteralPath $pyproject -Algorithm SHA256).Hash.ToLowerInvariant()
        lock_sha256 = $sourceLockHash
        runner_sha256 = (Get-FileHash -LiteralPath $runnerSource -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}
$manifestJson = $manifest | ConvertTo-Json -Depth 6
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText((Join-Path $runtimeOutput 'runtime-manifest.json'), $manifestJson + "`n", $utf8NoBom)

$actualInventory = @(Get-RelativeInventory -BasePath $outputRoot)
$actualSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($path in $actualInventory) { [void]$actualSet.Add($path) }
$missing = @($expectedInventory | Where-Object { -not $actualSet.Contains($_) })
$unexpected = @($actualInventory | Where-Object { -not $expectedInventory.Contains($_) })
if ($missing.Count -gt 0 -or $unexpected.Count -gt 0) {
    throw 'Packaged runtime inventory does not exactly match the pinned source inputs.'
}

'PASS portable runtime directory created: {0} files, {1:N1} MiB, {2} locked packages.' -f `
    $runtimeFiles.Count, ($runtimeBytes / 1MB), $lockedPackages
