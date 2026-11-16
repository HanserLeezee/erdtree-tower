param([Parameter(Mandatory=$true)][string]$GameDir,
    [ValidateSet('Auto','Steam','OnlineFix')][string]$Network = 'Auto')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
try {
    foreach ($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')) {
        Import-Module ([IO.Path]::Combine($PSHOME,'Modules',$module,($module + '.psd1'))) -ErrorAction Stop
    }
    . (Join-Path $PSScriptRoot 'Tower-Network.ps1')
    if ($Network -eq 'Auto') { $Network = Get-TowerNetwork $GameDir }
    Assert-TowerLaunchNetwork $GameDir $Network
} catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }
