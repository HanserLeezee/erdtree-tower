param([string]$GameDir = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game', [switch]$CheckOnly, [ValidateSet('ME3','YAFSML')][string]$Loader='ME3')
$ErrorActionPreference = 'Stop'
$loaderSupport=Join-Path $PSScriptRoot 'Tower-Loader.ps1'
if(!(Test-Path -LiteralPath $loaderSupport)){$loaderSupport=Join-Path $PSScriptRoot 'distribution/Tower-Loader.ps1'}
. $loaderSupport
$exePath = Join-Path $GameDir 'eldenring.exe'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) { throw '找不到游戏，请传入 -GameDir' }
$exeVersion = (Get-Item -LiteralPath $exePath).VersionInfo.FileVersion
if ($exeVersion -notmatch '^2\.7\.2\.\d+$') { throw "验证包仅支持 1.17.2 系列游戏文件版本（2.7.2.x），当前为：$exeVersion" }
function Get-NativeDllName([string]$Root) {
    $profilePath = Join-Path $Root 'preview.me3'
    if (-not (Test-Path -LiteralPath $profilePath -PathType Leaf)) { return $null }
    $names = @([regex]::Matches([System.IO.File]::ReadAllText($profilePath), "path\s*=\s*['""](erdtree_tower_preview_v\d+\.dll)['""]") | ForEach-Object { $_.Groups[1].Value })
    if ($names.Count -ne 1) { return $null }
    return $names[0]
}
# Extra plugins (plugins/natives.toml, managed by the launcher)
# Each enabled [[natives]] section becomes an absolute path for ME3; disabled or missing plugins are skipped with a warning.
function Get-TowerPluginSections([string]$PackageRoot) {
    $file = Join-Path $PackageRoot 'plugins'
    $file = Join-Path $file 'natives.toml'
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { return @() }
    $sections = [regex]::Matches([IO.File]::ReadAllText($file), '(?ms)^\s*\[\[natives\]\][^\r\n]*\r?\n.*?(?=^\s*\[\[|\z)')
    foreach ($section in $sections) {
        $text = $section.Value
        if ($text -match '(?m)^\s*enabled\s*=\s*false\b') { continue }
        $match = [regex]::Match($text, '(?m)^(\s*path\s*=\s*)[''"]([^''"\r\n]+)[''"]')
        if (-not $match.Success) { Write-Warning 'plugins/natives.toml: a [[natives]] entry has no single-line quoted path, skipped.'; continue }
        $path = $match.Groups[2].Value.Replace('\', '/').Replace('/', [IO.Path]::DirectorySeparatorChar)
        if (-not [IO.Path]::IsPathRooted($path)) { $path = Join-Path $PackageRoot $path }
        $path = [IO.Path]::GetFullPath($path)
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Write-Warning "Plugin not found, skipped: $path"; continue }
        if ($path.Contains("'")) { Write-Warning "Plugin path contains a quote, skipped: $path"; continue }
        $text = $text.Substring(0, $match.Index) + $match.Groups[1].Value + "'" + $path.Replace('\', '/') + "'" + $text.Substring($match.Index + $match.Length)
        # enabled is the launcher's switch, not an ME3 option
        $text = [regex]::Replace($text, '(?m)^\s*enabled\s*=.*(\r?\n|\z)', '')
        $text.TrimEnd()
    }
}
# End extra plugins
$packageRoot = $null
$dllName = $null
$candidates = @($PSScriptRoot, (Join-Path $PSScriptRoot 'release-preview'))
foreach ($candidate in $candidates) {
    if (-not [IO.File]::Exists([IO.Path]::Combine($candidate, 'scene-assets\reward-art\rare-blue.rgba'))) { continue }
    if (-not [IO.File]::Exists([IO.Path]::Combine($candidate, 'scene-assets\reward-art\rarity-cards.rgba'))) { continue }
    $candidateDll = Get-NativeDllName $candidate
    if (-not $candidateDll) { continue }
    $requiredFiles = New-Object 'System.Collections.Generic.List[string]'
    foreach ($item in @('loader\bin\me3.exe', $candidateDll, 'preview.me3', 'scene-assets\regulation.bin', 'scene-assets\msg\zhocn\menu_dlc02.msgbnd.dcx', 'scene-assets\msg\zhocn\item_dlc02.msgbnd.dcx', 'scene-assets\script\talk\m00_00_00_00.talkesdbnd.dcx', 'scene-assets\event\m11_10_00_00.emevd.dcx', 'scene-assets\enemy-pools.txt', 'scene-assets\oath-art\cards.rgba', 'scene-assets\oath-art\background.rgba', 'scene-assets\oath-art\button.rgba', 'scene-assets\map\mapstudio\m11_10_00_00.msb.dcx', 'scene-assets\battlefields.txt', 'scene-assets\reward-details.tsv', 'scene-assets\reward-art\card.rgba', 'scene-assets\map-art\paper.rgba', 'scene-assets\map-art\icons.rgba')) {
        [void]$requiredFiles.Add($item)
    }
    foreach ($map in @('m45_00_00_00', 'm45_01_00_00', 'm45_02_00_00')) {
        [void]$requiredFiles.Add("scene-assets\map\mapstudio\$map.msb.dcx")
        [void]$requiredFiles.Add("scene-assets\event\$map.emevd.dcx")
        [void]$requiredFiles.Add("scene-assets\script\talk\$map.talkesdbnd.dcx")
    }
    foreach ($map in @('m10_00_00_00', 'm12_01_00_00', 'm11_00_00_00')) {
        [void]$requiredFiles.Add("scene-assets\map\mapstudio\$map.msb.dcx")
        [void]$requiredFiles.Add("scene-assets\event\$map.emevd.dcx")
    }
    $generatedIconIds = [System.Collections.Generic.HashSet[string]]::new()
    $modeFile = [IO.Path]::Combine($candidate, 'asset-mode.txt')
    $assetManifest = [IO.Path]::Combine($candidate, 'asset-patches\manifest.json')
    if ([IO.File]::Exists($modeFile) -and [IO.File]::ReadAllText($modeFile).Trim() -eq 'local-v1' -and [IO.File]::Exists($assetManifest)) {
        $manifestText = [IO.File]::ReadAllText($assetManifest)
        $iconsMatch = [regex]::Match($manifestText, '"Icons"\s*:\s*\[(.*?)\]', [Text.RegularExpressions.RegexOptions]::Singleline)
        if ($iconsMatch.Success) {
            foreach ($m in [regex]::Matches($iconsMatch.Groups[1].Value, '"Id"\s*:\s*(\d+)')) {
                [void]$generatedIconIds.Add($m.Groups[1].Value)
            }
        }
        if ($generatedIconIds.Count -gt 0) {
            [void]$requiredFiles.Add('Tower.AssetRestore.exe')
            [void]$requiredFiles.Add('Tower.IconCodec.dll')
        }
    }
    $allPresent = $true
    foreach ($rel in $requiredFiles) {
        if (-not [IO.File]::Exists([IO.Path]::Combine($candidate, $rel))) { $allPresent = $false; break }
    }
    if (-not $allPresent) { continue }
    $detailsFile = [IO.Path]::Combine($candidate, 'scene-assets\reward-details.tsv')
    $iconDir = [IO.Path]::Combine($candidate, 'scene-assets\reward-icons')
    $existingIcons = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    if ([IO.Directory]::Exists($iconDir)) {
        foreach ($iconPath in [IO.Directory]::GetFiles($iconDir, '*.rgba')) {
            [void]$existingIcons.Add([IO.Path]::GetFileNameWithoutExtension($iconPath))
        }
    }
    $checkedIds = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($line in [IO.File]::ReadLines($detailsFile, [Text.UTF8Encoding]::new($false))) {
        $fields = $line.Split([char]9)
        if ($fields.Length -eq 7) {
            $iconId = $fields[1]
            if ($checkedIds.Add($iconId) -and -not $generatedIconIds.Contains($iconId) -and -not $existingIcons.Contains($iconId)) {
                $allPresent = $false; break
            }
        }
    }
    if ($allPresent) { $packageRoot = $candidate; $dllName = $candidateDll; break }
}
if (-not $packageRoot) { throw "找不到完整验证包，已检查目录：$($candidates -join '；')。请完整解压验证包。" }
$loaderPath = Get-TowerLoaderPath $packageRoot $Loader
$dll = Join-Path $packageRoot $dllName
$profile = Join-Path $packageRoot 'preview.me3'
if ($CheckOnly) {
    [pscustomobject]@{ PackageDirectory = $packageRoot; LoaderPath = $loaderPath; DllPath = $dll; ProfilePath = $profile; Loader = $Loader }
    return
}
if (Get-Process -Name eldenring -ErrorAction SilentlyContinue) { throw '请先退出正在运行的艾尔登法环' }
$assetMode = Join-Path $packageRoot 'asset-mode.txt'
if ((Test-Path -LiteralPath $assetMode) -and [IO.File]::ReadAllText($assetMode).Trim() -notin @('full', 'local-v1')) { throw '资源包模式损坏，请校验并修复文件。' }
if (-not (Test-Path -LiteralPath $assetMode) -and (Test-Path -LiteralPath (Join-Path $packageRoot 'asset-patches\manifest.json'))) { throw '缺少资源包模式，请校验并修复文件。' }
if ((Test-Path -LiteralPath $assetMode) -and [IO.File]::ReadAllText($assetMode).Trim() -eq 'local-v1') {
    if (-not (Test-Path -LiteralPath (Join-Path $packageRoot 'asset-patches\manifest.json'))) { throw '缺少资源补丁清单，请校验并修复文件。' }
    $restore = Join-Path $packageRoot 'Tower.AssetRestore.exe'
    if (-not (Test-Path -LiteralPath $restore)) { throw '缺少本地资源生成器，请校验并修复文件。' }
    & $restore $packageRoot $GameDir
    if ($LASTEXITCODE -ne 0) { throw '本地资源生成失败，请查看日志或使用完整整合包。' }
}
$previousMarker = $env:ERDTREE_TOWER_PREVIEW
$previousLogDir = $env:ERDTREE_TOWER_LOG_DIR
$previousCompat = $env:__COMPAT_LAYER
try {
    $env:ERDTREE_TOWER_PREVIEW = '1'
    $env:ERDTREE_TOWER_LOG_DIR = $packageRoot
    Remove-Item -LiteralPath 'Env:__COMPAT_LAYER' -ErrorAction SilentlyContinue
    $logPath = Join-Path $packageRoot 'tower-preview.log'
    [System.IO.File]::WriteAllText($logPath, "Tower preview launch: $(Get-Date -Format o)`r`n")
    Write-Host "预览初始化日志：$logPath"
    $plugins = @(Get-TowerPluginSections $packageRoot)
    if ($plugins.Count -gt 0) {
        # The shipped preview.me3 stays untouched (updates replace it); plugins go into a generated launch profile next to it
        $launchProfile = Join-Path $packageRoot 'preview.launch.me3'
        $text = [System.IO.File]::ReadAllText($profile).TrimEnd() + "`r`n`r`n" + ($plugins -join "`r`n`r`n") + "`r`n"
        [System.IO.File]::WriteAllText($launchProfile, $text, (New-Object System.Text.UTF8Encoding($false)))
        $profile = $launchProfile
        Write-Host "额外 DLL 插件：$($plugins.Count) 个"
    }
    Invoke-TowerLoader $packageRoot $Loader $exePath $profile
} finally {
    $env:ERDTREE_TOWER_PREVIEW = $previousMarker
    $env:ERDTREE_TOWER_LOG_DIR = $previousLogDir
    if ($null -eq $previousCompat) { Remove-Item -LiteralPath 'Env:__COMPAT_LAYER' -ErrorAction SilentlyContinue }
    else { $env:__COMPAT_LAYER = $previousCompat }
}
