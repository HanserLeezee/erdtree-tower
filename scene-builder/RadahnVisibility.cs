using SoulsFormats;

static class RadahnVisibility {
    static bool Target(MSBE.Part.Enemy actor)=>actor.EntityID is 48020298 or 48020299 or 51020298 or 51020299;
    public static void Prepare(MSBE.Part.Enemy actor) {
        if(!Target(actor)||actor.ModelName!="c4730")throw new Exception("Unexpected Radahn actor");
        actor.TileLoad=new MSBE.Part.TileLoadConfig{MapID=[255,255,255,255],Unk04=0,Unk0C=-1,Unk10=0,CullingHeightBehavior=-1};
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        int count=0,maps=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")) {
            var map=MSBE.Read(path);
            var actors=map.Parts.Enemies.Where(Target).ToArray();
            if(actors.Length==0)continue;
            var before=map.Write();
            var original=actors.ToDictionary(a=>a.EntityID,a=>a.TileLoad.DeepCopy());
            foreach(var actor in actors)Prepare(actor);
            var dest=Path.Combine(output,"map/mapstudio",Path.GetFileName(path));
            Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
            map.Write(dest);
            var check=MSBE.Read(dest);
            foreach(var actor in check.Parts.Enemies.Where(Target)) {
                var tile=actor.TileLoad;
                if(tile.MapID.Any(b=>b!=255)||tile.Unk04!=0||tile.Unk0C!=-1||tile.Unk10!=0||tile.CullingHeightBehavior!=-1)
                    throw new Exception("Radahn local loading did not survive roundtrip");
                actor.TileLoad=original[actor.EntityID];
            }
            if(!before.SequenceEqual(check.Write()))throw new Exception("Unrelated map data changed");
            count+=actors.Length;maps++;
        }
        if(count<14)throw new Exception("Incomplete Radahn coverage");
        MusicAssets.RefreshArenaManifest(input,output);
        Console.WriteLine($"Fixed local loading/culling for {count} Radahn actors in {maps} maps; unrelated map data preserved");
    }
}
