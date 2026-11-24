param(
    [string]$InstallDir = $PSScriptRoot,
    [string]$GameDir,
    [switch]$TestDirectLaunch,
    [switch]$NoOpen
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
try { [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false) } catch {}

$loaderSupport = Join-Path $PSScriptRoot 'Tower-Loader.ps1'
if (!(Test-Path -LiteralPath $loaderSupport -PathType Leaf)) {
    $loaderSupport = Join-Path $InstallDir 'Tower-Loader.ps1'
}
if (Test-Path -LiteralPath $loaderSupport -PathType Leaf) {
    . $loaderSupport
}

# 内联轻量 C# Minidump 解析器：读取 ExceptionStream (6) 与 ModuleListStream (4)
# 不依赖 WinDbg / 外部工具，兼容 Windows PowerShell 5.1 (.NET Framework 4.8) 与 PowerShell 7
$minidumpCode = @'
using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

public sealed class TowerDumpModule
{
    public string Path;
    public ulong BaseAddress;
    public uint Size;
}

public sealed class TowerDumpSummary
{
    public bool Valid;
    public string Error;
    public bool HasException;
    public uint ExceptionCode;
    public ulong ExceptionAddress;
    public string FaultModule;
    public ulong FaultOffset;
    public List<TowerDumpModule> Modules = new List<TowerDumpModule>();
    public List<TowerDumpModule> ThirdPartyModules = new List<TowerDumpModule>();
}

public static class TowerMinidumpInspector
{
    private const uint MDMP_SIGNATURE = 0x504D444D; // 'MDMP'
    private const uint MODULE_LIST_STREAM = 4;
    private const uint EXCEPTION_STREAM = 6;

    public static TowerDumpSummary Inspect(string dumpPath)
    {
        var summary = new TowerDumpSummary();
        try
        {
            using (var fs = File.Open(dumpPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
            using (var br = new BinaryReader(fs))
            {
                if (fs.Length < 32) { summary.Error = "File too small"; return summary; }
                uint sig = br.ReadUInt32();
                if (sig != MDMP_SIGNATURE) { summary.Error = "Not a valid MDMP minidump"; return summary; }
                br.ReadUInt32(); // version
                uint streamCount = br.ReadUInt32();
                uint streamDirRva = br.ReadUInt32();

                uint modRva = 0, modSize = 0;
                uint excRva = 0, excSize = 0;
                for (uint i = 0; i < streamCount; i++)
                {
                    long pos = (long)streamDirRva + (long)i * 12;
                    if (pos + 12 > fs.Length) break;
                    fs.Seek(pos, SeekOrigin.Begin);
                    uint streamType = br.ReadUInt32();
                    uint dataSize = br.ReadUInt32();
                    uint rva = br.ReadUInt32();
                    if (streamType == MODULE_LIST_STREAM) { modRva = rva; modSize = dataSize; }
                    else if (streamType == EXCEPTION_STREAM) { excRva = rva; excSize = dataSize; }
                }

                if (modRva > 0 && modSize >= 4 && (long)modRva + 4 <= fs.Length)
                {
                    fs.Seek(modRva, SeekOrigin.Begin);
                    uint count = br.ReadUInt32();
                    // MINIDUMP_MODULE is 108 bytes
                    for (uint i = 0; i < count; i++)
                    {
                        long entryPos = (long)modRva + 4 + (long)i * 108;
                        if (entryPos + 108 > fs.Length) break;
                        fs.Seek(entryPos, SeekOrigin.Begin);
                        ulong baseAddr = br.ReadUInt64();
                        uint imgSize = br.ReadUInt32();
                        br.ReadUInt32(); // checksum
                        br.ReadUInt32(); // timestamp
                        uint nameRva = br.ReadUInt32();
                        string name = ReadMinidumpString(fs, br, nameRva);
                        var m = new TowerDumpModule { Path = name, BaseAddress = baseAddr, Size = imgSize };
                        summary.Modules.Add(m);
                        if (IsThirdParty(name)) summary.ThirdPartyModules.Add(m);
                    }
                }

                if (excRva > 0 && excSize >= 32 && (long)excRva + 32 <= fs.Length)
                {
                    fs.Seek(excRva, SeekOrigin.Begin);
                    br.ReadUInt32(); // ThreadId
                    br.ReadUInt32(); // __alignment
                    uint code = br.ReadUInt32(); // ExceptionCode
                    br.ReadUInt32(); // ExceptionFlags
                    br.ReadUInt64(); // ExceptionRecord
                    ulong addr = br.ReadUInt64(); // ExceptionAddress
                    summary.HasException = true;
                    summary.ExceptionCode = code;
                    summary.ExceptionAddress = addr;
                    foreach (var m in summary.Modules)
                    {
                        if (addr >= m.BaseAddress && addr < m.BaseAddress + m.Size)
                        {
                            int slash = m.Path.LastIndexOfAny(new[] { '\\', '/' });
                            summary.FaultModule = slash >= 0 ? m.Path.Substring(slash + 1) : m.Path;
                            summary.FaultOffset = addr - m.BaseAddress;
                            break;
                        }
                    }
                }
                summary.Valid = true;
            }
        }
        catch (Exception ex)
        {
            summary.Error = ex.Message;
        }
        return summary;
    }

    private static string ReadMinidumpString(FileStream fs, BinaryReader br, uint rva)
    {
        if (rva == 0 || (long)rva + 4 > fs.Length) return "";
        fs.Seek(rva, SeekOrigin.Begin);
        uint byteLen = br.ReadUInt32();
        if (byteLen == 0 || byteLen > 4096 || (long)rva + 4 + byteLen > fs.Length) return "";
        byte[] bytes = br.ReadBytes((int)byteLen);
        return Encoding.Unicode.GetString(bytes).TrimEnd('\0');
    }

    private static bool IsThirdParty(string path)
    {
        if (string.IsNullOrEmpty(path)) return false;
        string p = path.Replace('/', '\\').ToLowerInvariant();
        if (p.EndsWith("\\eldenring.exe")) return false;
        if (p.Contains("\\windows\\system32\\") || p.Contains("\\windows\\syswow64\\") || p.Contains("\\windows\\winsxs\\"))
        {
            // System32 下常见会被第三方驱动/音频/覆盖替换或注入的关键字除外
            if (p.Contains("nahimic") || p.Contains("sonicstudio") || p.Contains("elevoc") || p.Contains("rtss") || p.Contains("rivatuner") || p.Contains("reshade"))
                return true;
            return false;
        }
        return true;
    }
}
'@

if (-not ('TowerMinidumpInspector' -as [type])) {
    Add-Type -TypeDefinition $minidumpCode -Language CSharp
}

function Find-TowerCrashDumps([string]$GameDirectory) {
    $markers = @('eldenring','start_protected_game.exe','me3.exe','me3-launcher.exe')
    $roots = @()
    if ($env:LOCALAPPDATA) {
        $roots += (Join-Path $env:LOCALAPPDATA 'CrashDumps')
        $roots += (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\WER\ReportQueue')
        $roots += (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\WER\ReportArchive')
    }
    if ($GameDirectory) { $roots += $GameDirectory }
    $found = New-Object System.Collections.ArrayList
    foreach ($r in ($roots | Select-Object -Unique)) {
        if (!(Test-Path -LiteralPath $r -PathType Container)) { continue }
        $candidates = @(Get-ChildItem -LiteralPath $r -File -ErrorAction SilentlyContinue)
        foreach ($sub in @(Get-ChildItem -LiteralPath $r -Directory -ErrorAction SilentlyContinue)) {
            $candidates += @(Get-ChildItem -LiteralPath $sub.FullName -File -ErrorAction SilentlyContinue)
        }
        foreach ($f in $candidates) {
            if ($f.Extension.ToLowerInvariant() -notin @('.dmp','.hdmp','.mdmp')) { continue }
            $rel = $f.FullName.Substring($r.Length)
            $hit = $false
            foreach ($m in $markers) { if ($rel.IndexOf($m, [StringComparison]::OrdinalIgnoreCase) -ge 0) { $hit = $true; break } }
            if ($hit) { [void]$found.Add($f) }
        }
    }
    return @($found | Sort-Object LastWriteTime -Descending)
}

function Invoke-StartupDiagnostics([string]$Root,[string]$GameDirectory,[switch]$DirectLaunch) {
    $root = [IO.Path]::GetFullPath($Root)
    if (!$GameDirectory -and (Test-Path -LiteralPath (Join-Path $root 'GamePath.txt') -PathType Leaf)) {
        $GameDirectory = [IO.File]::ReadAllText((Join-Path $root 'GamePath.txt')).Trim().Trim('"')
    }
    if ((!$GameDirectory -or !(Test-Path -LiteralPath (Join-Path $GameDirectory 'eldenring.exe') -PathType Leaf)) -and $env:OS -eq 'Windows_NT') {
        try {
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object Windows.Forms.OpenFileDialog
            $dialog.Title = '请选择游戏目录下的 eldenring.exe（通常在 ELDEN RING\Game）'
            $dialog.Filter = 'Elden Ring (eldenring.exe)|eldenring.exe'
            try {
                if ($dialog.ShowDialog() -eq [Windows.Forms.DialogResult]::OK) {
                    $GameDirectory = [IO.Path]::GetDirectoryName($dialog.FileName)
                }
            } finally { $dialog.Dispose() }
        } catch {}
    }

    $stamp = [DateTime]::Now.ToString('yyyyMMdd-HHmmss')
    $outDir = Join-Path $root ("launcher-data\startup-diagnostic\$stamp")
    [IO.Directory]::CreateDirectory($outDir) | Out-Null
    $reportPath = Join-Path $outDir 'startup-diagnostic.txt'
    $utf8Bom = [Text.UTF8Encoding]::new($true)
    $lines = New-Object System.Collections.ArrayList
    $suspects = New-Object System.Collections.ArrayList

    function Add-Line([string]$text) { [void]$lines.Add($text) }
    function Add-Section([string]$title) { Add-Line ''; Add-Line ("==================== $title ====================") }

    Add-Line "黄金树爬塔 · 启动崩溃自动诊断报告"
    Add-Line ("生成时间: " + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz'))
    Add-Line ("安装目录: $root")
    Add-Line ("游戏目录: " + $(if ($GameDirectory) {$GameDirectory} else {'（未定位到 eldenring.exe）'}))
    Add-Line ("系统: " + [Environment]::OSVersion + " | PowerShell: " + $PSVersionTable.PSVersion + " | 64位: " + [Environment]::Is64BitProcess)

    # 1. 权限与 Steam 进程状态
    Add-Section '1. 权限与进程环境'
    $isAdmin = $false
    try {
        $id = [Security.Principal.WindowsIdentity]::GetCurrent()
        $isAdmin = ([Security.Principal.WindowsPrincipal]::new($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch {}
    Add-Line ("当前诊断脚本管理员权限: $isAdmin")
    Add-Line ("环境变量 __COMPAT_LAYER: " + $(if ($env:__COMPAT_LAYER) {$env:__COMPAT_LAYER} else {'(无)'}))
    if ($env:__COMPAT_LAYER) { [void]$suspects.Add("当前进程继承了兼容层 __COMPAT_LAYER=$($env:__COMPAT_LAYER)") }
    $steam = @(Get-Process -Name 'steam' -ErrorAction SilentlyContinue)
    Add-Line ("Steam 运行中: " + ($steam.Count -gt 0))
    if ($steam.Count -eq 0) { [void]$suspects.Add('检测到 Steam 未在运行') }
    if ($env:OS -eq 'Windows_NT') {
        foreach ($pair in @(@('MOD 安装目录', $root), @('游戏目录', $GameDirectory))) {
            if (!$pair[1]) { continue }
            try {
                $driveRoot = [IO.Path]::GetPathRoot($pair[1])
                if ($driveRoot -and $driveRoot -match '^[A-Za-z]:\\?$') {
                    $fmt = [IO.DriveInfo]::new($driveRoot).DriveFormat
                    Add-Line ("$($pair[0]) 所在磁盘 ($driveRoot) 文件系统: $fmt")
                    if ($fmt -and $fmt -ine 'NTFS') {
                        [void]$suspects.Add("$($pair[0]) 位于 $fmt 分区（$driveRoot），不支持 NTFS 目录联接，会导致启动报“函数不正确”；请移动到 NTFS 分区后重试")
                    }
                }
            } catch {}
        }
    }

    # 2. 游戏目录与残留 DLL 扫描
    Add-Section '2. 游戏目录文件与残留 DLL 扫描'
    if ($GameDirectory -and (Test-Path -LiteralPath (Join-Path $GameDirectory 'eldenring.exe') -PathType Leaf)) {
        $exePath = Join-Path $GameDirectory 'eldenring.exe'
        $ver = try { (Get-Item -LiteralPath $exePath).VersionInfo.FileVersion } catch { 'unknown' }
        $sha = try { (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToLowerInvariant() } catch { 'unknown' }
        Add-Line "eldenring.exe 版本: $ver | SHA256: $sha"
        if (Get-Command Get-TowerGameDirDiagnostics -ErrorAction SilentlyContinue) {
            $diag = Get-TowerGameDirDiagnostics $GameDirectory
            Add-Line "Game\sd 音频文件数量: $($diag.SdFileCount) | steam_appid.txt 存在: $($diag.SteamAppIdPresent)"
            if ($diag.SdFileCount -le 0) { [void]$suspects.Add('游戏目录缺少 sd 音频目录或为空，请 Steam 校验文件完整性') }
            if ($diag.HighRiskProxies.Count -gt 0) {
                Add-Line ("[高危] 发现残留代理/劫持 DLL: " + ($diag.HighRiskProxies -join ', '))
                [void]$suspects.Add("游戏目录存在高危残留代理 DLL（EAC 启动时会屏蔽它们，走 ME3/YAFSML/裸跑时会加载并致崩）：" + ($diag.HighRiskProxies -join ', '))
            }
            if ($diag.UnexpectedFiles.Count -gt 0) {
                Add-Line ("非原版额外文件: " + ($diag.UnexpectedFiles -join ', '))
            } else {
                Add-Line '未发现非原版 .dll / .asi 残留文件。'
            }
        }
        Add-Line '--- Game 目录全部 .dll / .asi / .ini / .txt ---'
        foreach ($f in @(Get-ChildItem -LiteralPath $GameDirectory -File -ErrorAction SilentlyContinue | Where-Object { $_.Extension.ToLowerInvariant() -in @('.dll','.asi','.ini','.txt','.exe') } | Sort-Object Name)) {
            $sig = ''
            if ($f.Extension.ToLowerInvariant() -in @('.dll','.exe') -and (Get-Command Get-AuthenticodeSignature -ErrorAction SilentlyContinue)) {
                try { $s = Get-AuthenticodeSignature -LiteralPath $f.FullName; $sig = " | Sig=$($s.Status)" } catch {}
            }
            Add-Line ("  $($f.Name) | $($f.Length) B | $($f.LastWriteTime.ToString('yyyy-MM-dd HH:mm'))$sig")
        }
    } else {
        Add-Line '未找到 eldenring.exe，跳过游戏目录扫描。'
    }

    # 3. 兼容性、IFEO 与漏洞利用保护
    Add-Section '3. Windows 兼容性垫片与 IFEO / Exploit Protection'
    $compatHit = $false
    foreach ($hive in @('HKCU:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers','HKLM:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers')) {
        if (Test-Path -LiteralPath $hive) {
            $props = (Get-ItemProperty -LiteralPath $hive -ErrorAction SilentlyContinue).PSObject.Properties | Where-Object {
                $_.Name -notlike 'PS*' -and $_.Name -match '(eldenring|start_protected_game|me3|yafsml)\.exe$'
            }
            foreach ($p in $props) {
                $compatHit = $true
                Add-Line ("  [$hive] $($p.Name) = $($p.Value)")
                [void]$suspects.Add("注册表给 $($p.Name) 设置了兼容性垫片: $($p.Value)")
            }
        }
    }
    if (!$compatHit) { Add-Line '未发现针对 eldenring/me3/yafsml 的 AppCompatFlags\Layers 条目。' }
    $ifeo = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\eldenring.exe'
    $maxLoaderThreads = $null
    if (Test-Path -LiteralPath $ifeo) {
        $vals = (Get-ItemProperty -LiteralPath $ifeo -ErrorAction SilentlyContinue).PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' }
        foreach ($v in $vals) {
            Add-Line ("  [IFEO] $($v.Name) = $($v.Value)")
            if ($v.Name -eq 'MaxLoaderThreads') { $maxLoaderThreads = $v.Value }
            if ($v.Name -match 'Mitigation|Debugger') { [void]$suspects.Add("IFEO 对 eldenring.exe 启用了 $($v.Name)") }
        }
    } else {
        Add-Line '未发现 eldenring.exe 的 IFEO 策略项。'
    }
    if ($maxLoaderThreads -eq 1) {
        Add-Line '  [并行加载] 已通过 MaxLoaderThreads=1 关闭 eldenring.exe 的 Windows DLL 并行加载。'
    } else {
        Add-Line '  [并行加载] 未设置 MaxLoaderThreads=1（若崩在 me3_mod_host.dll / YAFSML 且未出画面，可在启动器「设置 → 问题修复」一键关闭法环并行加载）。'
    }

    # 4. 音频设备与常见第三方注入/覆盖/音效/反作弊进程
    Add-Section '4. 音频设备与后台注入/覆盖/音效进程'
    if (Get-Command Get-CimInstance -ErrorAction SilentlyContinue) {
        try {
            foreach ($dev in @(Get-CimInstance Win32_SoundDevice -ErrorAction Stop)) {
                Add-Line ("  声卡: $($dev.Name) | 状态=$($dev.Status) | 厂商=$($dev.Manufacturer)")
            }
        } catch { Add-Line ("  读取声卡失败: " + $_.Exception.Message) }
    } else {
        Add-Line '  (非 Windows CIM 环境，跳过声卡枚举)'
    }
    $watchPattern = '^(Nahimic|SS3Svc|SonicStudio|SteelSeries|Sonar|Razer|RzSynapse|THX|lghub|voicemeeter|RTSS|MSIAfterburner|GamePP|Discord|obs64|vgtray|SGuard|ACE|360|HipsDaemon|Overwolf|Medal|GeForce|nvcontainer|ArmouryCrate|Aac)'
    $procs = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match $watchPattern } | Sort-Object ProcessName -Unique)
    if ($procs.Count -gt 0) {
        Add-Line '检测到以下可能向游戏注入 DLL 的后台进程:'
        foreach ($p in $procs) {
            $path = try { $p.Path } catch { '' }
            Add-Line ("  $($p.ProcessName) (PID=$($p.Id)) $path")
        }
        [void]$suspects.Add("后台运行了常见音效/覆盖/安全注入进程: " + (($procs | Select-Object -ExpandProperty ProcessName) -join ', '))
    } else {
        Add-Line '未检测到常见音效/覆盖/反作弊后台进程。'
    }

    # 5. 最新崩溃转储（.dmp）解析
    Add-Section '5. 最新崩溃转储 (.dmp) 模块与异常解析'
    $dumps = @(Find-TowerCrashDumps $GameDirectory)
    if ($dumps.Count -eq 0) {
        Add-Line '未找到 eldenring / me3 相关 .dmp 崩溃转储。'
    } else {
        $latest = $dumps[0]
        Add-Line ("共找到 $($dumps.Count) 个转储，解析最新一个: $($latest.FullName) ($($latest.Length) B, $($latest.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss')))")
        $summary = [TowerMinidumpInspector]::Inspect($latest.FullName)
        if (!$summary.Valid) {
            Add-Line ("  解析失败: $($summary.Error)")
        } else {
            if ($summary.HasException) {
                $hexCode = ('0x{0:X8}' -f $summary.ExceptionCode)
                $loc = if ($summary.FaultModule) { "$($summary.FaultModule)+0x$('{0:X}' -f $summary.FaultOffset)" } else { "0x$('{0:X}' -f $summary.ExceptionAddress)" }
                Add-Line "  异常代码: $hexCode | 崩溃位置: $loc"
                if ($summary.FaultOffset -eq 0x1ebcef9) {
                    [void]$suspects.Add("最新转储命中已知 Wwise 初始化崩点 eldenring.exe+0x1EBCEF9 (MoWwiseMan.cpp:883)")
                }
                if ($summary.FaultModule -and $summary.FaultModule -match '^(me3_mod_host|me3|yafsml).*\.dll$') {
                    [void]$suspects.Add("最新转储崩在 $($summary.FaultModule)（常见原因为 Windows DLL 并行加载工作线程未分配 TLS 与加载器冲突）：请在启动器「设置 → 问题修复」点击「一键关闭法环并行加载」，或给 HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\eldenring.exe 写入 REG_DWORD MaxLoaderThreads = 1 后重试")
                }
            } else {
                Add-Line '  (转储内无 ExceptionStream)'
            }
            Add-Line ("  进程已加载模块总数: $($summary.Modules.Count) | 非 Windows 系统目录第三方模块: $($summary.ThirdPartyModules.Count)")
            foreach ($m in $summary.ThirdPartyModules) {
                Add-Line ("    [第三方模块] $($m.Path) (Base=0x$('{0:X}' -f $m.BaseAddress), Size=0x$('{0:X}' -f $m.Size))")
            }
        }
    }

    # 6. 最近加载器日志摘录
    Add-Section '6. 最近 ME3 / YAFSML / 启动器日志尾部'
    $logCandidates = New-Object System.Collections.ArrayList
    if ($env:LOCALAPPDATA) {
        $me3LogDir = Join-Path $env:LOCALAPPDATA 'garyttierney\me3\logs'
        if (Test-Path -LiteralPath $me3LogDir -PathType Container) {
            $latestMe3 = Get-ChildItem -LiteralPath $me3LogDir -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
            if ($latestMe3) { [void]$logCandidates.Add(@('ME3', $latestMe3.FullName)) }
        }
    }
    foreach ($yafLog in @((Join-Path $root 'YAFSML.log'), (Join-Path $root 'loader\yafsml\YAFSML.log'), $(if ($GameDirectory) { Join-Path $GameDirectory 'YAFSML.log' }))) {
        if ($yafLog -and (Test-Path -LiteralPath $yafLog -PathType Leaf)) { [void]$logCandidates.Add(@('YAFSML', $yafLog)) }
    }
    $launcherLogs = Join-Path $root 'launcher-data\logs'
    if (Test-Path -LiteralPath $launcherLogs -PathType Container) {
        $latestRun = Get-ChildItem -LiteralPath $launcherLogs -Directory -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($latestRun) {
            foreach ($n in @('stdout.log','stderr.log')) {
                $p = Join-Path $latestRun.FullName $n
                if (Test-Path -LiteralPath $p -PathType Leaf) { [void]$logCandidates.Add(@("Launcher/$n", $p)) }
            }
        }
    }
    if ($logCandidates.Count -eq 0) {
        Add-Line '未找到可读取的 ME3 / YAFSML / 启动器日志。'
    } else {
        foreach ($pair in $logCandidates) {
            Add-Line ("--- [$($pair[0])] $($pair[1]) (最后 40 行) ---")
            try {
                foreach ($l in @(Get-Content -LiteralPath $pair[1] -Tail 40 -ErrorAction Stop)) { Add-Line ("  $l") }
            } catch { Add-Line ("  读取失败: " + $_.Exception.Message) }
        }
    }

    # 7. 可选：裸跑 eldenring.exe 二分对照测试（绕过 ME3 与 YAFSML）
    if ($DirectLaunch) {
        Add-Section '7. 裸跑 eldenring.exe 二分对照测试（无 ME3 / 无 YAFSML / 无 Mod）'
        if (!$GameDirectory -or !(Test-Path -LiteralPath (Join-Path $GameDirectory 'eldenring.exe') -PathType Leaf)) {
            Add-Line '未找到 eldenring.exe，无法执行裸跑测试。'
        } elseif (Get-Process -Name 'eldenring' -ErrorAction SilentlyContinue) {
            Add-Line '检测到 eldenring.exe 已在运行，跳过裸跑测试。'
        } else {
            $appIdPath = Join-Path $GameDirectory 'steam_appid.txt'
            $createdAppId = $false
            $prevCompat = $env:__COMPAT_LAYER
            try {
                if (!(Test-Path -LiteralPath $appIdPath -PathType Leaf)) {
                    [IO.File]::WriteAllText($appIdPath, "1245620`r`n", [Text.UTF8Encoding]::new($false))
                    $createdAppId = $true
                }
                Remove-Item -LiteralPath 'Env:__COMPAT_LAYER' -ErrorAction SilentlyContinue
                $proc = Start-Process -FilePath (Join-Path $GameDirectory 'eldenring.exe') -WorkingDirectory $GameDirectory -PassThru
                $exited = $proc.WaitForExit(12000)
                if ($exited) {
                    $codeHex = ('0x{0:X8}' -f ([BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$proc.ExitCode), 0)))
                    Add-Line "结果: 裸跑 eldenring.exe 在 12 秒内异常退出！ExitCode=$($proc.ExitCode) ($codeHex)"
                    [void]$suspects.Add("裸跑 eldenring.exe（完全不经过 ME3/YAFSML）同样崩溃 ($codeHex)，可确认问题出在本机环境/残留 DLL/音频或系统策略，而非加载器或爬塔模组")
                } else {
                    Add-Line '结果: 裸跑 eldenring.exe 存活超过 12 秒（未在初始化阶段崩溃），正在自动关闭测试进程。'
                    try { $proc.CloseMainWindow() | Out-Null; if (!$proc.WaitForExit(3000)) { $proc.Kill() } } catch {}
                    [void]$suspects.Add('裸跑 eldenring.exe 正常存活，但加载器启动失败：重点排查 Windows DLL 并行加载冲突（在启动器「设置 → 问题修复」点击「一键关闭法环并行加载」写入 MaxLoaderThreads=1）、安全软件/反作弊拦截，或 ME3/YAFSML 日志报错')
                }
            } catch {
                Add-Line ("裸跑测试执行失败: " + $_.Exception.Message)
            } finally {
                if ($createdAppId -and (Test-Path -LiteralPath $appIdPath)) {
                    Remove-Item -LiteralPath $appIdPath -Force -ErrorAction SilentlyContinue
                }
                if ($null -eq $prevCompat) { Remove-Item -LiteralPath 'Env:__COMPAT_LAYER' -ErrorAction SilentlyContinue }
                else { $env:__COMPAT_LAYER = $prevCompat }
            }
        }
    }

    # 把重点疑似原因插到报告最前面，方便一眼定位
    $header = New-Object System.Collections.ArrayList
    [void]$header.Add('==================== 重点疑似原因（先看这里） ====================')
    if ($suspects.Count -eq 0) {
        [void]$header.Add('未自动命中明显异常项；请结合下方“第三方加载模块”与加载器日志进一步核对。')
    } else {
        for ($i = 0; $i -lt $suspects.Count; $i++) {
            [void]$header.Add(("  [$($i + 1)] " + $suspects[$i]))
        }
    }
    [void]$header.Add('')
    $finalLines = @($lines[0..4]) + @('') + @($header) + @($lines[5..($lines.Count - 1)])
    [IO.File]::WriteAllLines($reportPath, $finalLines, $utf8Bom)
    return $reportPath
}

if ($MyInvocation.InvocationName -ne '.') {
    $report = Invoke-StartupDiagnostics -Root $InstallDir -GameDirectory $GameDir -DirectLaunch:$TestDirectLaunch
    Write-Output "诊断报告已生成: $report"
    Write-Output '请将此 startup-diagnostic.txt 文件发给作者即可快速定位原因。'
    if (!$NoOpen -and $env:OS -eq 'Windows_NT') {
        try { Start-Process -FilePath 'notepad.exe' -ArgumentList $report | Out-Null } catch {}
    }
}
