param([ValidateSet('Install','Restore')][string]$Action = 'Install', [string]$GameDir)
$ErrorActionPreference = 'Stop'
try {
    foreach ($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')) {
        Import-Module ([IO.Path]::Combine($PSHOME,'Modules',$module,($module + '.psd1'))) -ErrorAction Stop
    }
    if (Get-Process eldenring,ersc_launcher,ErdtreeTowerLauncher -ErrorAction SilentlyContinue) { throw 'Close the game and launcher before applying or restoring files.' }
    if (!$GameDir) {
        Add-Type -AssemblyName System.Windows.Forms
        $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
        $dialog.Description = 'Select the GAME folder containing eldenring.exe (not the mod launcher folder).'
        try {
            if ($dialog.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { return }
            $GameDir = $dialog.SelectedPath
        } finally { $dialog.Dispose() }
    }
    $GameDir = [IO.Path]::GetFullPath($GameDir.Trim().Trim('"'))
    if (![IO.File]::Exists((Join-Path $GameDir 'eldenring.exe'))) { throw 'Select the Game folder containing eldenring.exe.' }
    . (Join-Path $PSScriptRoot 'Tower-Network.ps1')
    . (Join-Path $PSScriptRoot 'Network-Files.ps1')
    $store = Join-Path $GameDir '.tower-network'
    if ($Action -eq 'Install') {
        $recovery = Join-Path $store 'restore-tools'
        $null = [IO.Directory]::CreateDirectory($recovery)
        foreach ($name in @('Network-Manual.ps1','Network-Files.ps1','Tower-Network.ps1')) {
            $source = Join-Path $PSScriptRoot $name
            $target = Join-Path $recovery $name
            if ([IO.Path]::GetFullPath($source) -ine [IO.Path]::GetFullPath($target)) { [IO.File]::Copy($source, $target, $true) }
        }
        $restore = '@echo off' + "`r`n" + '"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0restore-tools\Network-Manual.ps1" -Action Restore -GameDir "%~dp0.."' + "`r`npause`r`n"
        [IO.File]::WriteAllText((Join-Path $store 'Restore-A.cmd'), $restore, [Text.Encoding]::ASCII)
        Set-TowerNetwork $GameDir 'OnlineFix' (Join-Path $PSScriptRoot 'payload')
        Write-Host 'B installed. Original files and standalone Restore-A.cmd are in the game folder: .tower-network'
    } else {
        if (![IO.File]::Exists((Join-Path $store 'state.json')) -and ![IO.File]::Exists((Join-Path $store 'pending.json'))) {
            throw 'No original-file backup record found. Nothing was replaced. Keep the backup folder; do not copy DLLs from another user.'
        }
        Set-TowerNetwork $GameDir 'Steam' ''
        Write-Host 'Original files restored. Backups retained.'
    }
} catch { Write-Error $_.Exception.Message; exit 1 }
