$ErrorActionPreference = 'Stop'

# 1. Verify UTF-8 BOM & PowerShell syntax on both scripts
foreach ($rel in @('distribution/Tower-Loader.ps1', 'distribution/Diagnose-Startup.ps1')) {
    $path = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\\$rel").Replace('\', [IO.Path]::DirectorySeparatorChar))
    $bytes = [IO.File]::ReadAllBytes($path)
    if ($bytes.Length -lt 3 -or $bytes[0] -ne 239 -or $bytes[1] -ne 187 -or $bytes[2] -ne 191) {
        throw "$rel must have UTF-8 BOM for Windows PowerShell 5.1"
    }
    $tokens = $null; $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -gt 0) {
        $msg = ($errors | ForEach-Object { "$($_.Extent.StartLineNumber): $($_.Message)" }) -join '; '
        throw "$rel parse error: $msg"
    }
}

. (Join-Path $PSScriptRoot '../distribution/Tower-Loader.ps1')
. (Join-Path $PSScriptRoot '../distribution/Diagnose-Startup.ps1')

$tmp = Join-Path ([IO.Path]::GetTempPath()) ('tower-diag-test-' + [guid]::NewGuid().ToString('N'))
$game = Join-Path $tmp 'Game'
New-Item -ItemType Directory -Path $game, (Join-Path $game 'sd') -Force | Out-Null

try {
    # 2. Verify game directory proxy/stray DLL detection
    foreach ($f in @('eldenring.exe','oo2core_6_win64.dll','steam_api64.dll','dinput8.dll','CustomAudioHook.dll')) {
        [IO.File]::WriteAllText((Join-Path $game $f), 'fixture')
    }
    [IO.File]::WriteAllText((Join-Path $game 'sd/Init.bnk'), 'bnk')
    $diag = Get-TowerGameDirDiagnostics $game
    if ($diag.SdFileCount -ne 1) { throw "Expected 1 sd file, got $($diag.SdFileCount)" }
    if ($diag.HighRiskProxies.Count -ne 1 -or $diag.HighRiskProxies[0] -notlike 'dinput8.dll*') {
        throw "Expected dinput8.dll high-risk proxy, got: $($diag.HighRiskProxies -join ', ')"
    }
    if ($diag.UnexpectedFiles.Count -ne 2) {
        throw "Expected 2 unexpected files, got $($diag.UnexpectedFiles.Count): $($diag.UnexpectedFiles -join ', ')"
    }

    # 3. Build a valid synthetic MDMP minidump (ExceptionStream=6 + ModuleListStream=4) and verify C# parser
    $dumpPath = Join-Path $game 'eldenring.exe.1234.dmp'
    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($ms)
    # Header (32 bytes): sig, version, streamCount=2, streamDirRva=32, checksum=0, time=0, flags=0
    $bw.Write([uint32]0x504D444D)
    $bw.Write([uint32]0x0000A793)
    $bw.Write([uint32]2)
    $bw.Write([uint32]32)
    $bw.Write([uint32]0)
    $bw.Write([uint32]0)
    $bw.Write([uint64]0)

    # Stream directory at offset 32 (2 * 12 = 24 bytes -> ends at 56)
    # ModuleListStream at offset 64: 4 + 3*108 = 328 bytes -> ends at 392
    # ExceptionStream at offset 400: 168 bytes -> ends at 568
    # Strings starting at offset 600
    $bw.Write([uint32]4); $bw.Write([uint32]328); $bw.Write([uint32]64)
    $bw.Write([uint32]6); $bw.Write([uint32]168); $bw.Write([uint32]400)
    while ($ms.Position -lt 64) { $bw.Write([byte]0) }

    $modSpecs = @(
        @{ Path = 'C:\Games\ELDEN RING\Game\eldenring.exe'; Base = [uint64]0x140000000; Size = [uint32]0x04000000 },
        @{ Path = 'C:\Windows\System32\ntdll.dll'; Base = [uint64]0x7FF900000000; Size = [uint32]0x00200000 },
        @{ Path = 'C:\Program Files\AudioEnhance\bad_audio_hook.dll'; Base = [uint64]0x7FF800000000; Size = [uint32]0x00050000 }
    )
    $strOffsets = @()
    $curStr = 600
    foreach ($m in $modSpecs) {
        $strOffsets += $curStr
        $curStr += 4 + ([Text.Encoding]::Unicode.GetByteCount($m.Path) + 2)
    }

    # ModuleListStream at 64
    $bw.Write([uint32]$modSpecs.Count)
    for ($i = 0; $i -lt $modSpecs.Count; $i++) {
        $m = $modSpecs[$i]
        $start = $ms.Position
        $bw.Write([uint64]$m.Base)
        $bw.Write([uint32]$m.Size)
        $bw.Write([uint32]0) # checksum
        $bw.Write([uint32]0) # timestamp
        $bw.Write([uint32]$strOffsets[$i])
        while ($ms.Position -lt ($start + 108)) { $bw.Write([byte]0) }
    }

    # ExceptionStream at 400
    while ($ms.Position -lt 400) { $bw.Write([byte]0) }
    $bw.Write([uint32]1234) # ThreadId
    $bw.Write([uint32]0)    # align
    $bw.Write([uint32]0xC0000005L) # ExceptionCode
    $bw.Write([uint32]0)    # Flags
    $bw.Write([uint64]0)    # Record
    $bw.Write([uint64](0x140000000 + 0x1EBCEF9)) # ExceptionAddress
    while ($ms.Position -lt 600) { $bw.Write([byte]0) }

    # Minidump UTF-16LE strings
    for ($i = 0; $i -lt $modSpecs.Count; $i++) {
        $bytes = [Text.Encoding]::Unicode.GetBytes($modSpecs[$i].Path)
        $bw.Write([uint32]$bytes.Length)
        $bw.Write($bytes)
        $bw.Write([uint16]0)
    }
    [IO.File]::WriteAllBytes($dumpPath, $ms.ToArray())
    $bw.Dispose(); $ms.Dispose()

    $sum = [TowerMinidumpInspector]::Inspect($dumpPath)
    if (!$sum.Valid) { throw "Minidump parse failed: $($sum.Error)" }
    if (!$sum.HasException -or $sum.ExceptionCode -ne ([uint32]0xC0000005L)) { throw "Expected 0xC0000005 exception" }
    if ($sum.FaultModule -ne 'eldenring.exe' -or $sum.FaultOffset -ne 0x1EBCEF9) {
        throw "Expected eldenring.exe+0x1EBCEF9, got $($sum.FaultModule)+0x$('{0:X}' -f $sum.FaultOffset)"
    }
    if ($sum.ThirdPartyModules.Count -ne 1 -or $sum.ThirdPartyModules[0].Path -notlike '*bad_audio_hook.dll') {
        throw "Expected 1 third-party module (bad_audio_hook.dll), got $($sum.ThirdPartyModules.Count)"
    }

    # 4. Run full Invoke-StartupDiagnostics end-to-end and verify report contents
    $reportPath = Invoke-StartupDiagnostics -Root $tmp -GameDirectory $game
    $reportText = [IO.File]::ReadAllText($reportPath)
    foreach ($needle in @('重点疑似原因','dinput8.dll','0xC0000005','eldenring.exe+0x1EBCEF9','bad_audio_hook.dll')) {
        if (!$reportText.Contains($needle)) { throw "Report missing expected diagnostic item: $needle" }
    }
    Write-Output 'Startup diagnostics tests passed: BOM, syntax, proxy DLL scan, MDMP exception/module parser, and report generation.'
} finally {
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
