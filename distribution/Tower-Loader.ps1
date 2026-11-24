function Get-TowerLoaderPath([string]$Root,[string]$Loader) {
    if($Loader -eq 'ME3'){$path=Join-Path $Root 'loader/bin/me3.exe'}
    elseif($Loader -eq 'YAFSML'){
        $path=Join-Path $Root 'loader/yafsml/YAFSML.exe'
        $hashes=@{'YAFSML.exe'='A7F1447C8DB43905C3EAF8653F5306DBB68A7418AFF0606335F7F0D6DE7B5437';'YAFSML.dll'='1A3EA05B0D3122BCA170AE80F8FCEC10659539939BC85B89BEE4A65DFB7DEBE4'}
        foreach($name in $hashes.Keys){
            $file=Join-Path "$Root/loader/yafsml" $name
            if(!(Test-Path -LiteralPath $file -PathType Leaf)){throw "Missing $name; update the complete launcher package."}
            if((Get-FileHash -LiteralPath $file).Hash -ne $hashes[$name]){throw "Unsupported $name; expected bundled YAFSML 0.11.0."}
        }
    }else{throw 'Unsupported loader'}
    if(!(Test-Path -LiteralPath $path -PathType Leaf)){throw "Missing loader: $path"}
    return $path
}
function Add-TowerLanguagePackage([string]$Text,[string]$Root,[string]$GameDirectory) {
    $language='auto'
    $options=Join-Path $Root 'tower-options.ini'
    if(Test-Path -LiteralPath $options -PathType Leaf){
        foreach($line in [IO.File]::ReadAllLines($options)){
            if($line -match '^\s*language\s*=\s*([^#;\s]+)\s*(?:[#;].*)?$'){$language=$Matches[1].ToLowerInvariant()}
        }
    }
    if($language -in @('auto','game')){
        $steamApps=Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $GameDirectory))
        if($steamApps){
            $manifest=Join-Path $steamApps 'appmanifest_1245620.acf'
            if(Test-Path -LiteralPath $manifest -PathType Leaf){
                $acf=[IO.File]::ReadAllText($manifest)
                if($acf -match '(?is)"UserConfig"\s*\{[^}]*"language"\s*"([^"]+)"'){$language=$Matches[1].ToLowerInvariant()}
            }
        }
    }
    $sections='(?ms)^\s*\[\[packages\]\][^\r\n]*\r?\n.*?(?=^\s*\[\[|\z)'
    $Text=[regex]::Replace($Text,$sections,{param($match)
        if($match.Value -match '(?m)^\s*id\s*=\s*[''"]tower-language[''"]\s*$'){return ''}
        return $match.Value
    }).TrimEnd()
    if($language -notin @('en','en-us','engus','english')){return $Text}
    $overlay=[IO.Path]::GetFullPath((Join-Path $Root 'scene-assets/locales/en-US'))
    if(!(Test-Path -LiteralPath (Join-Path $overlay 'menu') -PathType Container)){throw 'English menu resources are missing. Verify and repair the installation.'}
    if($overlay.Contains("'")){throw 'Unsupported quote in language asset path.'}
    return $Text+"`r`n[[packages]]`r`nid = 'tower-language'`r`npath = '$($overlay.Replace('\','/'))'`r`n"
}
function Read-TowerProfileString([string]$Value) {
    if($Value -match "^'([^'\r\n]*)'\s*(?:#.*)?$"){return $Matches[1]}
    if($Value -match '^("(?:[^"\\]|\\.)*")\s*(?:#.*)?$'){return ($Matches[1] | ConvertFrom-Json)}
    throw 'YAFSML requires single-line quoted profile values; use ME3 for advanced TOML profiles.'
}
function ConvertTo-TowerYafsmlConfig([string]$Text,[string]$BaseDirectory) {
    $sections=New-Object System.Collections.ArrayList
    $current=@{kind='root';values=@{}};[void]$sections.Add($current)
    foreach($raw in ($Text -split '\r?\n')){
        $line=$raw.Trim();if(!$line -or $line.StartsWith('#')){continue}
        if($line -match '^\[\[(natives|packages|supports)\]\]\s*(?:#.*)?$'){
            $current=@{kind=$Matches[1];values=@{}};[void]$sections.Add($current);continue
        }
        if($line -notmatch '^(\w+)\s*=\s*(.+)$'){throw 'Unsupported YAFSML profile syntax; select ME3 for advanced profiles.'}
        $key=$Matches[1];$value=$Matches[2]
        $allowed=switch($current.kind){root {@('profileVersion','start_online','savefile')} natives {@('path','id','enabled','load_after','load_before')} packages {@('path','id','enabled')} supports {@('game')}}
        if($key -notin $allowed -or $current.values.ContainsKey($key)){throw "Unsupported or duplicate $key in $($current.kind); select ME3 for this profile."}
        $current.values[$key]=$value
    }
    $header=$sections[0].values
    if((Read-TowerProfileString $header.profileVersion) -ne 'v1'){throw 'Unsupported profile version'}
    if($header.ContainsKey('start_online') -and $header.start_online -notmatch '^false\s*(?:#.*)?$'){throw 'YAFSML tower launch requires offline mode'}
    $save=if($header.ContainsKey('savefile')){Read-TowerProfileString $header.savefile}else{''}
    if($save -match '[\\/:|;#=\r\n]'){throw 'Unsupported save filename'}
    $natives=New-Object System.Collections.ArrayList;$packages=New-Object System.Collections.ArrayList
    foreach($section in $sections | Select-Object -Skip 1){
        $v=$section.values
        if($section.kind -eq 'supports'){
            if((Read-TowerProfileString $v.game) -ne 'eldenring'){throw 'Only Elden Ring profiles are supported'};continue
        }
        if($v.ContainsKey('enabled')){
            if($v.enabled -match '^false\s*(?:#.*)?$'){continue}
            if($v.enabled -notmatch '^true\s*(?:#.*)?$'){throw 'Invalid enabled flag'}
        }
        $path=Read-TowerProfileString $v.path
        if($path -match '[|;#=\r\n]'){throw 'Unsupported INI path characters'}
        if(![IO.Path]::IsPathRooted($path)){$path=Join-Path $BaseDirectory $path}
        $path=[IO.Path]::GetFullPath($path).Replace('\','/')
        if(!(Test-Path -LiteralPath $path)){throw "Missing enabled profile path: $path"}
        if($section.kind -eq 'packages'){
            if(!(Test-Path -LiteralPath $path -PathType Container)){throw 'YAFSML tower packages must be directories'}
            [void]$packages.Add($path);continue
        }
        if(!(Test-Path -LiteralPath $path -PathType Leaf)){throw 'Native path must be a file'}
        $id=if($v.ContainsKey('id')){Read-TowerProfileString $v.id}else{[IO.Path]::GetFileName($path)}
        if(@($natives | Where-Object {$_.id -eq $id}).Count){throw "Duplicate native ID: $id"}
        [void]$natives.Add(@{id=$id;path=$path;values=$v;after=(New-Object System.Collections.ArrayList)})
    }
    if($packages.Count -eq 0){throw 'YAFSML requires at least one tower asset package.'}
    foreach($native in $natives){foreach($direction in 'load_after','load_before'){
        if(!$native.values.ContainsKey($direction)){continue}
        $dependencies=$native.values[$direction] -replace '\s+#.*$',''
        if($dependencies -notmatch '^\[.*\]$'){throw 'Dependencies must be inline arrays'}
        $body=$dependencies.Substring(1,$dependencies.Length-2)
        $matchesList=[regex]::Matches($body,'\{([^{}]+)\}')
        if(([regex]::Replace($body,'\{[^{}]+\}','') -replace '[,\s]','').Length){throw 'Unsupported dependency syntax'}
        foreach($match in $matchesList){
            $fields=@{}
            foreach($field in ($match.Groups[1].Value -split ',')){
                if($field.Trim() -notmatch '^(id|optional)\s*=\s*(.+)$' -or $fields.ContainsKey($Matches[1])){throw 'Unsupported dependency field'}
                $fields[$Matches[1]]=$Matches[2]
            }
            $id=Read-TowerProfileString $fields.id
            if($fields.ContainsKey('optional') -and $fields.optional -notin @('true','false')){throw 'Invalid optional dependency flag'}
            $target=@($natives | Where-Object {$_.id -eq $id})
            if(!$target.Count){if($fields.optional -eq 'true'){continue};throw "Missing required native dependency: $id"}
            if($direction -eq 'load_after'){[void]$native.after.Add($id)}else{[void]$target[0].after.Add($native.id)}
        }
    }}
    $ordered=New-Object System.Collections.ArrayList;$remaining=@($natives)
    while($remaining.Count){
        $next=$remaining | Where-Object { $unmet=@($_.after | Where-Object {$_ -notin @($ordered | ForEach-Object {$_.id})});$unmet.Count -eq 0 } | Select-Object -First 1
        if($null -eq $next){throw 'Circular native dependencies'}
        [void]$ordered.Add($next);$remaining=@($remaining | Where-Object {$_.id -ne $next.id})
    }
    $lines=@('game=eldenring','','[patch]','skip_intro=1','prevent_regulation_save_write=1','patch_mem=1','patch_mem_dedicated_heap=1','patch_mem_heap_size=0','boot_boost=1','disable_arxan=0',"replace_save_filename=$save",'replace_seamless_coop_save_filename=','enable_ime=0','','[tweak]','cpu_affinity=0','','[log]','console=0','log_file=1','log_level=info','','[dll]')
    for($i=0;$i -lt $ordered.Count;$i++){
        $line="native$i=$($ordered[$i].path)"
        if($i -gt 0){$line+='|after,native'+($i-1)}
        $lines+=$line
    }
    $lines+=@('','[mod]',('tower='+$packages[0]))
    for($i=1;$i -lt $packages.Count;$i++){$lines+='overlay'+$i+'='+$packages[$i]}
    $lines+=''
    return ($lines -join "`n")
}
function Get-TowerGameDirDiagnostics([string]$GameDir) {
    $vanillaDlls = @(
        'amd_ags_x64.dll','bink2w64.dll','eossdk-win64-shipping.dll','libxess.dll',
        'nvngx_dlss.dll','oo2core_6_win64.dll','steam_api64.dll','tobii_gameintegration_x64.dll'
    )
    $highRiskProxies = @(
        'dinput8.dll','dxgi.dll','winhttp.dll','version.dll','dsound.dll','xaudio2_9.dll',
        'mmdevapi.dll','d3d11.dll','d3d12.dll','opengl32.dll','msacm32.dll','winmm.dll'
    )
    $unexpected = New-Object System.Collections.ArrayList
    $proxies = New-Object System.Collections.ArrayList
    $steamAppIdPresent = $false
    if (Test-Path -LiteralPath $GameDir -PathType Container) {
        $steamAppIdPresent = Test-Path -LiteralPath (Join-Path $GameDir 'steam_appid.txt') -PathType Leaf
        foreach ($file in Get-ChildItem -LiteralPath $GameDir -File -ErrorAction SilentlyContinue) {
            $lower = $file.Name.ToLowerInvariant()
            $ext = $file.Extension.ToLowerInvariant()
            if ($ext -in @('.dll','.asi') -or $lower -match '^(modengine|reshade|specialk|eldenmodloader)') {
                if ($lower -notin $vanillaDlls) {
                    $entry = "$($file.Name) ($($file.Length) B)"
                    [void]$unexpected.Add($entry)
                    if ($lower -in $highRiskProxies -or $lower -match '^(modengine|reshade|specialk)') {
                        [void]$proxies.Add($entry)
                    }
                }
            }
        }
    }
    $sdDir = Join-Path $GameDir 'sd'
    $sdCount = if (Test-Path -LiteralPath $sdDir -PathType Container) { @(Get-ChildItem -LiteralPath $sdDir -File -ErrorAction SilentlyContinue).Count } else { -1 }
    return [pscustomobject]@{
        UnexpectedFiles = @($unexpected)
        HighRiskProxies = @($proxies)
        SteamAppIdPresent = $steamAppIdPresent
        SdFileCount = $sdCount
    }
}
function Write-TowerLaunchDiagnostics([string]$GameExe,[string]$Loader) {
    try {
        $gameDir = Split-Path -Parent $GameExe
        $diag = Get-TowerGameDirDiagnostics $gameDir
        $exeVer = try { (Get-Item -LiteralPath $GameExe -ErrorAction Stop).VersionInfo.FileVersion } catch { 'unknown' }
        $isAdmin = $false
        try {
            $id = [Security.Principal.WindowsIdentity]::GetCurrent()
            $isAdmin = ([Security.Principal.WindowsPrincipal]::new($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        } catch {}
        Write-Host ("DIAG_ENV loader=$Loader exe_ver=$exeVer admin=$isAdmin compat_env=" + $(if ($env:__COMPAT_LAYER) {$env:__COMPAT_LAYER} else {'none'}) + " steam_appid_txt=$($diag.SteamAppIdPresent) sd_files=$($diag.SdFileCount)")
        if ($diag.HighRiskProxies.Count -gt 0) {
            Write-Host ("DIAG_WARN high_risk_proxy_dlls_in_game_dir=" + ($diag.HighRiskProxies -join ', '))
        }
        if ($diag.UnexpectedFiles.Count -gt 0) {
            Write-Host ("DIAG_GAME_EXTRA_FILES=" + ($diag.UnexpectedFiles -join ', '))
        } else {
            Write-Host 'DIAG_GAME_EXTRA_FILES=none'
        }
        foreach ($hive in @('HKCU:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers','HKLM:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers')) {
            if (Test-Path -LiteralPath $hive) {
                $props = (Get-ItemProperty -LiteralPath $hive -ErrorAction SilentlyContinue).PSObject.Properties | Where-Object {
                    $_.Name -notlike 'PS*' -and $_.Name -match '(eldenring|start_protected_game|me3|yafsml)\.exe$'
                }
                foreach ($p in $props) { Write-Host ("DIAG_WARN appcompat [$hive] $($p.Name) = $($p.Value)") }
            }
        }
        $ifeo = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\eldenring.exe'
        if (Test-Path -LiteralPath $ifeo) {
            $vals = (Get-ItemProperty -LiteralPath $ifeo -ErrorAction SilentlyContinue).PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | ForEach-Object { "$($_.Name)=$($_.Value)" }
            if ($vals) { Write-Host ("DIAG_WARN ifeo_eldenring=" + ($vals -join '; ')) }
        }
        $watchPattern = '^(Nahimic|SS3Svc|SonicStudio|SteelSeries|Sonar|Razer|RzSynapse|THX|lghub|voicemeeter|RTSS|MSIAfterburner|GamePP|Discord|obs64|vgtray|SGuard|ACE|360|HipsDaemon)'
        $activeHooks = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match $watchPattern } | Select-Object -ExpandProperty ProcessName -Unique | Sort-Object)
        if ($activeHooks.Count -gt 0) {
            Write-Host ("DIAG_WATCH_PROCESSES=" + ($activeHooks -join ', '))
        } else {
            Write-Host 'DIAG_WATCH_PROCESSES=none'
        }
    } catch {
        Write-Host ("DIAG_SKIPPED=" + $_.Exception.Message)
    }
}
function Invoke-TowerLoader([string]$Root,[string]$Loader,[string]$GameExe,[string]$Profile) {
    $exe=Get-TowerLoaderPath $Root $Loader
    Write-Host "Tower loader: $Loader"
    Write-TowerLaunchDiagnostics $GameExe $Loader
    Write-Output "TOWER_PROGRESS|starting|0|0|正在启动游戏|$Loader 加载器正在准备游戏进程"
    if($Loader -eq 'ME3'){
        # 有的电脑上 me3 必须同时满足两点，否则游戏在 hooking system allocator 时 0xC0000005：
        # 关闭 Arxan（与测试入口 Launch-Tower.ps1 相同，ERDTREE_TOWER_LOADER_STABILITY=0 可关），
        # 以及有自己的控制台——启动器用无窗口、重定向输出的 PowerShell 拉起脚本，直接 & 调用会让 me3 继承这个环境
        $stability = if ($env:ERDTREE_TOWER_LOADER_STABILITY -ne '0') { @('--disable-arxan','true') } else { @() }
        $meArguments = @('launch','--exe',('"'+$GameExe+'"'),'--profile',('"'+$Profile+'"'),'--auto-detect','--online','false','--skip-steam-init','true') + $stability
        Write-Host ("ME3 arguments: " + ($meArguments -join ' '))
        $process = Start-Process -FilePath $exe -ArgumentList $meArguments -PassThru -WindowStyle Hidden -WorkingDirectory (Split-Path -Parent $exe)
        $handle = $process.Handle
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {throw "ME3 loader exited with code $($process.ExitCode)"}
        return
    }else{
        $ini=ConvertTo-TowerYafsmlConfig ([IO.File]::ReadAllText($Profile)) (Split-Path -Parent $Profile)
        $config=Join-Path (Split-Path -Parent $Profile) 'yafsml.launch.ini'
        $utf8=[Text.UTF8Encoding]::new($false)
        if(![IO.File]::Exists($config) -or [IO.File]::ReadAllText($config,$utf8) -cne $ini){[IO.File]::WriteAllText($config,$ini,$utf8)}
        $loaderArguments = @('--launch-target','eldenring','--game-path',('"'+$GameExe+'"'),'--config',('"'+$config+'"'),'--modloader-dll',('"'+(Join-Path $Root 'loader/yafsml/YAFSML.dll')+'"'))
        Write-Host ("YAFSML arguments: " + ($loaderArguments -join ' '))
        $process = Start-Process -FilePath $exe -ArgumentList $loaderArguments -PassThru -WindowStyle Hidden -WorkingDirectory (Split-Path -Parent $GameExe)
        $handle = $process.Handle
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {throw "YAFSML loader exited with code $($process.ExitCode)"}
        return
    }
    if($LASTEXITCODE -ne 0){throw "$Loader loader exited with code $LASTEXITCODE"}
}
