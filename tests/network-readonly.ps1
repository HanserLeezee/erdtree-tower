param([Parameter(Mandatory=$true)][string]$PatchDir, [Parameter(Mandatory=$true)][string]$WorkDir)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../distribution/Tower-Network.ps1')
if (Get-Command Set-TowerNetwork,Set-TowerOnlineFixDllList -ErrorAction SilentlyContinue) { throw 'Main-package helper exposes patch mutation.' }
$fixture = Join-Path $WorkDir ('network-readonly-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $fixture
if ((Get-TowerNetwork $fixture) -ne 'Steam') { throw 'Default profile is not Steam.' }
Assert-TowerLaunchNetwork $fixture 'Steam'
foreach ($name in @('OnlineFix64.dll','steam_api64.dll','winmm.dll','OnlineFix.ini')) {
    Copy-Item -LiteralPath (Join-Path $PatchDir $name) -Destination $fixture
}
$list = Join-Path $fixture 'dlllist.txt'
[IO.File]::WriteAllText($list, 'OnlineFix64.dll')
$before = @(Get-ChildItem -LiteralPath $fixture -Recurse -File | Get-FileHash | Select-Object Path,Hash | ConvertTo-Json)
if ((Get-TowerNetwork $fixture) -ne 'OnlineFix') { throw 'Manual B not detected.' }
Assert-TowerLaunchNetwork $fixture 'OnlineFix'
$after = @(Get-ChildItem -LiteralPath $fixture -Recurse -File | Get-FileHash | Select-Object Path,Hash | ConvertTo-Json)
if ($before -cne $after) { throw 'Launch validation mutated game files.' }
[IO.File]::WriteAllText($list, "OnlineFix64.dll`r`n")
$failed = $false
try { Assert-TowerLaunchNetwork $fixture 'OnlineFix' } catch { $failed = $true }
if (!$failed -or [IO.File]::ReadAllText($list) -cne "OnlineFix64.dll`r`n") { throw 'Invalid list must be rejected without repair.' }
Write-Output 'PASS: automatic profile detection is read-only; malformed B requires manual repair.'
