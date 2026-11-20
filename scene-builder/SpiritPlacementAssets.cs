using SoulsFormats;
using System.Numerics;
using System.Text.Json;

static class SpiritPlacementAssets {
    const float PlayerMargin=0.75f,PeerMargin=0.5f,Tolerance=0.0001f;
    static float F(PARAM.Row row,string field)=>Convert.ToSingle(row[field].Value);
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    static bool Clear(Vector2[] points,float radius) {
        for(int i=0;i<points.Length;i++) {
            if(!float.IsFinite(points[i].X)||!float.IsFinite(points[i].Y)||points[i].Length()+Tolerance<radius+PlayerMargin)return false;
            for(int j=0;j<i;j++)if(Vector2.Distance(points[i],points[j])+Tolerance<2*radius+PeerMargin)return false;
        }
        return true;
    }
    static Vector2[] Layout(Vector2[] current,float radius) {
        if(!float.IsFinite(radius)||radius<=0||current.Length is <1 or >3)throw new Exception("Unsupported summon geometry");
        if(Clear(current,radius))return current;
        float distance=radius+PlayerMargin;
        if(current.Length>1)distance=Math.Max(distance,(2*radius+PeerMargin)/(2*MathF.Sin(MathF.PI/current.Length)));
        var points=Enumerable.Range(0,current.Length).Select(i=>new Vector2(MathF.Cos(i*2*MathF.PI/current.Length),MathF.Sin(i*2*MathF.PI/current.Length))*distance).ToArray();
        if(!Clear(points,radius))throw new Exception("Summon geometry failed clearance check");
        return points;
    }
    public static void Test() {
        foreach(float radius in new[]{0.3f,0.8f,1.4f,2.25f,4.4f,7f,12f})for(int count=1;count<=3;count++) {
            var old=Enumerable.Range(0,count).Select(i=>new Vector2((i-(count-1)/2f)*1.5f,count==3&&i==1?1.5f:0)).ToArray();
            var next=Layout(old,radius);
            if(!Clear(next,radius)||!Layout(next,radius).SequenceEqual(next))throw new Exception("Placement regression");
        }
        var safe=new[]{new Vector2(4.5f,0)};
        if(!ReferenceEquals(Layout(safe,2.25f),safe))throw new Exception("Valid sphere layout changed");
        Console.WriteLine("PASS: small/large/giant bodies, 1-3 units, player/peer clearance and idempotency");
    }
    public static void Patch(string input,string output,bool verify=false) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var original=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name){var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,name+".xml")));return p;}
        var npc=Read("NpcParam").Rows.ToDictionary(r=>r.ID);var buddies=Read("BuddyParam");
        var groups=buddies.Rows.GroupBy(r=>I(r,"triggerSpEffectId")).ToDictionary(g=>g.Key,g=>g.OrderBy(r=>r.ID).ToArray());
        var changed=new HashSet<int>();var report=new List<object>();int checkedGroups=0;
        var lines=File.ReadAllLines(Path.Combine(input,"custom-spirit-definitions.tsv"));
        foreach(var spec in lines.Select((line,index)=>(p:line.Split('|'),index))) {
            var (p,index)=spec;int goods=int.Parse(p[0]),id=100000000+int.Parse(p[1])*10000+8000+index;
            var n=npc[id];float radius=Math.Max(F(n,"hitRadius"),F(n,"chrHitRadius"));bool familyChanged=false;
            for(int level=0;level<40;level++) {
                int trigger=610000000+index*40000+level*1000;
                if(!groups.TryGetValue(trigger,out var group)||group.Any(r=>I(r,"npcParamId")!=id||r.Name?.StartsWith($"Tower custom spirit buddy {goods}+{level}")!=true))throw new Exception($"Unexpected summon group: {goods}+{level}");
                var old=group.Select(r=>new Vector2(F(r,"x_offset"),F(r,"z_offset"))).ToArray();
                var next=Layout(old,radius);checkedGroups++;
                if(old.SequenceEqual(next))continue;
                if(verify)throw new Exception($"Overlapping summon layout: {goods}+{level}");
                for(int slot=0;slot<group.Length;slot++) {
                    group[slot]["x_offset"].Value=next[slot].X;group[slot]["z_offset"].Value=next[slot].Y;changed.Add(group[slot].ID);
                }
                if(!familyChanged){report.Add(new {goods,name=p[7],radius,count=group.Length,before=old.Select(v=>new[]{v.X,v.Y}),after=next.Select(v=>new[]{v.X,v.Y})});familyChanged=true;}
            }
        }
        if(verify){Console.WriteLine($"PASS: {checkedGroups} groups have player and peer clearance");return;}
        var before=PARAM.Read(original[File_("BuddyParam").Name]);before.ApplyParamdef(buddies.AppliedParamdef);
        var byId=buddies.Rows.ToDictionary(r=>r.ID);
        if(before.Rows.Count!=byId.Count)throw new Exception("Summon count changed");
        foreach(var row in before.Rows)foreach(var cell in row.Cells) {
            if(changed.Contains(row.ID)&&cell.Def.InternalName is "x_offset" or "z_offset")continue;
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,byId[row.ID][cell.Def.InternalName].Value))throw new Exception("Unrelated summon field changed");
        }
        File_("BuddyParam").Bytes=buddies.Write();Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        foreach(var file in SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path).Files)
            if(!file.Bytes.SequenceEqual(file.Name==File_("BuddyParam").Name?File_("BuddyParam").Bytes:original[file.Name]))throw new Exception("Serialization or unrelated table mismatch");
        File.WriteAllLines(Path.Combine(output,"custom-spirit-definitions.tsv"),lines);
        File.WriteAllText(Path.Combine(output,"spirit-placement-report.json"),JsonSerializer.Serialize(report,new JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"PASS: checked {checkedGroups} groups; repaired {report.Count} families / {changed.Count} buddy rows; only x/z spawn offsets changed");
    }
}
