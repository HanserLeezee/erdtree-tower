using SoulsFormats;

static class DanceStageAssets {
    const uint Flag=45011580;
    const string MapPath="map/mapstudio/m45_01_00_00.msb.dcx",EventPath="event/m45_01_00_00.emevd.dcx";
    public static void Patch(string input,string output){
        var map=MSBE.Read(Path.Combine(input,MapPath));var events=EMEVD.Read(Path.Combine(input,EventPath));
        foreach(var part in map.Parts.Assets.Where(a=>a.Name.StartsWith("tower_dance_")).ToArray())map.Parts.Assets.Remove(part);
        var init=events.Events.Single(e=>e.ID==0);
        init.Instructions.RemoveAll(i=>i.Bank==2000&&i.ID==0&&i.ArgData.Length>=8&&BitConverter.ToUInt32(i.ArgData,4)==Flag);
        events.Events.RemoveAll(e=>e.ID==Flag);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,MapPath));events.Write(Path.Combine(output,EventPath));
        if(MSBE.Read(Path.Combine(output,MapPath)).Parts.Assets.Any(p=>p.Name.StartsWith("tower_dance_")))throw new Exception("Stage removal failed");
        if(EMEVD.Read(Path.Combine(output,EventPath)).Events.Any(e=>e.ID==Flag))throw new Exception("Stage event removal failed");
        MusicAssets.RefreshArenaManifest(input,output);
        Console.WriteLine("Dance stage scenery and visibility event removed; roundtrip verified");
    }
}
