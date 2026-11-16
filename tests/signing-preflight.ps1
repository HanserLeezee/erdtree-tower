param([Parameter(Mandatory=$true)][string]$Launcher)
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('tower-signing-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
Copy-Item -LiteralPath $Launcher -Destination (Join-Path $root 'ErdtreeTowerLauncher.exe')
Copy-Item -LiteralPath $Launcher -Destination (Join-Path $root 'steam_api64.dll')
$sign = Join-Path $PSScriptRoot '../distribution/Sign-Tower.ps1'
$before = (Get-FileHash -LiteralPath (Join-Path $root 'ErdtreeTowerLauncher.exe')).Hash
$result = @(& $sign -PackageRoot $root -CheckOnly)
if ($result.Count -ne 1 -or $result[0].Company -ne 'TamikiP' -or $result[0].Signature -ne 'NotSigned') { throw 'Wrong publisher, signature state or signing whitelist.' }
$failed = $false
try { & $sign -PackageRoot $root } catch { $failed = $true }
if (!$failed) { throw 'Signing without a certificate was accepted.' }
if ((Get-FileHash -LiteralPath (Join-Path $root 'ErdtreeTowerLauncher.exe')).Hash -ne $before) { throw 'Preflight changed a binary.' }
if (Test-Path -LiteralPath (Join-Path $root 'signing-report.json')) { throw 'Unsigned files were reported as signed.' }
Write-Output 'PASS: TamikiP metadata, unsigned state, third-party exclusion and no-certificate rejection without mutations.'
