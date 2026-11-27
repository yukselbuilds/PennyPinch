function Get-Digest([string]$path) {
    (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Enter-CodexFixture {
    $result.phase = 'codex_setup'
    New-Item -ItemType Directory -Path $clientRoot -ErrorAction Stop | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'codex.exe') -Destination $clientRoot
    $client = Join-Path $clientRoot 'codex.exe'
    $digest = Get-Digest $client
    if ($digest -cne '9d563ee4e0441ff9d59d612013e0460ddf8e5709735c2d9f6b98b7ef8ca4eb42') {
        throw 'codex_digest_mismatch'
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $client
    if ($signature.Status -ne 'Valid' -or
        $signature.SignerCertificate.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName,$false) -cne 'OpenAI OpCo, LLC') {
        throw 'codex_signature_not_valid'
    }
    $env:PATH = "$clientRoot;$env:PATH"
    $env:CODEX_HOME = Join-Path $clientRoot 'profile'
    $env:CODEX_SQLITE_HOME = Join-Path $clientRoot 'cli-state'
    New-Item -ItemType Directory -Path $env:CODEX_HOME,$env:CODEX_SQLITE_HOME | Out-Null
    [IO.File]::WriteAllText($clientConfig, ('cli_auth_credentials_store = "file"' + [char]10 +
        'web_search = "disabled"' + [char]10), [Text.UTF8Encoding]::new($false))
    $result.codex = [ordered]@{ executable_sha256 = $digest; original_config_sha256 = (Get-Digest $clientConfig); version = '0.166.0' }
    $result.checks.codex_authenticode_valid = $true
    $result.phase = 'codex_local_preflight'
    $smoke = New-Item -ItemType Directory -Path (Join-Path $clientRoot 'local-preflight')
    $python = Join-Path $destination 'runtime/payload/python.exe'
    $script = Join-Path $PSScriptRoot 'cli-preflight.py'
    $null = & $python -I -B $script $client $smoke.FullName
    $code = $LASTEXITCODE
    $record = Join-Path $smoke.FullName 'synthetic-preflight.json'
    if (Test-Path -LiteralPath $record) { $result.codex.preflight = Get-Content -LiteralPath $record -Raw -Encoding UTF8 | ConvertFrom-Json }
    if ($code -ne 0 -or !$result.codex.preflight.passed) { throw 'codex_local_preflight_not_verified' }
    $result.checks.codex_local_preflight = $true
}

function Test-CodexLive([int]$enginePort) {
    $result.phase = 'codex_discovery'
    if (!(Wait-For 18 { Invoke-Page "(() => {const b=document.querySelector('button[data-screen=applications]');if(!b||b.disabled)return false;b.click();return true;})()" })) {
        throw 'applications_control_missing'
    }
    $card = "Array.from(document.querySelectorAll('article.application-card')).find(c=>c.querySelector('h2')?.textContent.trim()==='Codex')"
    if (!(Wait-For 75 { Invoke-Page "(() => {const c=$card;const d=c?.querySelector('details');if(!d)return false;d.open=true;return c.querySelector('.application-card__metadata')?.textContent.includes('0.166.0');})()" })) {
        throw 'codex_not_discovered'
    }
    if ((Get-Digest $clientConfig) -cne $result.codex.original_config_sha256) { throw 'discovery_modified_codex_config' }
    $result.checks.codex_discovered = $true
    $result.phase = 'codex_connect'
    if (!(Wait-For 25 { Invoke-Page "(() => {const c=$card;const b=c?.querySelector('.application-card__actions .button-primary');if(!b||b.disabled)return false;b.click();return true;})()" })) {
        throw 'codex_card_action_unavailable'
    }
    $dialog = 'dialog.connection-dialog[open]'
    $button = "const d=document.querySelector('$dialog');const buttons=Array.from(d?.querySelectorAll('button')||[]);"
    $connect = "const b=buttons.find(e=>e.getClientRects().length&&!e.disabled&&/^(Connect|Ba\u011flan)$/.test(e.textContent.trim()));if(!b)return false;b.click();return true;"
    if (!(Wait-For 40 { Invoke-Page "(() => { $button $connect })()" }) -or
        !(Wait-For 60 { Invoke-Page "(() => /Routing saved|Connection saved|Y\u00f6nlendirme kaydedildi|Ba\u011flant\u0131 kaydedildi/.test(document.querySelector('$dialog')?.textContent||''))()" })) {
        throw 'codex_route_apply_not_confirmed'
    }
    $receipt = Join-Path $profile 'ownership/codex.json'
    if ((Get-Content -LiteralPath $receipt -Raw -Encoding UTF8 | ConvertFrom-Json).state -cne 'active') {
        throw 'codex_ownership_not_active'
    }
    $result.codex.applied_config_sha256 = Get-Digest $clientConfig
    if ($result.codex.applied_config_sha256 -ceq $result.codex.original_config_sha256) { throw 'codex_route_not_written' }
    $result.checks.codex_connect_applied = $true
    Publish-Progress connected

    $result.phase = 'codex_provider'
    $python = Join-Path $destination 'runtime/payload/python.exe'
    $script = Join-Path $PSScriptRoot 'live-client.py'
    $arguments = '-I -B "{0}" "{1}" "{2}" "{3}"' -f $script, $clientRoot, $profile, $enginePort
    $call = Start-Process -FilePath $python -ArgumentList $arguments -WindowStyle Hidden -PassThru
    try {
        if (!$call.WaitForExit(1010000)) {
            $call.Kill()
            throw 'codex_live_probe_timeout'
        }
    } finally { $call.Dispose() }
    $result.codex.live = Get-Content -LiteralPath (Join-Path $clientRoot 'live-result.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    if (!$result.codex.live.temporary_auth_removed) { throw 'temporary_codex_auth_remains' }
    if ($result.codex.live.status -ceq 'pass') { $result.checks.live_chatgpt_turn_observed = $true }

    $result.phase = 'codex_undo'
    $remove = "const b=buttons.find(e=>e.getClientRects().length&&!e.disabled&&/^(Remove connection|Ba\u011flant\u0131y\u0131 kald\u0131r)$/.test(e.textContent.trim()));if(!b)return false;b.click();return true;"
    if (!(Wait-For 20 { Invoke-Page "(() => { $button $remove })()" }) -or
        !(Wait-For 20 { Invoke-Page "(() => { const buttons=Array.from(document.querySelectorAll('#disconnect-dialog[open] button'));$remove })()" }) -or
        !(Wait-For 35 { Invoke-Page "(() => /Routing released|Y\u00f6nlendirme geri al\u0131nd\u0131/.test(document.querySelector('$dialog')?.textContent||''))()" })) {
        throw 'codex_undo_not_confirmed'
    }
    $result.codex.restored_config_sha256 = Get-Digest $clientConfig
    if ($result.codex.restored_config_sha256 -cne $result.codex.original_config_sha256 -or
        (Get-Content -LiteralPath $receipt -Raw -Encoding UTF8 | ConvertFrom-Json).state -cne 'released') {
        throw 'codex_undo_changed_config'
    }
    $result.checks.codex_undo_restored_original = $true
    if ($result.checks.live_chatgpt_turn_observed) { Publish-Progress provider }
    if (!(Wait-For 20 { Invoke-Page "(() => {$button const b=buttons.find(e=>e.getClientRects().length&&!e.disabled&&e.closest('.connection-dialog__footer')&&/^(Close|Kapat)$/.test(e.textContent.trim()));if(!b)return false;b.click();return true;})()" }) -or
        !(Wait-For 20 { Invoke-Page "(() => {const b=document.querySelector('button[data-screen=overview]');if(!b||b.disabled)return false;b.click();return true;})()" })) {
        throw 'overview_return_failed'
    }
}
