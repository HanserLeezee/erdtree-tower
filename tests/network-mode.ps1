param([string]$WorkDir = [IO.Path]::GetTempPath())
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../distribution/Tower-Network.ps1')
if (Get-Command Set-TowerNetwork,Set-TowerOnlineFixDllList -ErrorAction SilentlyContinue) { throw 'Main helper must remain read-only.' }
$fixture = Join-Path $WorkDir ('network-detect-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
if ((Get-TowerNetwork $fixture) -ne 'Steam') { throw 'Wrong default network.' }
Assert-TowerLaunchNetwork $fixture 'Steam'
[IO.File]::WriteAllText((Join-Path $fixture 'OnlineFix.ini'), 'fixture only, not a usable patch')
if ((Get-TowerNetwork $fixture) -ne 'OnlineFix') { throw 'External configuration was not detected.' }
function Snapshot { @(Get-ChildItem -LiteralPath $fixture -Recurse -File | Sort-Object FullName | Get-FileHash | Select-Object Path,Hash) | ConvertTo-Json -Compress }
$before = Snapshot
foreach ($mode in 'Steam','OnlineFix') {
    $failed = $false
    try { Assert-TowerLaunchNetwork $fixture $mode } catch { $failed = $true }
    if (!$failed) { throw 'Incomplete/mismatched profile accepted.' }
}
if ($before -cne (Snapshot)) { throw 'Validation modified files.' }
$pending = Join-Path $fixture '.tower-network'
New-Item -ItemType Directory -Path $pending | Out-Null
[IO.File]::WriteAllText((Join-Path $pending 'pending.json'), '{}')
$before = Snapshot
$failed = $false
try { Assert-TowerLaunchNetwork $fixture 'Steam' } catch { $failed = $_.Exception.Message -like '*Interrupted*' }
if (!$failed -or $before -cne (Snapshot)) { throw 'Interrupted operation was not rejected read-only.' }
Write-Output 'PASS: profile detection, incomplete/mismatched configuration and interrupted-state rejection; no deployment or game mutation.'
