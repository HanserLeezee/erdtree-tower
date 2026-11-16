function Assert-TowerSessionGate([string]$Root,[string]$Loader) {
    if($Loader -ne 'ME3'){throw 'Private sessions require ME3. Select ME3 in launcher settings.'}
    $folder=Join-Path $Root 'SeamlessCoop'
    $file=Join-Path $folder 'gate-manifest.json'
    if(!(Test-Path -LiteralPath $file -PathType Leaf)){throw 'Session component manifest missing. Verify and repair the launcher package.'}
    $manifest=[IO.File]::ReadAllText($file) | ConvertFrom-Json
    if($manifest.protocol -ne 1 -or $manifest.files.'ersc.dll' -ine '2ebde62be864ba3d759728aac44c459f0f813e8b6d7a342304ce68f6513f280c'){throw 'Unsupported session component version. Verify and repair the package.'}
    foreach($name in 'ersc.dll','Tower.SeamlessGate.dll'){
        $path=Join-Path $folder $name
        if(!(Test-Path -LiteralPath $path -PathType Leaf) -or $manifest.files.$name -notmatch '^[0-9a-fA-F]{64}$'){throw "Session component missing: $name"}
        if((Get-FileHash -LiteralPath $path).Hash -ine $manifest.files.$name){throw "Session component damaged or mismatched: $name. Verify and repair the package."}
    }
}

function Add-TowerSessionGate([string]$Profile) {
    $pattern='(?ms)^\s*\[\[natives\]\][^\r\n]*\r?\n.*?(?=^\s*\[\[|\z)'
    $clean=[regex]::Replace($Profile,$pattern,{param($m)
        if($m.Value -match '(?i)Tower\.SeamlessGate\.dll'){return ''}
        return $m.Value
    })
    return $clean.TrimEnd()+@'

[[natives]]
id = "tower-session-gate"
path = 'SeamlessCoop/Tower.SeamlessGate.dll'
initializer = { function = "TowerGateInitialize" }
load_before = [{ id = "ersc.dll", optional = false }]
'@
}
