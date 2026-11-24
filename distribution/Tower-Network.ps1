function Get-TowerNetwork([string]$GameDir) {
    if ([IO.File]::Exists((Join-Path $GameDir 'OnlineFix64.dll')) -or [IO.File]::Exists((Join-Path $GameDir 'OnlineFix.ini'))) { return 'OnlineFix' }
    return 'Steam'
}

function Assert-TowerLaunchNetwork([string]$GameDir, [string]$Network) {
    if ([IO.File]::Exists((Join-Path $GameDir '.tower-network/pending.json'))) {
        throw '上次 A/B 切换中途被中断（Interrupted manual patch operation）。请先关闭游戏和启动器，打开补丁包中的「联机环境切换(A-B).cmd」选择 A 还原后再启动。'
    }
    Assert-TowerNetwork $GameDir $Network
    if ($Network -eq 'OnlineFix') {
        $bytes = [IO.File]::ReadAllBytes((Join-Path $GameDir 'dlllist.txt'))
        $canonical = [Text.Encoding]::ASCII.GetBytes('OnlineFix64.dll')
        if ([Convert]::ToBase64String($bytes) -cne [Convert]::ToBase64String($canonical)) {
            throw 'B 补丁的 dlllist.txt 未适配当前爬塔启动器（B patch list is not ready）。请关闭启动器，运行补丁包中的「联机环境切换(A-B).cmd」选择 B 重新应用一次。'
        }
    }
}

function Assert-TowerNetwork([string]$GameDir, [string]$Network) {
    if ($Network -notin @('Steam','OnlineFix')) { throw '未知的联机环境模式（Unknown network mode）。' }
    if ($Network -eq 'Steam') {
        foreach ($name in @('OnlineFix64.dll','OnlineFix.ini')) {
            if (Test-Path -LiteralPath (Join-Path $GameDir $name)) {
                throw 'Selected profile does not match the manually installed patch. 检测到游戏目录已有 OnlineFix 文件但未记录在备份中，请重启启动器或先用「联机环境切换(A-B).cmd」恢复后再切换。'
            }
        }
        return
    }
    $hashes = @{
        'OnlineFix64.dll' = '71a4f566abbf2ceb3b0d6159cd7950d120593c5badda4dde2711671957f16003'
        'steam_api64.dll' = 'ca1fa9a7609ab10b7926400559cf073e5888423cc156af72c6027d72a89eea73'
        'winmm.dll' = '54a6ece54641ee6e437caa351e493ddf4435f3e23e3d4ab4655c35f57b94fc59'
    }
    foreach ($name in $hashes.Keys) {
        $file = Join-Path $GameDir $name
        if (!(Test-Path -LiteralPath $file -PathType Leaf)) {
            throw "缺少 B 补丁文件 $name（Missing $name），可能被杀毒软件误删或未完整解压。请检查隔离区并用「联机环境切换(A-B).cmd」重新选择 B 或还原为 A。"
        }
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ine $hashes[$name]) {
            throw "B 补丁组件版本不匹配：$name（Unsupported OnlineFix component）。仅支持本补丁包自带的 2.0.3 组件，请勿混用其他来源的 OnlineFix 文件。"
        }
    }
    $ini = [IO.File]::ReadAllText((Join-Path $GameDir 'OnlineFix.ini'))
    foreach ($setting in @(@('RealAppId','1245620'), @('FakeAppId','480'))) {
        $entries = [regex]::Matches($ini, '(?mi)^\s*' + $setting[0] + '\s*=\s*([^\r\n;#]+)')
        if ($entries.Count -ne 1 -or $entries[0].Groups[1].Value.Trim() -ne $setting[1]) {
            throw "OnlineFix.ini 配置异常（Unexpected OnlineFix $($setting[0])），应为 RealAppId=1245620 且 FakeAppId=480。"
        }
    }
    if (Test-Path -LiteralPath (Join-Path $GameDir 'steam_api64_o.dll')) {
        throw '检测到混装的其他 Steam 代理文件 steam_api64_o.dll（Mixed Steam API proxies detected）。请先清理旧代理并通过 Steam 验证游戏文件完整性，再用「联机环境切换(A-B).cmd」切换。'
    }
    $null = Get-TowerOnlineFixDllList $GameDir
}

function Get-TowerOnlineFixDllList([string]$GameDir) {
    $file = Join-Path $GameDir 'dlllist.txt'
    $entries = @([IO.File]::ReadAllLines($file) | ForEach-Object { $_.Trim().Replace('/','\') } | Where-Object { $_ })
    if (@($entries | Where-Object { $_ -ieq 'OnlineFix64.dll' }).Count -ne 1 -or
        @($entries | Where-Object { $_ -ine 'OnlineFix64.dll' -and $_ -ine 'SeamlessCoop\ersc.dll' }).Count -ne 0 -or
        @($entries | Where-Object { $_ -ieq 'SeamlessCoop\ersc.dll' }).Count -gt 1) {
        throw 'dlllist.txt 内容异常（Unsupported dlllist.txt）。请关闭游戏后运行「联机环境切换(A-B).cmd」重新选择 B 修复。'
    }
    return $entries
}
