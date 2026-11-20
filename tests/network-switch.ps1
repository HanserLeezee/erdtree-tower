param([Parameter(Mandatory=$true)][string]$PatchDir, [Parameter(Mandatory=$true)][string]$WorkDir)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../distribution/Tower-Network.ps1')
. (Join-Path $PSScriptRoot '../distribution/manual-network/Network-Files.ps1')
$fixture = Join-Path $WorkDir ('network-switch-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $fixture
function Get-Process { param($Name,$ErrorAction) }
function Must-Fail([scriptblock]$Action) {
    $failed = $false
    try { & $Action } catch { $failed = $true }
    if (!$failed) { throw 'Expected rejection.' }
}
[IO.File]::WriteAllText((Join-Path $fixture 'eldenring.exe'), 'game untouched')
[IO.File]::WriteAllText((Join-Path $fixture 'steam_api64.dll'), 'original API')
[IO.File]::WriteAllText((Join-Path $fixture 'winmm.dll'), 'original audio proxy')
[IO.File]::WriteAllText((Join-Path $fixture 'steam_api64_o.dll'), 'original fallback')
Set-TowerNetwork $fixture 'OnlineFix' $PatchDir
Assert-TowerNetwork $fixture 'OnlineFix'
if ([IO.File]::ReadAllText((Join-Path $fixture 'dlllist.txt')) -cne 'OnlineFix64.dll') { throw 'B proxy treats a trailing newline as an empty DLL path (error 126).' }
[IO.File]::WriteAllText((Join-Path $fixture 'dlllist.txt'), "OnlineFix64.dll`r`n")
Set-TowerNetwork $fixture 'OnlineFix' $PatchDir
if ([IO.File]::ReadAllText((Join-Path $fixture 'dlllist.txt')) -cne 'OnlineFix64.dll') { throw 'Existing B installation was not repaired.' }
Set-TowerNetwork $fixture 'Steam' $PatchDir
foreach ($entry in @(@('steam_api64.dll','original API'),@('winmm.dll','original audio proxy'),@('steam_api64_o.dll','original fallback'),@('eldenring.exe','game untouched'))) {
    if ([IO.File]::ReadAllText((Join-Path $fixture $entry[0])) -cne $entry[1]) { throw "Not restored: $($entry[0])" }
}
foreach ($name in @('OnlineFix64.dll','OnlineFix.ini','dlllist.txt')) {
    if (Test-Path -LiteralPath (Join-Path $fixture $name)) { throw "Unexpected B file: $name" }
}
function Get-Process { param($Name,$ErrorAction); [pscustomobject]@{Id=1} }
Must-Fail { Set-TowerNetwork $fixture 'OnlineFix' $PatchDir }
function Get-Process { param($Name,$ErrorAction) }
Must-Fail { Set-TowerNetwork $fixture 'OnlineFix' (Join-Path $fixture 'missing-patch') }
$lock = [IO.File]::Open((Join-Path $fixture 'winmm.dll'), 'Open', 'ReadWrite', 'None')
try { Must-Fail { Set-TowerNetwork $fixture 'OnlineFix' $PatchDir } } finally { $lock.Dispose() }
Assert-TowerNetwork $fixture 'Steam'
if ([IO.File]::ReadAllText((Join-Path $fixture 'steam_api64.dll')) -cne 'original API') { throw 'Failed switch changed A.' }
$script:realWrite = (Get-Command Write-TowerNetworkFile).ScriptBlock
$script:failOnce = $true
function Write-TowerNetworkFile([string]$Path, [byte[]]$Bytes) {
    if ($script:failOnce -and [IO.Path]::GetFileName($Path) -eq 'OnlineFix.ini') {
        $script:failOnce = $false
        throw 'Injected write failure after DLL replacement'
    }
    & $script:realWrite $Path $Bytes
}
Must-Fail { Set-TowerNetwork $fixture 'OnlineFix' $PatchDir }
Assert-TowerNetwork $fixture 'Steam'
if ([IO.File]::ReadAllText((Join-Path $fixture 'steam_api64.dll')) -cne 'original API') { throw 'Rollback lost original API.' }
$store = Join-Path $fixture '.tower-network'
if (Test-Path -LiteralPath (Join-Path $store 'pending.json')) { throw 'Successful rollback left pending journal.' }
$snapshot = Save-TowerNetworkSnapshot $fixture $store
Write-TowerNetworkJson (Join-Path $store 'pending.json') @{Before=@{Network='Steam';Original=$snapshot};Snapshot=$snapshot}
[IO.File]::WriteAllText((Join-Path $fixture 'steam_api64.dll'), 'interrupted replacement')
Set-TowerNetwork $fixture 'Steam' $PatchDir
if ([IO.File]::ReadAllText((Join-Path $fixture 'steam_api64.dll')) -cne 'original API') { throw 'Interrupted switch was not recovered.' }
Set-TowerNetwork $fixture 'OnlineFix' $PatchDir
[IO.File]::Delete((Join-Path $fixture 'OnlineFix64.dll'))
[IO.File]::Delete((Join-Path $fixture 'steam_api64.dll'))
Set-TowerNetwork $fixture 'Steam' $PatchDir
if ([IO.File]::ReadAllText((Join-Path $fixture 'steam_api64.dll')) -cne 'original API') { throw 'Cannot restore after B components are missing.' }
Write-Output 'PASS: A/B/A roundtrip, absent files, idempotence, running guard, missing patch, locked file, mid-write rollback, interrupted recovery, repeated roundtrip.'
