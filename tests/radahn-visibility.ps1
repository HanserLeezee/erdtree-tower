param(
    [Parameter(Mandatory=$true)][string]$Game,
    [string]$Assets = "$PSScriptRoot/../scene-assets"
)
$ErrorActionPreference = 'Stop'
Add-Type -Path "$PSScriptRoot/../scene-builder/bin/Release/net10.0/SoulsFormats.dll"
[SoulsFormats.Oodle]::Oodle6Ptr = [System.Runtime.InteropServices.NativeLibrary]::Load((Join-Path $Game 'oo2core_6_win64.dll'))
$count = 0
foreach ($file in Get-ChildItem (Join-Path $Assets 'map/mapstudio') -Filter '*.msb.dcx') {
    $map = [SoulsFormats.MSBE]::Read($file.FullName)
    foreach ($actor in $map.Parts.Enemies | Where-Object { $_.EntityID -in 48020298,48020299,51020298,51020299 }) {
        $tile = $actor.TileLoad
        if ($actor.ModelName -ne 'c4730' -or @($tile.MapID | Where-Object { $_ -ne 255 }).Count -gt 0 -or
            $tile.Unk04 -ne 0 -or $tile.Unk0C -ne -1 -or $tile.Unk10 -ne 0 -or $tile.CullingHeightBehavior -ne -1) {
            throw "Radahn retains foreign map loading/culling: $($file.Name) entity=$($actor.EntityID)"
        }
        $count++
    }
}
if ($count -lt 14) { throw "Incomplete Radahn resource coverage: $count actors" }
Write-Output "Verified local loading/culling for $count Radahn actors."
