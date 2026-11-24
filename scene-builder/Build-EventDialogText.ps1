param(
    [Parameter(Mandatory)][string]$InputAssets,
    [Parameter(Mandatory)][string]$OutputAssets,
    [Parameter(Mandatory)][string]$GameDir,
    [Parameter(Mandatory)][string]$TextJson
)
$ErrorActionPreference='Stop'
$source=(Resolve-Path -LiteralPath $InputAssets).Path
$target=[IO.Path]::GetFullPath($OutputAssets)
if($source -eq $target){throw 'Use a separate staging directory.'}
Add-Type -Path (Join-Path $PSScriptRoot 'bin/Release/net10.0/SoulsFormats.dll')
[SoulsFormats.Oodle]::Oodle6Ptr=[Runtime.InteropServices.NativeLibrary]::Load((Join-Path $GameDir 'oo2core_6_win64.dll'))
$rows=Get-Content -LiteralPath $TextJson -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
$langs=@('zhocn','zhotw','engus')
for($i=0;$i -lt $langs.Length;$i++){
    $relative="msg/$($langs[$i])/menu_dlc02.msgbnd.dcx"
    $binder=[SoulsFormats.BND4]::Read((Join-Path $source $relative))
    $file=$binder.Files.Where({$_.ID -eq 33})[0]
    $table=[SoulsFormats.FMG]::Read($file.Bytes)
    $byId=@{}
    foreach($entry in $table.Entries){$byId[$entry.ID]=$entry}
    foreach($key in $rows.Keys){
        $id=[int]$key
        $text=$rows[$key][$i]
        if([string]::IsNullOrWhiteSpace($text)){throw "Missing text $id"}
        if($byId.ContainsKey($id)){
            if($byId[$id].Text -cne $text){throw "Dialogue text ID collision in $relative at $id"}
        }
        else{$table.Entries.Add([SoulsFormats.FMG+Entry]::new($id,$text))}
    }
    $table.Entries.Sort([Comparison[SoulsFormats.FMG+Entry]]{param($a,$b) $a.ID.CompareTo($b.ID)})
    $file.Bytes=$table.Write()
    $dest=Join-Path $target $relative
    [IO.Directory]::CreateDirectory((Split-Path $dest))|Out-Null
    $binder.Write($dest)
    $check=[SoulsFormats.BND4]::Read($dest)
    for($j=0;$j -lt $binder.Files.Count;$j++){
        if(![Linq.Enumerable]::SequenceEqual[byte]($binder.Files[$j].Bytes,$check.Files[$j].Bytes)){throw "Round-trip mismatch $relative"}
    }
    Write-Output "$relative verified ($($rows.Count) dialogue entries)"
}
