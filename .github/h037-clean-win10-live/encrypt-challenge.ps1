$ErrorActionPreference = 'Stop'
$key = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'device-public.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$deviceCode = [Console]::In.ReadToEnd()
if ($deviceCode -cnotmatch '\A[A-Z0-9][A-Z0-9-]{4,27}\z') { exit 1 }
$parts = [Security.Cryptography.RSAParameters]::new()
$parts.Modulus = [Convert]::FromBase64String($key.modulus)
$parts.Exponent = [Convert]::FromBase64String($key.exponent)
if ($parts.Modulus.Length -ne 384 -or $key.challenge_id -cnotmatch '\A[a-f0-9]{32}\z') { exit 1 }
$rsa = [Security.Cryptography.RSACng]::new()
try {
    $rsa.ImportParameters($parts)
    $plain = [Text.Encoding]::UTF8.GetBytes("$($key.challenge_id):$deviceCode")
    $sealed = $rsa.Encrypt($plain, [Security.Cryptography.RSAEncryptionPadding]::OaepSHA256)
    [Console]::Out.Write([Convert]::ToBase64String($sealed))
} finally {
    if ($plain) { [Array]::Clear($plain, 0, $plain.Length) }
    $rsa.Dispose()
}
