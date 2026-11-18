using SoulsFormats;
using System.Text.Json;

static class SpiritBalanceAssets {
    record Spec(int Id,int Family,int Tier,int Count,string Name,string Info);
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    static void Set(PARAM.Row row,string field,int value)=>row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
    public static void Patch(string input,string output,string definitions=null) {
        var lines=File.ReadAllLines(Path.Combine(definitions??input,"custom-spirit-definitions.tsv"));
        var specs=lines.Select((line,family)=>{
            var p=line.Split('|');return new Spec(int.Parse(p[0]),family,int.Parse(p[4]),p.Length>10?int.Parse(p[10]):1,p[7],p[8]);
        }).Where(s=>s.Id>=282000).ToArray();
        if(specs.Length!=44||specs.Any(s=>s.Id!=270000+s.Family*1000||s.Tier is <1 or >3||s.Count is <1 or >3||s.Tier==3&&s.Count!=1))throw new Exception("Invalid spirit balance definitions");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var originals=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name) {
            var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,name+".xml")));return p;
        }
        var goods=Read("EquipParamGoods");var buddies=Read("BuddyParam");
        var g=goods.Rows.ToDictionary(r=>r.ID);var b=buddies.Rows.ToDictionary(r=>r.ID);
        var allowedGoods=new HashSet<int>();var allowedBuddies=new HashSet<int>();
        foreach(var s in specs)for(int level=0;level<=SpiritLevelAssets.Max;level++) {
            int buddyId=1600000000+s.Family*40000+level*1000;
            int trigger=610000000+s.Family*40000+level*1000;
            var source=b[buddyId];
            if(source.Name!=$"Tower custom spirit buddy {s.Id}+{level}"||I(source,"triggerSpEffectId")!=trigger||I(g[s.Id+level],"refId_default")!=trigger)throw new Exception("Unexpected custom summon reference");
            Set(g[s.Id+level],"rarity",s.Tier);allowedGoods.Add(s.Id+level);
            for(int slot=0;slot<s.Count;slot++) {
                int id=buddyId+slot;
                if(!b.TryGetValue(id,out var row)) {
                    row=new PARAM.Row(source){ID=id,Name=$"Tower custom spirit buddy {s.Id}+{level}/{slot}"};b.Add(id,row);buddies.Rows.Add(row);
                } else if(slot>0&&row.Name!=$"Tower custom spirit buddy {s.Id}+{level}/{slot}")throw new Exception($"Spirit group ID collision: {id}");
                if(s.Count>1) {
                    row["x_offset"].Value=(slot-(s.Count-1)/2f)*1.5f;
                    row["z_offset"].Value=slot==1&&s.Count==3?1.5f:0f;
                    allowedBuddies.Add(id);
                }
            }
            var group=b.Values.Where(r=>I(r,"triggerSpEffectId")==trigger).ToArray();
            if(group.Length!=s.Count||group.Any(r=>r.ID<buddyId||r.ID>=buddyId+s.Count))throw new Exception($"Unexpected summon count: {s.Id}+{level}");
            foreach(var row in group)foreach(var cell in source.Cells)
                if(cell.Def.InternalName is not ("x_offset" or "z_offset")&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,row[cell.Def.InternalName].Value))throw new Exception("Group members have different scaling or AI");
        }
        foreach(var (name,p) in new[]{("EquipParamGoods",goods),("BuddyParam",buddies)}) {
            var before=PARAM.Read(originals[File_(name).Name]);before.ApplyParamdef(p.AppliedParamdef);
            var rows=p.Rows.ToDictionary(r=>r.ID);
            foreach(var row in before.Rows)foreach(var cell in row.Cells) {
                if(name=="EquipParamGoods"&&allowedGoods.Contains(row.ID)&&cell.Def.InternalName=="rarity")continue;
                if(name=="BuddyParam"&&allowedBuddies.Contains(row.ID)&&cell.Def.InternalName is "x_offset" or "z_offset")continue;
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Unrelated field changed: {name}/{row.ID}/{cell.Def.InternalName}");
            }
            p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_(name).Bytes=p.Write();
        }
        Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files) {
            var expected=reg.Files.Single(r=>r.Name==f.Name).Bytes;
            if(!f.Bytes.SequenceEqual(expected))throw new Exception("Spirit balance serialization mismatch");
            if(f.Name!=File_("EquipParamGoods").Name&&f.Name!=File_("BuddyParam").Name&&!f.Bytes.SequenceEqual(originals[f.Name]))throw new Exception("Unrelated table changed");
        }
        var byId=specs.ToDictionary(s=>0x40000000u|(uint)s.Id);
        string Update(string line,bool detail) {
            var p=line.Split('\t');if(!byId.TryGetValue(uint.Parse(p[0]),out var s))return line;
            if(detail) {p[3]=s.Info;p[4]=s.Info+"\\n\\n可强化至 +39。每次召唤消耗一份骨灰。";}else p[1]=s.Tier.ToString();
            return string.Join('\t',p);
        }
        foreach(var name in new[]{"reward-rarities.tsv","reward-details.tsv"}) {
            var data=File.ReadAllLines(Path.Combine(input,name));
            foreach(var id in byId.Keys)if(data.Count(l=>l.StartsWith(id+"\t"))!=1)throw new Exception($"Missing/duplicate reward metadata: {id}");
            File.WriteAllLines(Path.Combine(output,name),data.Select(l=>Update(l,name=="reward-details.tsv")));
        }
        const string message="msg/zhocn/item_dlc02.msgbnd.dcx";
        var text=BND4.Read(Path.Combine(input,message));
        var textBefore=text.Files.ToDictionary(f=>f.ID,f=>f.Bytes.ToArray());
        var managed=specs.SelectMany(s=>Enumerable.Range(0,40).Select(level=>s.Id+level)).ToHashSet();
        var changedTables=new HashSet<int>();
        foreach(var table in new[]{"GoodsInfo","GoodsCaption"}) {
            var file=text.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name).Split("_dlc")[0]==table&&FMG.Read(f.Bytes).Entries.Any(e=>e.ID==200000));
            var fmg=FMG.Read(file.Bytes);var entries=fmg.Entries.ToDictionary(e=>e.ID);
            foreach(var s in specs)for(int level=0;level<40;level++) {
                string value=s.Info+(table=="GoodsCaption"?"\n\n黄金树爬塔的召唤骨灰。可强化至 +39。\n每次召唤消耗一份骨灰；召唤师在胜利时按存活情况返还。":"");
                if(entries.TryGetValue(s.Id+level,out var entry))entry.Text=value;else fmg.Entries.Add(new FMG.Entry(s.Id+level,value));
            }
            foreach(var e in FMG.Read(textBefore[file.ID]).Entries)if(!managed.Contains(e.ID)&&entries[e.ID].Text!=e.Text)throw new Exception("Unrelated spirit text changed");
            file.Bytes=fmg.Write();changedTables.Add(file.ID);
        }
        var textPath=Path.Combine(output,message);Directory.CreateDirectory(Path.GetDirectoryName(textPath)!);text.Write(textPath);
        foreach(var f in BND4.Read(textPath).Files)if(!f.Bytes.SequenceEqual(changedTables.Contains(f.ID)?text.Files.Single(t=>t.ID==f.ID).Bytes:textBefore[f.ID]))throw new Exception("Spirit text serialization mismatch");
        File.WriteAllLines(Path.Combine(output,"custom-spirit-definitions.tsv"),lines);
        File.WriteAllText(Path.Combine(output,"spirit-balance.json"),JsonSerializer.Serialize(specs,new JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"PASS: {specs.Length} spirit families x 40 levels; group counts, AI/scaling, rarity, descriptions and unrelated fields verified");
    }
}
