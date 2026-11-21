using SoulsFormats;

static class GodskinArenaAssets {
    const uint Controller=48030242,Apostle=48035241,Noble=48035242,Generator=48036241,Event=48037240;
    public static void RemoveLegacy(string input,string output) {
        int maps=0;
        foreach(var sourcePath in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")){
            var path=Path.Combine(output,"map/mapstudio",Path.GetFileName(sourcePath));
            if(!File.Exists(path)) {
                if(!MSBE.Read(sourcePath).Parts.Enemies.Any(e=>e.EntityID==Controller))continue;
                Directory.CreateDirectory(Path.GetDirectoryName(path)!);File.Copy(sourcePath,path);
                var relative=Path.Combine("event",Path.GetFileName(path).Replace(".msb.",".emevd."));
                Directory.CreateDirectory(Path.Combine(output,"event"));File.Copy(Path.Combine(input,relative),Path.Combine(output,relative),true);
            }
            var map=MSBE.Read(path);var anchor=map.Parts.Enemies.SingleOrDefault(p=>p.EntityID==Controller);if(anchor==null)continue;
            var eventPath=Path.Combine(output,"event",Path.GetFileName(path).Replace(".msb.",".emevd."));var script=EMEVD.Read(eventPath);
            foreach(var id in new[]{Apostle,Noble})map.Parts.Enemies.RemoveAll(p=>p.EntityID==id&&p.Name=="tower_godskin_"+id);
            map.Events.Generators.RemoveAll(g=>g.EntityID>=Generator&&g.EntityID<=Generator+2&&g.Name=="tower_godskin_generator_"+(g.EntityID-Generator));
            var init=script.Events.Single(e=>e.ID==0);
            if(init.Instructions.RemoveAll(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4) is >=Event and <=Event+3)>0)
                script.Events.RemoveAll(e=>e.ID>=Event&&e.ID<=Event+3);
            map.Regions.InvasionPoints.RemoveAll(r=>r.EntityID==48039242&&r.Name=="tower_godskin_health");
            map.Write(path);script.Write(eventPath);
            if(MSBE.Read(path).Events.Generators.Any(g=>g.EntityID>=Generator&&g.EntityID<=Generator+2)||
                EMEVD.Read(eventPath).Events.Any(e=>e.ID>=Event&&e.ID<=Event+3))throw new Exception("Godskin respawn still present");
            maps++;
        }
        if(maps==0)throw new Exception("No act two maps");MusicAssets.RefreshArenaManifest(input,output);Console.WriteLine($"Verified removal of Godskin generators and battle events in {maps} arenas");
    }
}
