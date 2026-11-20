param([Parameter(Mandatory=$true)][string]$KitDir, [Parameter(Mandatory=$true)][string]$WorkDir)
$ErrorActionPreference = 'Stop'
function Get-Process { param($Name,$ErrorAction) }
$fixture = Join-Path $WorkDir ('manual-kit-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $fixture
[IO.File]::WriteAllText((Join-Path $fixture 'eldenring.exe'), 'game untouched')
[IO.File]::WriteAllText((Join-Path $fixture 'steam_api64.dll'), 'original local API')
& (Join-Path $KitDir 'Network-Manual.ps1') -Action Install -GameDir $fixture
$recovery = Join-Path $fixture '.tower-network/restore-tools/Network-Manual.ps1'
if (!(Test-Path -LiteralPath $recovery) -or !(Test-Path -LiteralPath (Join-Path $fixture '.tower-network/Restore-A.cmd'))) { throw 'Standalone restore kit missing.' }
[IO.File]::Delete((Join-Path $fixture 'OnlineFix64.dll'))
[IO.File]::Delete((Join-Path $fixture 'steam_api64.dll'))
& $recovery -Action Restore -GameDir $fixture
if ([IO.File]::ReadAllText((Join-Path $fixture 'steam_api64.dll')) -cne 'original local API') { throw 'Standalone restore did not restore local original.' }
if (Test-Path -LiteralPath (Join-Path $fixture 'OnlineFix.ini')) { throw 'B configuration was not removed.' }
if ([IO.File]::ReadAllText((Join-Path $fixture 'eldenring.exe')) -cne 'game untouched') { throw 'Game executable changed.' }
Write-Output 'PASS: manually invoked kit installs B and restores local originals with missing B DLLs, using only the retained recovery kit.'
