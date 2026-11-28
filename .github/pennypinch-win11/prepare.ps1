$ErrorActionPreference = 'Stop'
$phase = 'portable_archive'
$destination = 'C:\PP-Verification'
$report = [ordered]@{status = 'fail'; phase = $phase}
try {
    $descriptor = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'bundle.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $source = Join-Path $PSScriptRoot 'portable.zip'
    if (Test-Path -LiteralPath $destination) { throw 'existing_destination' }
    if ($descriptor.archive_scope -cne 'engine_only') { throw 'wrong_package_scope' }
    if ((Get-Item -LiteralPath $source).Length -ne $descriptor.portable_archive_bytes -or
        (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ine $descriptor.portable_archive_sha256) {
        throw 'portable_archive_hash_mismatch'
    }
    $report.archive_sha256 = $descriptor.portable_archive_sha256
    $report.phase = 'portable_extract'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory($source, $destination)
    $report.phase = 'portable_verify'
    $resources = @(
        @{ name = 'runtime/desktop_helper.py'; hash = $descriptor.candidate_helper_sha256 },
        @{ name = 'package-inputs.json'; hash = $descriptor.resource_receipt_sha256 }
    )
    foreach ($file in $resources) {
        $path = Join-Path $destination $file.name
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $file.hash) {
            throw 'extracted_resource_hash_mismatch'
        }
    }
    $count = @([IO.Directory]::EnumerateFiles($destination, '*', [IO.SearchOption]::AllDirectories)).Length
    if ($count -ne $descriptor.file_count) { throw 'extracted_file_count_mismatch' }
    $report.file_count = $count
    $report.archive_scope = $descriptor.archive_scope
    $report.candidate_helper_sha256 = $descriptor.candidate_helper_sha256
    $report.status = 'pass'
    $report.phase = 'complete'
} catch {
    $report.error_type = $_.Exception.GetType().Name
    if ($_.Exception.Message -cmatch '^[a-z_]{1,70}$') { $report.error_code = $_.Exception.Message }
} finally {
    $p = 'C:\PP-Portable-Preparation.json'
    [IO.File]::WriteAllText($p, (ConvertTo-Json -InputObject $report -Compress -Depth 5), [Text.UTF8Encoding]::new($false))
}
if ($report.status -ne 'pass') { exit 1 }
