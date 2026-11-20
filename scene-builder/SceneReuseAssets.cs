using SoulsFormats;

static class SceneReuseAssets {
    public const uint Request=45007890,Cleared=45007892;
    static bool WaitRequest(EMEVD.Instruction i)=>i.Bank==3&&i.ID==0&&i.ArgData.Length==8&&BitConverter.ToUInt32(i.ArgData,4)==Request;
    public static bool Armed(EMEVD.Event e)=>e.Instructions.Any(WaitRequest);
    public static List<EMEVD.Instruction> BaseInstructions(EMEVD.Event e) {
        var list=e.Instructions.ToList();int at=list.FindIndex(WaitRequest);
        if(at>=0){list.RemoveRange(at,list.Count-at);list.Add(new(1000,4,new object[]{(byte)0}));}
        return list;
    }
    public static void Strip(EMEVD.Event e) {var list=BaseInstructions(e);e.Instructions.Clear();e.Instructions.AddRange(list);}
    static void Wait(EMEVD.Event e,uint flag,bool on)=>e.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)(on?1:0),(byte)0,flag}));
    static void Flag(EMEVD.Event e,uint flag,bool on)=>e.Instructions.Add(new(2003,66,new object[]{(byte)0,flag,(byte)(on?1:0)}));
    public static void Apply(EMEVD events,uint portal) {
        var visual=PortalReveal.Visual(events,portal);PersonalEvents.Verify(visual);Strip(visual);
        if(visual.Instructions.Last() is not {Bank:1000,ID:4})throw new Exception("Unexpected portal terminator");
        visual.Instructions.RemoveAt(visual.Instructions.Count-1);
        Wait(visual,Request,true);
        foreach(uint id in Enumerable.Range(0,PortalReveal.Steps).Select(i=>PortalReveal.StepEntity(portal,i))
            .Concat(new[]{portal,PortalReveal.ArchEntity(portal),PortalReveal.BorderEntity(portal),PortalReveal.RuneEntity(portal)})) {
            visual.Instructions.Add(new(2006,5,new object[]{id,1}));
            visual.Instructions.Add(new(2005,3,new object[]{id,(byte)0}));
        }
        Flag(visual,Cleared,true);Wait(visual,Request,false);
        visual.Instructions.Add(new(1000,4,new object[]{(byte)1}));
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        int count=0;var arrivals=new List<string>();
        foreach(var map in File.ReadAllLines(Path.Combine(input,"arenas-built.txt")).Where(l=>!l.StartsWith('#')).Select(l=>l.Split('|')).Where(p=>p.Length>=13&&p[12]=="1").Select(p=>p[1]).Distinct()) {
            string relative=$"event/{map}.emevd.dcx";var events=EMEVD.Read(Path.Combine(input,relative));
            var msb=MSBE.Read(Path.Combine(input,$"map/mapstudio/{map}.msb.dcx"));
            var at=msb.Regions.GetEntries().Single(r=>r.EntityID==45022951).Position;
            arrivals.Add(FormattableString.Invariant($"{map}|{at.X:R}|{at.Y:R}|{at.Z:R}"));
            var before=events.Events.ToDictionary(e=>e.ID,e=>new EMEVD{Events=[e]}.Write());
            Apply(events,45021900);var once=events.Write();Apply(events,45021900);
            if(!once.SequenceEqual(events.Write()))throw new Exception("Scene reset is not idempotent");
            foreach(var e in events.Events.Where(e=>e.ID!=45020986))
                if(!before[e.ID].SequenceEqual(new EMEVD{Events=[e]}.Write()))throw new Exception("Unrelated event changed");
            var dest=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);events.Write(dest);
            if(!EMEVD.Read(dest).Write().SequenceEqual(once))throw new Exception("Scene reset roundtrip mismatch");
            Console.WriteLine($"PASS {map}: local entry relocation, portal cleanup acknowledgement and repeatable reveal");count++;
        }
        if(count==0)throw new Exception("No arena events found");
        File.WriteAllLines(Path.Combine(output,"combat-arrivals.txt"),arrivals);
        MusicAssets.RefreshArenaManifest(input,output);
    }
}
