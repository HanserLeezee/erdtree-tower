param(
    [ValidateSet('Menu','Install','Restore')][string]$Action = 'Menu',
    [string]$GameDir
)
$ErrorActionPreference = 'Stop'

function Import-TowerManualModules {
    foreach ($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')) {
        $psd1 = [IO.Path]::Combine($PSHOME, 'Modules', $module, ($module + '.psd1'))
        if ([IO.File]::Exists($psd1)) {
            Import-Module $psd1 -ErrorAction Stop
        }
    }
}

function Format-TowerNetworkError([string]$Message) {
    $cause = '切换过程中遇到未识别的错误。'
    $fix = '请截图完整窗口内容反馈给作者，不要手动删除游戏目录里的 .tower-network 文件夹。'

    if ($Message -match 'Close the game and launcher|Exit the game|仍在运行') {
        $cause = '游戏或爬塔启动器还没有完全关闭。'
        $fix = '请先彻底关闭《艾尔登法环》、爬塔启动器和 Steam 里的“正在运行”状态；必要时打开任务管理器结束 eldenring.exe / ersc_launcher.exe / ErdtreeTowerLauncher.exe 后重试。'
    } elseif ($Message -match 'Select the Game (folder|directory) containing eldenring\.exe|未找到 eldenring\.exe') {
        $cause = '选中的目录里没有找到 eldenring.exe。'
        $fix = '请选择游戏本体里的 Game 文件夹（例如 D:\SteamLibrary\steamapps\common\ELDEN RING\Game），不要选成启动器文件夹或上一层的 ELDEN RING 文件夹。'
    } elseif ($Message -match 'No original-file backup record found|没有本机原版备份') {
        $cause = '当前游戏目录没有本机原版文件的备份记录（.tower-network\state.json），无法自动还原。'
        $fix = '如果你之前手动覆盖过联机补丁或删掉了 .tower-network，请在 Steam 中右键艾尔登法环 → 属性 → 已安装文件 → “验证游戏文件的完整性”恢复原版文件，并手动清理残留的 OnlineFix64.dll / OnlineFix.ini / winmm.dll / dlllist.txt。'
    } elseif ($Message -match 'Selected profile does not match|已有 OnlineFix 文件但未记录在备份中') {
        $cause = '游戏目录里已经存在未通过本工具安装的 OnlineFix 文件（OnlineFix64.dll 或 OnlineFix.ini），且没有原版备份。'
        $fix = '为防止把你的原版 steam_api64.dll 彻底弄丢，本工具拒绝直接覆盖。请先通过 Steam “验证游戏文件的完整性”恢复纯净原版，并删除 OnlineFix64.dll / OnlineFix.ini / winmm.dll / dlllist.txt 后再运行本工具切换。'
    } elseif ($Message -match 'Mixed Steam API proxies|steam_api64_o\.dll') {
        $cause = '检测到游戏目录中混装了其他 Steam 代理文件（steam_api64_o.dll）。'
        $fix = '请先卸载/清理旧的破解补丁或第三方联机补丁（删除 steam_api64_o.dll），并在 Steam 验证游戏文件完整性后再使用本工具切换。'
    } elseif ($Message -match 'Missing original steam_api64\.dll') {
        $cause = '游戏目录缺少原版 steam_api64.dll，无法在切 B 前完成原版备份。'
        $fix = '请先在 Steam 中“验证游戏文件的完整性”补回原版 steam_api64.dll，再重新运行本工具。'
    } elseif ($Message -match 'Interrupted|pending\.json') {
        $cause = '上一次 A/B 切换中途被强行关闭或中断，文件正处于半完成状态。'
        $fix = '请在本工具中选择 [A] 还原为原版 Steam，工具会自动按备份回滚修复；修复完成后再按需重新切换。'
    } elseif ($Message -match 'Unsupported OnlineFix component|Missing (OnlineFix64\.dll|steam_api64\.dll|winmm\.dll|OnlineFix\.ini)|payload') {
        $cause = 'B 补丁核心文件缺失或内容不匹配（常见原因是压缩包未完整解压，或被杀毒软件 / Windows Defender 误杀隔离）。'
        $fix = '请检查 Windows 安全中心 / 杀毒软件的隔离区，放行并恢复 payload 里的 OnlineFix64.dll、steam_api64.dll、winmm.dll，或将补丁压缩包重新完整解压到单独文件夹后再试。'
    } elseif ($Message -match 'Unsupported dlllist\.txt|B patch list is not ready') {
        $cause = '游戏目录里的 dlllist.txt 内容与爬塔加载器不匹配（可能被其他工具改动过或带了多余换行）。'
        $fix = '请在本工具中选择 [B] 重新应用一次，工具会自动修正 dlllist.txt。'
    } elseif ($Message -match 'Damaged A/B backup|Incomplete A/B backup|Invalid A/B backup|Invalid A/B state') {
        $cause = '游戏目录里的 .tower-network 原版备份已损坏或不完整，为避免覆盖出错已中止操作。'
        $fix = '请不要拿别人的备份混用；建议直接在 Steam 中“验证游戏文件的完整性”恢复纯净原版文件，并手动清理 OnlineFix64.dll / OnlineFix.ini / winmm.dll / dlllist.txt。'
    } elseif ($Message -match 'Linked patch file rejected') {
        $cause = '游戏目录中的某个补丁文件是符号链接/软链接，出于安全限制被拒绝。'
        $fix = '请移除游戏目录中指向外部的符号链接 DLL，恢复普通文件后重试。'
    } elseif ($Message -match 'being used by another process|正由另一进程使用|sharing violation') {
        $cause = '游戏目录中的 DLL 文件正被其他程序占用，无法替换。'
        $fix = '请关闭游戏、启动器、Steam 客户端及可能正在扫描目录的杀毒软件后重试。'
    } elseif ($Message -match 'Access to the path .* is denied|UnauthorizedAccess|拒绝访问') {
        $cause = '没有足够权限修改游戏目录中的文件（常见于游戏装在 C:\Program Files 下或文件被设为只读）。'
        $fix = '请右键「联机环境切换(A-B).cmd」选择“以管理员身份运行”，或检查游戏目录是否被设为只读 / 被安全软件拦截。'
    } elseif ($Message -match 'Unexpected OnlineFix') {
        $cause = 'OnlineFix.ini 中的 AppID 配置异常（要求 RealAppId=1245620 且 FakeAppId=480）。'
        $fix = '请不要手动修改 OnlineFix.ini，重新解压原始补丁包后再选择 [B] 覆盖修复。'
    }

    return @(
        ''
        '========================================================'
        '  【操作失败】'
        "  原因：$cause"
        "  解决：$fix"
        "  详情：$Message"
        '========================================================'
    ) -join "`r`n"
}

function Resolve-TowerGameDirInput([string]$RawInput) {
    if ([string]::IsNullOrWhiteSpace($RawInput)) { return $null }
    $clean = $RawInput.Trim().Trim('"').Trim("'")
    if ([string]::IsNullOrWhiteSpace($clean)) { return $null }
    try {
        $full = [IO.Path]::GetFullPath($clean)
    } catch {
        return $null
    }
    if ([IO.File]::Exists($full) -and ([IO.Path]::GetFileName($full) -ieq 'eldenring.exe')) {
        return [IO.Path]::GetDirectoryName($full)
    }
    if ([IO.Directory]::Exists($full)) {
        if ([IO.File]::Exists((Join-Path $full 'eldenring.exe'))) {
            return $full
        }
        $nested = Join-Path $full 'Game'
        if ([IO.File]::Exists((Join-Path $nested 'eldenring.exe'))) {
            return $nested
        }
    }
    return $null
}

function Find-TowerGameCandidates {
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $list = New-Object 'System.Collections.Generic.List[string]'
    $add = {
        param([string]$Candidate)
        $resolved = Resolve-TowerGameDirInput $Candidate
        if ($resolved -and $seen.Add($resolved)) { $list.Add($resolved) }
    }

    # 1. 上次本工具记住的路径 / 附近爬塔启动器的 GamePath.txt
    foreach ($hint in @(
        (Join-Path $PSScriptRoot '.last-game-dir.txt'),
        (Join-Path $PSScriptRoot 'GamePath.txt'),
        (Join-Path (Split-Path $PSScriptRoot -Parent) 'GamePath.txt')
    )) {
        if ([IO.File]::Exists($hint)) {
            try { & $add ([IO.File]::ReadAllText($hint)) } catch {}
        }
    }

    # 2. Steam 注册表与 libraryfolders.vdf 全部库目录
    $steamRoots = New-Object 'System.Collections.Generic.List[string]'
    foreach ($reg in @(
        @{ Path = 'HKCU:\Software\Valve\Steam'; Name = 'SteamPath' },
        @{ Path = 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam'; Name = 'InstallPath' },
        @{ Path = 'HKLM:\SOFTWARE\Valve\Steam'; Name = 'InstallPath' }
    )) {
        try {
            $val = (Get-ItemProperty -Path $reg.Path -Name $reg.Name -ErrorAction SilentlyContinue).($reg.Name)
            if ($val -and [IO.Directory]::Exists($val)) { $steamRoots.Add($val) }
        } catch {}
    }
    foreach ($root in $steamRoots) {
        & $add (Join-Path $root 'steamapps\common\ELDEN RING\Game')
        $vdf = Join-Path $root 'steamapps\libraryfolders.vdf'
        if ([IO.File]::Exists($vdf)) {
            try {
                $content = [IO.File]::ReadAllText($vdf)
                foreach ($m in [regex]::Matches($content, '"path"\s+"([^"]+)"')) {
                    $lib = $m.Groups[1].Value.Replace('\\', '\')
                    & $add (Join-Path $lib 'steamapps\common\ELDEN RING\Game')
                }
            } catch {}
        }
    }

    # 3. 常见盘符兜底扫描
    foreach ($drive in @('C','D','E','F','G','H')) {
        foreach ($sub in @(
            'SteamLibrary\steamapps\common\ELDEN RING\Game',
            'Steam\steamapps\common\ELDEN RING\Game',
            'Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game',
            'Games\ELDEN RING\Game',
            'ELDEN RING\Game'
        )) {
            $p = "${drive}:\$sub"
            if ([IO.Directory]::Exists($p)) { & $add $p }
        }
    }
    return @($list)
}

function Open-TowerGameFileDialog([string]$InitialDir) {
    Add-Type -AssemblyName System.Windows.Forms
    $dialog = New-Object System.Windows.Forms.OpenFileDialog
    $dialog.Title = '请选择艾尔登法环 Game 目录中的 eldenring.exe（顶部地址栏可直接粘贴路径）'
    $dialog.Filter = '艾尔登法环主程序 (eldenring.exe)|eldenring.exe|所有文件 (*.*)|*.*'
    $dialog.FileName = 'eldenring.exe'
    $dialog.CheckFileExists = $true
    $dialog.Multiselect = $false
    if ($InitialDir -and [IO.Directory]::Exists($InitialDir)) {
        $dialog.InitialDirectory = $InitialDir
    }
    try {
        if ($dialog.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { return $null }
        return Resolve-TowerGameDirInput $dialog.FileName
    } finally {
        $dialog.Dispose()
    }
}

function Select-TowerGameDirInteractive {
    $candidates = @(Find-TowerGameCandidates)
    while ($true) {
        Write-Host ''
        Write-Host '--------------------------------------------------------'
        Write-Host '第一步：选择《艾尔登法环》游戏目录（含 eldenring.exe 的 Game 文件夹）'
        Write-Host '--------------------------------------------------------'
        if ($candidates.Count -gt 0) {
            Write-Host '已自动检测到以下游戏目录：' -ForegroundColor Cyan
            for ($i = 0; $i -lt $candidates.Count; $i++) {
                Write-Host ("  [{0}] {1}" -f ($i + 1), $candidates[$i])
            }
            Write-Host ''
            Write-Host '你可以：'
            Write-Host '  · 直接按 回车：使用第 [1] 个目录'
            Write-Host '  · 输入编号（如 1 / 2）：选择对应目录'
            Write-Host '  · 直接粘贴路径，或把 Game 文件夹 / eldenring.exe 拖进本窗口后按回车'
            Write-Host '  · 输入 F：打开新版文件选择窗口（顶部地址栏可粘贴路径，选中 eldenring.exe 即可）'
            Write-Host '  · 输入 Q：退出'
        } else {
            Write-Host '未自动检测到游戏目录，你可以：' -ForegroundColor Yellow
            Write-Host '  · 直接粘贴 Game 目录路径，或把 Game 文件夹 / eldenring.exe 拖进本窗口后按回车'
            Write-Host '  · 输入 F：打开新版文件选择窗口（顶部地址栏可粘贴路径，选中 eldenring.exe 即可）'
            Write-Host '  · 输入 Q：退出'
        }
        Write-Host ''
        $raw = Read-Host '请输入选项或路径'
        if ($null -eq $raw) { return $null }
        $trimmed = $raw.Trim()

        if ([string]::IsNullOrEmpty($trimmed)) {
            if ($candidates.Count -gt 0) { return $candidates[0] }
            Write-Host '请粘贴路径或输入 F 打开文件选择窗口。' -ForegroundColor Yellow
            continue
        }
        if ($trimmed -ieq 'Q') { return $null }
        if ($trimmed -ieq 'F') {
            $init = if ($candidates.Count -gt 0) { $candidates[0] } else { '' }
            $picked = Open-TowerGameFileDialog $init
            if ($picked) { return $picked }
            Write-Host '未选择有效文件（需要选中 eldenring.exe），请重试或直接粘贴路径。' -ForegroundColor Yellow
            continue
        }
        if ($trimmed -match '^\d+$') {
            $idx = [int]$trimmed - 1
            if ($idx -ge 0 -and $idx -lt $candidates.Count) { return $candidates[$idx] }
        }
        $resolved = Resolve-TowerGameDirInput $trimmed
        if ($resolved) { return $resolved }
        Write-Host ''
        Write-Host "【路径无效】在「$trimmed」下没有找到 eldenring.exe。" -ForegroundColor Red
        Write-Host '提示：请粘贴包含 eldenring.exe 的 Game 文件夹路径（或直接粘贴 eldenring.exe 的完整路径），不要选成爬塔启动器目录。' -ForegroundColor Yellow
    }
}

function Save-TowerLastGameDir([string]$Dir) {
    try {
        [IO.File]::WriteAllText((Join-Path $PSScriptRoot '.last-game-dir.txt'), $Dir, (New-Object Text.UTF8Encoding($false)))
    } catch {}
}

function Assert-TowerProcessesClosed {
    $running = @(Get-Process eldenring,ersc_launcher,ErdtreeTowerLauncher -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) {
        $names = ($running | ForEach-Object { $_.ProcessName + '.exe' } | Select-Object -Unique) -join '、'
        throw "Close the game and launcher before applying or restoring files. (仍在运行：$names)"
    }
}

function Invoke-TowerInstallB([string]$TargetDir) {
    Assert-TowerProcessesClosed
    $store = Join-Path $TargetDir '.tower-network'
    $recovery = Join-Path $store 'restore-tools'
    $null = [IO.Directory]::CreateDirectory($recovery)
    foreach ($name in @('Network-Manual.ps1','Network-Files.ps1','Tower-Network.ps1')) {
        $source = Join-Path $PSScriptRoot $name
        $target = Join-Path $recovery $name
        if ([IO.Path]::GetFullPath($source) -ine [IO.Path]::GetFullPath($target)) {
            [IO.File]::Copy($source, $target, $true)
        }
    }
    $restore = '@echo off' + "`r`n" + 'chcp 65001 >nul' + "`r`n" + '"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0restore-tools\Network-Manual.ps1" -Action Restore -GameDir "%~dp0.."' + "`r`npause`r`n"
    [IO.File]::WriteAllText((Join-Path $store 'Restore-A.cmd'), $restore, [Text.Encoding]::ASCII)
    Set-TowerNetwork $TargetDir 'OnlineFix' (Join-Path $PSScriptRoot 'payload')
}

function Invoke-TowerRestoreA([string]$TargetDir) {
    Assert-TowerProcessesClosed
    $store = Join-Path $TargetDir '.tower-network'
    if (![IO.File]::Exists((Join-Path $store 'state.json')) -and ![IO.File]::Exists((Join-Path $store 'pending.json'))) {
        if ((Get-TowerNetwork $TargetDir) -eq 'Steam' -and ![IO.File]::Exists((Join-Path $TargetDir 'steam_api64_o.dll'))) {
            return 'already-a'
        }
        throw 'No original-file backup record found. Nothing was replaced. Keep the backup folder; do not copy DLLs from another user.'
    }
    Set-TowerNetwork $TargetDir 'Steam' ''
    return 'restored'
}

function Show-TowerNetworkStatus([string]$TargetDir) {
    $current = Get-TowerNetwork $TargetDir
    $store = Join-Path $TargetDir '.tower-network'
    $hasBackup = [IO.File]::Exists((Join-Path $store 'state.json'))
    $hasPending = [IO.File]::Exists((Join-Path $store 'pending.json'))
    $hasMixed = [IO.File]::Exists((Join-Path $TargetDir 'steam_api64_o.dll'))

    Write-Host ''
    Write-Host '========================================================'
    Write-Host "  游戏目录：$TargetDir"
    if ($current -eq 'OnlineFix') {
        Write-Host '  当前环境：B — Spacewar / OnlineFix 联机（AppID 480）' -ForegroundColor Cyan
    } else {
        Write-Host '  当前环境：A — 原版 Steam 联机（AppID 1245620）' -ForegroundColor Green
    }
    if ($hasBackup) {
        Write-Host '  原版备份：已有本机原版备份（.tower-network），可随时安全切回 A'
    } elseif ($current -eq 'OnlineFix') {
        Write-Host '  原版备份：未找到 .tower-network 备份（外部手动安装过 B，需先通过 Steam 验证文件完整性再切）' -ForegroundColor Yellow
    } else {
        Write-Host '  原版备份：首次切到 B 时将自动备份本机原版文件'
    }
    if ($hasPending) {
        Write-Host '  状态警告：检测到上次切换中途中断（pending.json），建议先选 [A] 自动回滚修复！' -ForegroundColor Red
    }
    if ($hasMixed) {
        Write-Host '  状态警告：检测到 steam_api64_o.dll（其他联机/破解代理残留），启动器会拦截，请先清理并验证原版文件！' -ForegroundColor Red
    }
    Write-Host '========================================================'
}

try {
    Import-TowerManualModules
    . (Join-Path $PSScriptRoot 'Tower-Network.ps1')
    . (Join-Path $PSScriptRoot 'Network-Files.ps1')

    if ($Action -in @('Install','Restore')) {
        Assert-TowerProcessesClosed
        if (!$GameDir) {
            $GameDir = Open-TowerGameFileDialog ''
            if (!$GameDir) { return }
        }
        $resolved = Resolve-TowerGameDirInput $GameDir
        if (!$resolved) { throw 'Select the Game folder containing eldenring.exe.' }
        if ($Action -eq 'Install') {
            Invoke-TowerInstallB $resolved
            Write-Host '【切换成功】已切换为 B 环境（Spacewar / OnlineFix）。本机原版文件与独立还原脚本保存在：Game\.tower-network' -ForegroundColor Green
            Write-Host '现在可以直接打开原来的爬塔启动器游玩（全队需同为 B 环境，且 Steam 保持登录）。'
        } else {
            $res = Invoke-TowerRestoreA $resolved
            if ($res -eq 'already-a') {
                Write-Host '当前已经是 A 环境（原版 Steam），无需还原。' -ForegroundColor Green
            } else {
                Write-Host '【还原成功】已切回 A 环境（原版 Steam），本机原版文件已恢复，备份仍保留。' -ForegroundColor Green
            }
        }
        return
    }

    # 统一交互式入口 (-Action Menu)
    Write-Host '========================================================' -ForegroundColor Cyan
    Write-Host '  黄金树爬塔 - 联机环境切换工具 (A / B 单入口版)' -ForegroundColor Cyan
    Write-Host '========================================================' -ForegroundColor Cyan
    Write-Host '  A 环境：原版 Steam 联机（AppID 1245620）'
    Write-Host '  B 环境：Spacewar / OnlineFix 联机（AppID 480）'
    Write-Host '  说明：切换完成后仍用原来的爬塔启动器启动；全队必须同为 A 或同为 B。'

    Assert-TowerProcessesClosed

    $targetDir = if ($GameDir) { Resolve-TowerGameDirInput $GameDir } else { $null }
    if (!$targetDir) {
        $targetDir = Select-TowerGameDirInteractive
        if (!$targetDir) {
            Write-Host '已取消操作。'
            return
        }
    }
    Save-TowerLastGameDir $targetDir

    while ($true) {
        Show-TowerNetworkStatus $targetDir
        Write-Host ''
        Write-Host '请选择要切换的联机环境：'
        Write-Host '  [A] 切换 / 还原为 A 环境（原版 Steam 1245620）'
        Write-Host '  [B] 切换 / 安装为 B 环境（Spacewar / OnlineFix 480）'
        Write-Host '  [C] 更换游戏目录'
        Write-Host '  [Q] 退出'
        Write-Host ''
        $choice = Read-Host '请输入 A / B / C / Q'
        if ($null -eq $choice) { return }
        $choice = $choice.Trim().ToUpperInvariant()

        if ($choice -eq 'Q') { return }
        if ($choice -eq 'C') {
            $nextDir = Select-TowerGameDirInteractive
            if ($nextDir) {
                $targetDir = $nextDir
                Save-TowerLastGameDir $targetDir
            }
            continue
        }
        if ($choice -notin @('A','B')) {
            Write-Host '无效输入，请输入 A、B、C 或 Q。' -ForegroundColor Yellow
            continue
        }

        try {
            if ($choice -eq 'B') {
                Invoke-TowerInstallB $targetDir
                Write-Host ''
                Write-Host '【切换成功】已切换为 B 环境（Spacewar / OnlineFix，AppID 480）！' -ForegroundColor Green
                Write-Host '  · 本机原版文件已备份在：Game\.tower-network（切勿删除）'
                Write-Host '  · 请保持 Steam 登录在线，直接用原来的爬塔启动器启动即可'
                Write-Host '  · 联机队友也必须同为 B 环境'
            } else {
                $res = Invoke-TowerRestoreA $targetDir
                Write-Host ''
                if ($res -eq 'already-a') {
                    Write-Host '【无需切换】当前游戏目录已经是 A 环境（原版 Steam），文件未作改动。' -ForegroundColor Green
                } else {
                    Write-Host '【切换成功】已还原为 A 环境（原版 Steam，AppID 1245620）！' -ForegroundColor Green
                    Write-Host '  · 本机原版文件已恢复，备份仍保留在 Game\.tower-network'
                    Write-Host '  · 直接用原来的爬塔启动器启动即可'
                }
            }
        } catch {
            Write-Host (Format-TowerNetworkError $_.Exception.Message) -ForegroundColor Red
        }
    }
} catch {
    Write-Host (Format-TowerNetworkError $_.Exception.Message) -ForegroundColor Red
    exit 1
}
