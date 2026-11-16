function Get-TowerNetwork([string]$GameDir) {
    if ([IO.File]::Exists((Join-Path $GameDir 'OnlineFix64.dll')) -or [IO.File]::Exists((Join-Path $GameDir 'OnlineFix.ini'))) { return 'OnlineFix' }
    return 'Steam'
}

function Assert-TowerLaunchNetwork([string]$GameDir, [string]$Network) {
    if ([IO.File]::Exists((Join-Path $GameDir '.tower-network/pending.json'))) { throw 'Interrupted manual patch operation. Run Restore-A.cmd from the separate patch kit before launching.' }
    Assert-TowerNetwork $GameDir $Network
    if ($Network -eq 'OnlineFix') {
        $bytes = [IO.File]::ReadAllBytes((Join-Path $GameDir 'dlllist.txt'))
        $canonical = [Text.Encoding]::ASCII.GetBytes('OnlineFix64.dll')
        if ([Convert]::ToBase64String($bytes) -cne [Convert]::ToBase64String($canonical)) {
            throw 'B patch list is not ready for this launcher. Run Install-B.cmd from the separate patch kit; the launcher will not change game files.'
        }
    }
}

function Assert-TowerNetwork([string]$GameDir, [string]$Network) {
    if ($Network -notin @('Steam','OnlineFix')) { throw 'Unknown network mode.' }
    if ($Network -eq 'Steam') {
        foreach ($name in @('OnlineFix64.dll','OnlineFix.ini')) {
            if (Test-Path -LiteralPath (Join-Path $GameDir $name)) { throw 'Selected profile does not match the manually installed patch. Restart the launcher.' }
        }
        return
    }
    $hashes = @{
        'OnlineFix64.dll' = '71a4f566abbf2ceb3b0d6159cd7950d120593c5badda4dde2711671957f16003'
        'steam_api64.dll' = 'ca1fa9a7609ab10b7926400559cf073e5888423cc156af72c6027d72a89eea73'
        'winmm.dll' = '54a6ece54641ee6e437caa351e493ddf4435f3e23e3d4ab4655c35f57b94fc59'
    }
    foreach ($name in $hashes.Keys) {
        $file = Join-Path $GameDir $name
        if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing $name. Install or restore the separate B patch manually." }
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ine $hashes[$name]) { throw "Unsupported OnlineFix component: $name. This profile supports the supplied 2.0.3 patch only." }
    }
    $ini = [IO.File]::ReadAllText((Join-Path $GameDir 'OnlineFix.ini'))
    foreach ($setting in @(@('RealAppId','1245620'), @('FakeAppId','480'))) {
        $entries = [regex]::Matches($ini, '(?mi)^\s*' + $setting[0] + '\s*=\s*([^\r\n;#]+)')
        if ($entries.Count -ne 1 -or $entries[0].Groups[1].Value.Trim() -ne $setting[1]) { throw "Unexpected OnlineFix $($setting[0])." }
    }
    if (Test-Path -LiteralPath (Join-Path $GameDir 'steam_api64_o.dll')) { throw 'Mixed Steam API proxies detected. Use the separate patch kit to install or restore the original files.' }
    $null = Get-TowerOnlineFixDllList $GameDir
}

function Get-TowerOnlineFixDllList([string]$GameDir) {
    $file = Join-Path $GameDir 'dlllist.txt'
    $entries = @([IO.File]::ReadAllLines($file) | ForEach-Object { $_.Trim().Replace('/','\') } | Where-Object { $_ })
    if (@($entries | Where-Object { $_ -ieq 'OnlineFix64.dll' }).Count -ne 1 -or
        @($entries | Where-Object { $_ -ine 'OnlineFix64.dll' -and $_ -ine 'SeamlessCoop\ersc.dll' }).Count -ne 0 -or
        @($entries | Where-Object { $_ -ieq 'SeamlessCoop\ersc.dll' }).Count -gt 1) {
        throw 'Unsupported dlllist.txt. Expected OnlineFix64.dll and optional SeamlessCoop\ersc.dll only.'
    }
    return $entries
}
