[CmdletBinding(PositionalBinding = $false)]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $CodeGraphArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$separator = [System.IO.Path]::DirectorySeparatorChar
$repoPrefix = $repoRoot.TrimEnd($separator) + $separator
$toolchainRoot = Join-Path $repoRoot 'work\toolchains\codegraph\1.6.2\codegraph-win32-x64'
$toolchainPrefix = [System.IO.Path]::GetFullPath($toolchainRoot).TrimEnd($separator) + $separator

function Resolve-PinnedFile([string[]] $Parts, [string] $Label) {
    $candidate = $repoRoot
    foreach ($part in $Parts) {
        $candidate = Join-Path $candidate $part
        if (-not (Test-Path -LiteralPath $candidate)) {
            throw "Pinned project-local CodeGraph $Label is missing."
        }
        $item = Get-Item -LiteralPath $candidate -Force
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'Pinned CodeGraph path contains a reparse point.'
        }
    }

    $resolved = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $candidate).Path)
    if (-not $resolved.StartsWith($toolchainPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Pinned CodeGraph path escaped its versioned toolchain directory.'
    }
    return $resolved
}

$nodePath = Resolve-PinnedFile @('work', 'toolchains', 'codegraph', '1.6.2', 'codegraph-win32-x64', 'node.exe') 'runtime'
$entryPoint = Resolve-PinnedFile @('work', 'toolchains', 'codegraph', '1.6.2', 'codegraph-win32-x64', 'lib', 'dist', 'bin', 'codegraph.js') 'entry point'

$settingNames = @(
    'DO_NOT_TRACK',
    'CODEGRAPH_TELEMETRY',
    'CODEGRAPH_NO_UPDATE_CHECK',
    'CODEGRAPH_DIR'
)
$processEnvironment = [System.Environment]::GetEnvironmentVariables([System.EnvironmentVariableTarget]::Process)
$previousEnvironment = @{}
foreach ($name in $settingNames) {
    if ($processEnvironment.Contains($name)) {
        $previousEnvironment[$name] = [pscustomobject]@{
            Present = $true
            Value = $processEnvironment[$name]
        }
    } else {
        $previousEnvironment[$name] = [pscustomobject]@{
            Present = $false
            Value = $null
        }
    }
}

$invokeArgs = @()
if ($CodeGraphArgs -and $CodeGraphArgs.Count -gt 0) {
    $invokeArgs = @($CodeGraphArgs)
} else {
    $invokeArgs = @('--help')
}

$previousLocation = Get-Location
$exitCode = 1
try {
    [System.Environment]::SetEnvironmentVariable('DO_NOT_TRACK', '1', [System.EnvironmentVariableTarget]::Process)
    [System.Environment]::SetEnvironmentVariable('CODEGRAPH_TELEMETRY', '0', [System.EnvironmentVariableTarget]::Process)
    [System.Environment]::SetEnvironmentVariable('CODEGRAPH_NO_UPDATE_CHECK', '1', [System.EnvironmentVariableTarget]::Process)
    [System.Environment]::SetEnvironmentVariable('CODEGRAPH_DIR', '.codegraph', [System.EnvironmentVariableTarget]::Process)

    Set-Location -LiteralPath $repoRoot
    & $nodePath '--liftoff-only' '--disable-warning=ExperimentalWarning' $entryPoint @invokeArgs
    $exitCode = $LASTEXITCODE
} finally {
    try {
        foreach ($name in $settingNames) {
            $previous = $previousEnvironment[$name]
            if ($previous.Present) {
                [System.Environment]::SetEnvironmentVariable(
                    $name,
                    [string]$previous.Value,
                    [System.EnvironmentVariableTarget]::Process
                )
            } else {
                Remove-Item -LiteralPath ("Env:" + $name) -ErrorAction SilentlyContinue
            }
        }
    } finally {
        Set-Location -LiteralPath $previousLocation
    }
}

if ($MyInvocation.InvocationName -eq '.') {
    return $exitCode
}

exit $exitCode