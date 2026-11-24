$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('tower direct test ' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../distribution/Start-Tower.ps1') -Destination $root
$game = Join-Path $root "game 'quoted'"
New-Item -ItemType Directory -Path $game | Out-Null
[IO.File]::WriteAllText((Join-Path $game 'eldenring.exe'), 'test fixture, never executed')
@'
param($GameDir,$Loader,$Network,[switch]$CheckOnly,$PartyConfig)
if ($Loader -ne 'YAFSML' -or $Network -ne 'OnlineFix') { throw 'Lost launch options' }
if (!$CheckOnly) { Write-Output "DIRECT_PID=$PID"; Write-Output '资源检查通过' }
'@ | Set-Content -LiteralPath (Join-Path $root 'Launch-Seamless-Tower.ps1') -Encoding utf8
@'
param($GameDir)
$PSModuleAutoLoadingPreference = 'None'
$env:PSModulePath = 'Z:\missing-tower-test-modules'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::WriteLine("ROOT_PID=$PID")
& ([IO.Path]::Combine($PSScriptRoot,'Start-Tower.ps1')) -GameDir $GameDir -Loader YAFSML -Network OnlineFix -ErrorFile ([IO.Path]::Combine($PSScriptRoot,'error.txt'))
exit $LASTEXITCODE
'@ | Set-Content -LiteralPath (Join-Path $root 'direct.ps1') -Encoding utf8
$output = & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $root 'direct.ps1') -GameDir $game
if ($LASTEXITCODE -ne 0) { throw 'Direct entry failed' }
$parent = @($output | Where-Object { $_ -match '^ROOT_PID=' })[0] -replace '^ROOT_PID=',''
$child = @($output | Where-Object { $_ -match '^DIRECT_PID=' })[0] -replace '^DIRECT_PID=',''
if (!$parent -or $parent -ne $child) { throw 'Entry spawned another PowerShell process' }
if (!(Test-Path -LiteralPath (Join-Path $root 'GamePath.txt'))) { throw 'Shared game path was not saved' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../distribution/Tower-Network.ps1') -Destination $root
[IO.File]::WriteAllText((Join-Path $game 'OnlineFix64.dll'), 'detection fixture')
$automatic = & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $root 'Start-Tower.ps1') -GameDir $game -Loader YAFSML
if ($LASTEXITCODE -ne 0 -or !($automatic -match '^DIRECT_PID=')) { throw 'Default command entry did not detect external B.' }
if ([IO.File]::ReadAllText((Join-Path $game 'OnlineFix64.dll')) -cne 'detection fixture') { throw 'Automatic launch replaced game files.' }
Write-Output 'PASS: direct entry uses one process, forwards loader/network options and bootstraps modules with inherited module discovery disabled.'
