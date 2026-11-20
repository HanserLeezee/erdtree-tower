$script:TowerNetworkFiles = @('steam_api64.dll','winmm.dll','OnlineFix64.dll','OnlineFix.ini','dlllist.txt','steam_api64_o.dll','dlllist.txt.before-tower.bak')

function Write-TowerNetworkFile([string]$Path, [byte[]]$Bytes) {
    $temporary = $Path + '.' + [guid]::NewGuid().ToString('N') + '.tmp'
    try {
        [IO.File]::WriteAllBytes($temporary, $Bytes)
        if ([IO.File]::Exists($Path)) { [IO.File]::Replace($temporary, $Path, [NullString]::Value) }
        else { [IO.File]::Move($temporary, $Path) }
    } finally { if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) } }
}

function Write-TowerNetworkJson([string]$Path, $Value) {
    Write-TowerNetworkFile $Path ([Text.Encoding]::UTF8.GetBytes(($Value | ConvertTo-Json -Depth 6)))
}

function Save-TowerNetworkSnapshot([string]$GameDir, [string]$Store) {
    $id = [guid]::NewGuid().ToString('N')
    $folder = Join-Path $Store $id
    $null = [IO.Directory]::CreateDirectory($folder)
    $files = @{}
    foreach ($name in $script:TowerNetworkFiles) {
        $source = Join-Path $GameDir $name
        $files[$name] = $null
        if ([IO.File]::Exists($source)) {
            [IO.File]::Copy($source, (Join-Path $folder $name))
            $files[$name] = (Get-FileHash -LiteralPath (Join-Path $folder $name) -Algorithm SHA256).Hash
        }
    }
    Write-TowerNetworkJson (Join-Path $folder 'files.json') $files
    return $id
}

function Restore-TowerNetworkSnapshot([string]$GameDir, [string]$Store, [string]$Id) {
    if ($Id -notmatch '^[a-f0-9]{32}$') { throw 'Invalid A/B backup identifier.' }
    $folder = Join-Path $Store $Id
    $files = [IO.File]::ReadAllText((Join-Path $folder 'files.json')) | ConvertFrom-Json
    foreach ($name in $script:TowerNetworkFiles) {
        $property = $files.PSObject.Properties[$name]
        if (!$property) { throw "Incomplete A/B backup: $name" }
        if ($property.Value -and (Get-FileHash -LiteralPath (Join-Path $folder $name) -Algorithm SHA256).Hash -cne $property.Value) {
            throw "Damaged A/B backup: $name"
        }
    }
    foreach ($name in $script:TowerNetworkFiles) {
        $target = Join-Path $GameDir $name
        if ($files.$name) { Write-TowerNetworkFile $target ([IO.File]::ReadAllBytes((Join-Path $folder $name))) }
        elseif ([IO.File]::Exists($target)) { [IO.File]::Delete($target) }
    }
}

function Set-TowerNetwork([string]$GameDir, [string]$Network, [string]$PatchDir) {
    if ($Network -notin @('Steam','OnlineFix')) { throw 'Unknown network mode.' }
    if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) { throw 'Exit the game before switching A/B.' }
    $GameDir = [IO.Path]::GetFullPath($GameDir)
    if (![IO.File]::Exists((Join-Path $GameDir 'eldenring.exe'))) { throw 'Select the Game directory containing eldenring.exe.' }
    $store = Join-Path $GameDir '.tower-network'
    $null = [IO.Directory]::CreateDirectory($store)
    $mutex = [IO.File]::Open((Join-Path $store 'switch.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
    try {
        foreach ($name in $script:TowerNetworkFiles) {
            $file = Join-Path $GameDir $name
            if (Test-Path -LiteralPath $file) {
                if ((Get-Item -LiteralPath $file -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked patch file rejected: $name" }
                $handle = [IO.File]::Open($file, 'Open', 'ReadWrite', 'None')
                $handle.Dispose()
            }
        }
        $stateFile = Join-Path $store 'state.json'
        $pendingFile = Join-Path $store 'pending.json'
        if ([IO.File]::Exists($pendingFile)) {
            $pending = [IO.File]::ReadAllText($pendingFile) | ConvertFrom-Json
            Restore-TowerNetworkSnapshot $GameDir $store $pending.Snapshot
            Write-TowerNetworkJson $stateFile $pending.Before
            [IO.File]::Delete($pendingFile)
        }
        $state = if ([IO.File]::Exists($stateFile)) { [IO.File]::ReadAllText($stateFile) | ConvertFrom-Json } else { [pscustomobject]@{Network='Steam';Original=$null} }
        if ($state.Network -notin @('Steam','OnlineFix')) { throw 'Invalid A/B state; preserve .tower-network for recovery.' }
        if ($Network -ne 'Steam' -or $state.Network -eq 'Steam') { Assert-TowerNetwork $GameDir $state.Network }
        if ($state.Network -eq $Network) {
            if ($Network -eq 'OnlineFix') { Set-TowerOnlineFixDllList $GameDir }
            return
        }
        if ($Network -eq 'OnlineFix') { Assert-TowerNetwork $PatchDir 'OnlineFix' }
        if ($Network -eq 'OnlineFix' -and ![IO.File]::Exists((Join-Path $GameDir 'steam_api64.dll'))) { throw 'Missing original steam_api64.dll; cannot back up A.' }
        $snapshot = Save-TowerNetworkSnapshot $GameDir $store
        $next = @{Network=$Network;Original=$state.Original}
        if ($Network -eq 'OnlineFix') { $next.Original = $snapshot }
        Write-TowerNetworkJson $pendingFile @{Before=$state;Snapshot=$snapshot}
        try {
            if ($Network -eq 'Steam') { Restore-TowerNetworkSnapshot $GameDir $store $state.Original }
            else {
                foreach ($name in @('steam_api64.dll','winmm.dll','OnlineFix64.dll','OnlineFix.ini')) {
                    Write-TowerNetworkFile (Join-Path $GameDir $name) ([IO.File]::ReadAllBytes((Join-Path $PatchDir $name)))
                }
                Write-TowerNetworkFile (Join-Path $GameDir 'dlllist.txt') ([Text.Encoding]::ASCII.GetBytes('OnlineFix64.dll'))
                foreach ($name in @('steam_api64_o.dll','dlllist.txt.before-tower.bak')) { [IO.File]::Delete((Join-Path $GameDir $name)) }
            }
            Assert-TowerNetwork $GameDir $Network
            Write-TowerNetworkJson $stateFile $next
            [IO.File]::Delete($pendingFile)
        } catch {
            $failure = $_
            Restore-TowerNetworkSnapshot $GameDir $store $snapshot
            Write-TowerNetworkJson $stateFile $state
            [IO.File]::Delete($pendingFile)
            throw $failure
        }
    } finally { $mutex.Dispose() }
}

function Set-TowerOnlineFixDllList([string]$GameDir) {
    if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) { throw 'Exit the game before preparing the OnlineFix loader.' }
    Assert-TowerNetwork $GameDir 'OnlineFix'
    $entries = @(Get-TowerOnlineFixDllList $GameDir)
    $file = Join-Path $GameDir 'dlllist.txt'
    # The supplied proxy loops on EOF and tries loading an empty path after a final newline.
    if ($entries.Count -eq 1) {
        $canonical = [Text.Encoding]::ASCII.GetBytes('OnlineFix64.dll')
        if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($file)) -cne [Convert]::ToBase64String($canonical)) {
            Write-TowerNetworkFile $file $canonical
        }
        return
    }
    $backup = $file + '.before-tower.bak'
    $bytes = [IO.File]::ReadAllBytes($file)
    if (Test-Path -LiteralPath $backup) {
        if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($backup)) -cne [Convert]::ToBase64String($bytes)) { throw 'Existing dlllist backup differs; no changes made.' }
    } else {
        $stream = [IO.File]::Open($backup, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
        try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
    }
    $temporary = $file + '.tower.tmp'
    $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try {
        $value = [Text.Encoding]::ASCII.GetBytes('OnlineFix64.dll')
        $stream.Write($value, 0, $value.Length)
    } finally { $stream.Dispose() }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($file)) -cne [Convert]::ToBase64String($bytes)) {
        [IO.File]::Delete($temporary)
        throw 'dlllist.txt changed during preparation; no changes applied.'
    }
    [IO.File]::Replace($temporary, $file, [NullString]::Value)
    Write-Host 'OnlineFix proxy enabled; Seamless is loaded once by the tower loader. Original dlllist.txt backed up.'
}
