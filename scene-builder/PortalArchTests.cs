using SoulsFormats;
using System.Numerics;

static class PortalArchTests {
    public static void Test() {
        const uint portal=45021900;
        var oldCenter=new Vector3(14.45f,1009.706f,329.504f);
        var aligned=PortalReveal.Position("farum_dragonlord",oldCenter);
        if(aligned!=new Vector3(15.0099f,oldCenter.Y,330.5107f)||PortalReveal.Position("farum_dragonlord",aligned)!=aligned)
            throw new Exception("Dragonlord portal must use the geometric center without moving vertically");
        if(PortalReveal.Position("raya_library",oldCenter)!=oldCenter)throw new Exception("Other portal positions changed");
        var map=new MSBE();
        map.Models.Assets.Add(new MSBE.Model.Asset{Name="AEG099_510"});
        map.Parts.Assets.Add(new MSBE.Part.Asset{Name="portal",ModelName="AEG099_510",EntityID=portal,Position=new(3,2,7),Rotation=new(0,43,0)});
        PortalReveal.AddRegion(map,portal);
        var copy=MSBE.Read(map.Write());
        var carrier=copy.Parts.Assets.Single(a=>a.EntityID==portal);
        if(carrier.ModelName!="AEG099_001")throw new Exception("Expected native fog carrier at the original interaction entity");
        var ring=copy.Parts.Assets.Single(a=>a.EntityID==portal+2101);
        var rune=copy.Parts.Assets.Single(a=>a.EntityID==portal+2102);
        if(rune.Position!=carrier.Position+new Vector3(0,1.15f,0)||rune.Scale!=new Vector3(1.3f)||rune.ModelName!="AEG099_991")throw new Exception("Rune must be enlarged and lowered into the doorway");
        if(ring.Rotation.X!=90||ring.Rotation.Y!=carrier.Rotation.Y||ring.ModelName!="AEG099_001")throw new Exception("Inner flow must be upright in the doorway plane");
        if(copy.Regions.SFX.Any(r=>r.EntityID==PortalReveal.RingRegion(portal)))throw new Exception("Obsolete upright ground-ring effect remains");
        var arch=copy.Parts.Assets.Single(a=>a.EntityID==PortalReveal.ArchEntity(portal));
        if(arch.ModelName!=PortalReveal.ArchModel||arch.Position!=carrier.Position||arch.Rotation!=carrier.Rotation||arch.Scale!=PortalReveal.DoorScale)
            throw new Exception("Reward portal frame is missing or misplaced");
        if(copy.Parts.Assets.Count(a=>Enumerable.Range(0,PortalReveal.Steps).Any(s=>a.EntityID==PortalReveal.StepEntity(portal,s)))!=PortalReveal.Steps)
            throw new Exception("Portal rise frames missing");
        if(copy.Regions.SFX.Any(r=>r.EntityID==PortalReveal.RiseRegion(portal)))throw new Exception("Reward portal still contains ground light");
        if(copy.Parts.Assets.Any(a=>a.ModelName=="AEG099_510"))throw new Exception("Old small portal still visible");
        PortalReveal.Test();
        Console.WriteLine("Effect-only portal, transforms, roundtrip and lifecycle passed");
    }
}
