param(
    [ValidateSet('Solo', 'Seamless')][string]$Mode = 'Seamless',
    [ValidateSet('ME3', 'YAFSML')][string]$Loader = 'ME3',
    [ValidateSet('Auto', 'Steam', 'OnlineFix')][string]$Network = 'Auto',
    [string]$GameDir,
    [switch]$CheckOnly,
    [string]$PartyConfig,
    # Launcher use: write the failure message here so it can be shown without a console window
    [string]$ErrorFile
)
$ErrorActionPreference = 'Stop'
trap {
    if ($ErrorFile) { [IO.File]::WriteAllText($ErrorFile, $_.Exception.Message, (New-Object Text.UTF8Encoding($false))) }
    exit 1
}
foreach ($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')) {
    Import-Module ([IO.Path]::Combine($PSHOME,'Modules',$module,($module + '.psd1'))) -ErrorAction Stop
}
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$pathFile = Join-Path $PSScriptRoot 'GamePath.txt'
if (-not $GameDir -and (Test-Path -LiteralPath $pathFile)) {
    $GameDir = [IO.File]::ReadAllText($pathFile).Trim()
}
if (-not $GameDir) {
    if ($CheckOnly) { throw 'Specify -GameDir or create GamePath.txt first.' }
    $GameDir = Read-Host 'Paste the Game folder path containing eldenring.exe'
}
$GameDir = $GameDir.Trim().Trim('"')
if (-not (Test-Path -LiteralPath (Join-Path $GameDir 'eldenring.exe') -PathType Leaf)) {
    throw 'Invalid GameDir: select the Game folder containing eldenring.exe.'
}
$GameDir = (Resolve-Path -LiteralPath $GameDir).Path
if ($Network -eq 'Auto') {
    . (Join-Path $PSScriptRoot 'Tower-Network.ps1')
    $Network = Get-TowerNetwork $GameDir
}
$launcher = 'Launch-Seamless-Tower.ps1'
$entry = Join-Path $PSScriptRoot $launcher
$options = @{GameDir=$GameDir;Loader=$Loader;Network=$Network}
if ($PartyConfig) {
    $options.PartyConfig=$PartyConfig
}
$result = & $entry @options -CheckOnly
if ($CheckOnly) { $result; return }
$launchGuard=$null
try {
    try {
        $launchGuard=[IO.File]::Open((Join-Path $PSScriptRoot '.tower-launch.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
    } catch [IO.IOException] {
        if (($_.Exception.HResult -band 0xffff) -in @(32,33)) { throw '已有启动任务正在运行，请等待资源准备或游戏启动完成，勿重复启动。' }
        throw
    }
    [IO.File]::WriteAllText($pathFile, $GameDir, (New-Object Text.UTF8Encoding($true)))
    & $entry @options
} finally {
    if($launchGuard){$launchGuard.Dispose()}
}
