$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
. (Join-Path $root 'distribution/Tower-SessionGate.ps1')
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('tower-gate-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $fixture 'SeamlessCoop') | Out-Null
$folder=Join-Path $fixture 'SeamlessCoop'
Copy-Item -LiteralPath (Join-Path $root 'SeamlessCoop/ersc.dll') -Destination $folder
[IO.File]::WriteAllText((Join-Path $folder 'Tower.SeamlessGate.dll'),'fixture')
$files=@{}
foreach($name in 'ersc.dll','Tower.SeamlessGate.dll'){$files[$name]=(Get-FileHash (Join-Path $folder $name)).Hash}
@{protocol=1;files=$files}|ConvertTo-Json -Depth 4|Set-Content (Join-Path $folder 'gate-manifest.json') -Encoding UTF8
Assert-TowerSessionGate $fixture 'ME3'
try {Assert-TowerSessionGate $fixture 'YAFSML';throw 'accepted unsupported loader'}catch{if($_.Exception.Message -notlike '*require ME3*'){throw}}
$profile="profileVersion = 'v1'`n[[natives]]`npath = 'SeamlessCoop/ersc.dll'`n[[packages]]`npath = 'scene-assets'`n"
$once=Add-TowerSessionGate $profile
$twice=Add-TowerSessionGate $once
if([regex]::Matches($twice,"path = 'SeamlessCoop/Tower.SeamlessGate.dll'").Count -ne 1){throw 'Gate duplicated'}
if($twice -notmatch 'initializer = \{ function = "TowerGateInitialize" \}' -or $twice -notmatch 'load_before = \[\{ id = "ersc.dll", optional = false \}\]'){throw 'Unsafe load ordering'}
if($twice -notmatch "path = 'scene-assets'" -or $twice -notmatch "path = 'SeamlessCoop/ersc.dll'"){throw 'Original profile lost'}
[IO.File]::WriteAllText((Join-Path $folder 'Tower.SeamlessGate.dll'),'changed')
try {Assert-TowerSessionGate $fixture 'ME3';throw 'accepted changed adapter'}catch{if($_.Exception.Message -notlike '*damaged or mismatched*'){throw}}
Write-Output 'PASS: gate component hashes, supported loader, profile migration and initialization ordering.'
