$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot '../Launch-Seamless-Tower.ps1'
$tokens=$null; $errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($source,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Launch script parse failed'}
$function=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Get-TowerJunctionPath'},$true)
if(!$function){throw 'Missing junction path normalization'}
. ([scriptblock]::Create($function.Extent.Text))
foreach($path in @('C:\Users\Test User\scene-assets','\\server\share\scene-assets')) {
    $expected=[IO.Path]::GetFullPath($path).TrimEnd('\')
    $native=if($path.StartsWith('\\')){'UNC\'+$path.Substring(2)}else{$path}
    foreach($value in @($path,($path+'\'),('Global\'+$path),('\Global\'+$path),('\\?\'+$native),('\??\'+$native),('\??\Global\'+$native))) {
        if((Get-TowerJunctionPath $value) -ine $expected){throw "Normalization failed: $value"}
    }
}
$root=Join-Path ([IO.Path]::GetFullPath("$PSScriptRoot/../work")) ('junction-test-'+[guid]::NewGuid().ToString('N'))
$assets=Join-Path $root 'assets'
$runtime=Join-Path $root 'runtime'
New-Item -ItemType Directory -Path $assets,$runtime -Force | Out-Null
$text=[IO.File]::ReadAllText($source)
$start=$text.IndexOf('$assetLink = Join-Path $runtime')
$end=$text.IndexOf('foreach ($path in @($native,$assets))',$start)
if($start -lt 0 -or $end -lt 0){throw 'Junction setup block not found'}
$setup=[scriptblock]::Create($text.Substring($start,$end-$start))
& $setup
$link=Join-Path $runtime 'scene-assets'
$created=(Get-Item -LiteralPath $link -Force).CreationTimeUtc
& $setup
if((Get-Item -LiteralPath $link -Force).CreationTimeUtc -ne $created){throw 'Second launch recreated valid junction'}
if(@(Get-ChildItem -LiteralPath $runtime).Count -ne 1){throw 'Second launch left stale resources'}
$assets=Join-Path $root 'moved-assets'
New-Item -ItemType Directory -Path $assets | Out-Null
& $setup
$target=@((Get-Item -LiteralPath $link -Force).Target)[0]
if((Get-TowerJunctionPath $target) -ine (Get-TowerJunctionPath $assets)){throw 'Stale junction not replaced'}
Write-Output "Junction path and repeated launch tests passed on PowerShell $($PSVersionTable.PSVersion). Fixtures: $root"
