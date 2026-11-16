param(
    [Parameter(Mandatory=$true)][string]$PackageRoot,
    [string]$CertificateThumbprint,
    [ValidateSet('CurrentUser','LocalMachine')][string]$Store = 'CurrentUser',
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $PackageRoot).Path
if ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Sign a real staging directory, not a link.' }
if (Get-ChildItem -LiteralPath $root -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) { throw 'Staging directory contains a reparse point.' }
$owned = '^(ErdtreeTowerLauncher\.exe|Tower\.(Update|AssetRestore|AssetDiagnose|IconCodec|SeamlessGate)\.(exe|dll)|erdtree_tower_preview(?:_v\d+)?\.dll|tower_transmog\.dll)$'
$scripts = @('Start-Tower.ps1','Launch-Logged.ps1','Launch-Seamless-Tower.ps1','Tower-Loader.ps1','Tower-Network.ps1','Tower-SessionGate.ps1')
$files = @(Get-ChildItem -LiteralPath $root -Recurse -File | Where-Object {
    $_.Name -match $owned -or ($_.DirectoryName -eq $root -and $_.Name -in $scripts)
})
if (!$files.Count) { throw 'No tower-owned release components found.' }
if ($CheckOnly) {
    foreach ($file in $files) {
        $signature = Get-AuthenticodeSignature -LiteralPath $file.FullName
        [pscustomobject]@{File=$file.FullName;Company=$file.VersionInfo.CompanyName;Version=$file.VersionInfo.FileVersion;Signature=[string]$signature.Status}
    }
    return
}
$thumbprint = ($CertificateThumbprint -replace '\s','').ToUpperInvariant()
if ($thumbprint -notmatch '^[0-9A-F]{40}$') { throw 'A code-signing certificate thumbprint is required. No certificate is created or installed by this script.' }
$certificate = Get-Item -LiteralPath "Cert:\$Store\My\$thumbprint"
if (!$certificate.HasPrivateKey -or $certificate.NotBefore -gt (Get-Date) -or $certificate.NotAfter -le (Get-Date)) { throw 'Certificate has no accessible private key or is outside its validity period.' }
if ($certificate.Subject -eq $certificate.Issuer) { throw 'Self-signed publishing certificates are not accepted.' }
$usage = $certificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' } | Select-Object -First 1
if (!$usage) { throw 'Certificate has no explicit code-signing usage.' }
$enhanced = New-Object Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension
$enhanced.CopyFrom($usage)
if (@($enhanced.EnhancedKeyUsages | Where-Object {$_.Value -eq '1.3.6.1.5.5.7.3.3'}).Count -eq 0) { throw 'Certificate is not valid for code signing.' }
$chain = New-Object Security.Cryptography.X509Certificates.X509Chain
try { if (!$chain.Build($certificate)) { throw ('Untrusted signing certificate: ' + ($chain.ChainStatus.Status -join ', ')) } }
finally { $chain.Dispose() }
$sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$tool = Get-ChildItem -LiteralPath $sdk -Directory | Sort-Object Name -Descending | ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (!$tool) { throw 'Install Windows SDK SignTool before signing.' }
$report = @()
foreach ($file in $files) {
    if ($file.Extension -eq '.ps1') {
        $result = Set-AuthenticodeSignature -LiteralPath $file.FullName -Certificate $certificate -HashAlgorithm SHA256 -TimestampServer $TimestampUrl
        if ($result.Status -ne 'Valid') { throw "Script signing failed: $($file.FullName): $($result.StatusMessage)" }
    } else {
        $arguments = @('sign','/sha1',$thumbprint,'/s','My','/fd','SHA256','/tr',$TimestampUrl,'/td','SHA256','/d','Erdtree Tower','/du','https://github.com/tamikip/erdtree-tower')
        if ($Store -eq 'LocalMachine') { $arguments += '/sm' }
        & $tool @arguments $file.FullName
        if ($LASTEXITCODE -ne 0) { throw "Signing failed: $($file.FullName)" }
        & $tool verify /pa /all $file.FullName
        if ($LASTEXITCODE -ne 0) { throw "Signature verification failed: $($file.FullName)" }
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $file.FullName
    if ($signature.Status -ne 'Valid' -or !$signature.TimeStamperCertificate -or $signature.SignerCertificate.Thumbprint -ne $thumbprint) { throw "Signature or timestamp validation failed: $($file.FullName)" }
    $report += [pscustomobject]@{File=$file.FullName.Substring($root.Length).TrimStart('\');Status='Valid';Signer=$signature.SignerCertificate.Subject;Thumbprint=$thumbprint;Sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash}
}
$report | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'signing-report.json') -Encoding utf8
