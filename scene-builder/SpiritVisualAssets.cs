using SoulsFormats;

static class SpiritVisualAssets {
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    static void Set(PARAM.Row row,string field,int value)=>row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
    public static void Patch(string input,string output,bool verify=false) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var original=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name) {
            var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,(name=="SpEffectParam"?"SpEffect":name=="SpEffectVfxParam"?"SpEffectVfx":name)+".xml")));return p;
        }
        var npc=Read("NpcParam");var effects=Read("SpEffectParam");var vfx=Read("SpEffectVfxParam");var phantoms=Read("PhantomParam");
        var e=effects.Rows.ToDictionary(r=>r.ID);var v=vfx.Rows.ToDictionary(r=>r.ID);
        const int phantom=205,legacyPhantom=9681000;
        var nativeVisual=v[I(e[295000],"vfxId")];
        if(I(nativeVisual,"phantomParamOverwriteId") is not (200 or legacyPhantom or phantom))throw new Exception("Native spirit shader changed");
        var managed=File.ReadAllLines(Path.Combine(input,"custom-spirit-definitions.tsv")).Select((line,family)=>100000000+int.Parse(line.Split('|')[1])*10000+8000+family).ToHashSet();
        if(managed.Count<56||npc.Rows.Count(r=>managed.Contains(r.ID)&&r.Name?.StartsWith("Tower custom spirit ")==true)!=managed.Count)throw new Exception("Custom spirit NPC set incomplete");
        if(!verify) {
            foreach(var row in vfx.Rows.Where(r=>r.Name?.StartsWith("Tower spirit visual ")==true&&I(r,"phantomParamOverwriteId")==legacyPhantom))
                Set(row,"phantomParamOverwriteId",phantom);
        }
        var replacements=new Dictionary<int,int>{{14495,9681000},{20011320,9681001}};
        // 其余带着自己幻影着色的继承效果（Boss 骨灰带进来的）按编号依次排在后面，已有的两个编号不变
        bool Conflicting(PARAM.Row effect)=>effect.Cells.Where(c=>c.Def.InternalName.StartsWith("vfxId"))
            .Any(c=>v.TryGetValue(Convert.ToInt32(c.Value),out var visual)&&I(visual,"phantomParamOverwriteType")!=0&&I(visual,"phantomParamOverwriteId")!=phantom);
        if(!verify)foreach(int source in npc.Rows.Where(r=>managed.Contains(r.ID)).SelectMany(r=>Enumerable.Range(0,32).Select(slot=>I(r,$"spEffectID{slot}")))
            .Where(id=>id!=295000&&!replacements.ContainsKey(id)&&!replacements.ContainsValue(id)&&e.TryGetValue(id,out var effect)&&Conflicting(effect)).Distinct().OrderBy(id=>id))
            replacements[source]=9681000+replacements.Count;
        if(replacements.Count>32)throw new Exception("Too many inherited spirit visual effects");
        void Put(PARAM table,PARAM.Row row) {
            var existing=table.Rows.SingleOrDefault(r=>r.ID==row.ID);
            if(existing!=null&&existing.Name!=row.Name)throw new Exception($"Visual ID collision: {row.ID}");
            if(existing!=null)table.Rows.Remove(existing);
            table.Rows.Add(row);
        }
        // 原版 PhantomParam 全表编号都在 1..999 内，7 位编号 9681000 会导致引擎查不到灵体着色器。
        // 改用未占用的三位编号 205，并调低 alpha / diffMulColorA 呈现明显的白色半透明骨灰质感。
        var silver=new PARAM.Row(phantoms.Rows.Single(r=>r.ID==200)){ID=phantom,Name="Tower silver spirit appearance"};
        Set(silver,"edgeColorR",235);Set(silver,"edgeColorG",245);Set(silver,"edgeColorB",255);
        Set(silver,"frontColorR",200);Set(silver,"frontColorG",215);Set(silver,"frontColorB",235);
        silver["edgeColorA"].Value=0.55f;silver["frontColorA"].Value=0.25f;
        silver["diffMulColorA"].Value=0.55f;silver["alpha"].Value=0.55f;
        silver["edgePower"].Value=0.75f;silver["glowScale"].Value=1.8f;
        if(!verify) {
            phantoms.Rows.RemoveAll(r=>r.ID==legacyPhantom&&r.Name=="Tower silver spirit appearance");
            Put(phantoms,silver);Set(nativeVisual,"phantomParamOverwriteId",phantom);
        }
        if(phantoms.Rows.Any(r=>r.ID==legacyPhantom&&r.Name=="Tower silver spirit appearance"))throw new Exception("Legacy out-of-range PhantomParam row still present");
        var actualSilver=phantoms.Rows.Single(r=>r.ID==phantom);
        if(I(nativeVisual,"phantomParamOverwriteId")!=phantom||silver.Cells.Any(c=>!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,actualSilver[c.Def.InternalName].Value)))throw new Exception("Silver spirit material missing");
        if(!verify)foreach(var (source,id) in replacements) {
            var effect=new PARAM.Row(e[source]){ID=id,Name=$"Tower spirit visual effect {source}"};
            bool changed=false;
            for(int slot=0;slot<8;slot++) {
                string field=slot==0?"vfxId":$"vfxId{slot}";
                if(!v.TryGetValue(I(effect,field),out var visual)||I(visual,"phantomParamOverwriteType")==0||I(visual,"phantomParamOverwriteId")==phantom)continue;
                int visualId=9681000+(id-9681000)*8+slot;
                var clone=new PARAM.Row(visual){ID=visualId,Name=$"Tower spirit visual {source}/{slot}"};
                Set(clone,"phantomParamOverwriteId",phantom);Put(vfx,clone);Set(effect,field,visualId);changed=true;
            }
            if(!changed)throw new Exception($"Expected inherited visual override missing: {source}");
            Put(effects,effect);
        }
        e=effects.Rows.ToDictionary(r=>r.ID);v=vfx.Rows.ToDictionary(r=>r.ID);
        foreach(var row in npc.Rows.Where(r=>managed.Contains(r.ID))) {
            if(!verify) {
                Set(row,"phantomShaderId",phantom);
                for(int slot=0;slot<32;slot++)if(replacements.TryGetValue(I(row,$"spEffectID{slot}"),out int id))Set(row,$"spEffectID{slot}",id);
            }
            if(I(row,"phantomShaderId")!=phantom||Enumerable.Range(0,32).Any(slot=>replacements.ContainsKey(I(row,$"spEffectID{slot}"))))throw new Exception($"Missing spirit appearance: {row.ID}");
            foreach(int slot in Enumerable.Range(0,32))if(e.TryGetValue(I(row,$"spEffectID{slot}"),out var effect))
                foreach(var cell in effect.Cells.Where(c=>c.Def.InternalName.StartsWith("vfxId")))
                    if(v.TryGetValue(Convert.ToInt32(cell.Value),out var visual)&&I(visual,"phantomParamOverwriteType")!=0&&I(visual,"phantomParamOverwriteId")!=phantom)throw new Exception($"Conflicting spirit shader: {row.ID}/{effect.ID}");
        }
        if(verify) {Console.WriteLine($"PASS: all {managed.Count} custom summon NPCs use spirit shader {phantom}; conflicting inherited effects replaced");return;}
        foreach(var (name,table) in new[]{("NpcParam",npc),("SpEffectParam",effects),("SpEffectVfxParam",vfx),("PhantomParam",phantoms)}) {
            var before=PARAM.Read(original[File_(name).Name]);before.ApplyParamdef(table.AppliedParamdef);var rows=table.Rows.ToDictionary(r=>r.ID);
            foreach(var row in before.Rows) {
                if(name=="PhantomParam"&&row.ID==legacyPhantom&&row.Name=="Tower silver spirit appearance")continue;
                if(name=="PhantomParam"&&row.ID==phantom&&row.Name=="Tower silver spirit appearance")continue;
                var afterRow=rows[row.ID];
                for(int ci=0;ci<row.Cells.Count;ci++) {
                    var cell=row.Cells[ci];
                    var expected=cell.Value;
                    if(name=="SpEffectVfxParam"&&row.ID==nativeVisual.ID&&cell.Def.InternalName=="phantomParamOverwriteId")expected=Convert.ChangeType(phantom,expected.GetType());
                    if(name=="SpEffectVfxParam"&&row.Name?.StartsWith("Tower spirit visual ")==true&&cell.Def.InternalName=="phantomParamOverwriteId")expected=Convert.ChangeType(phantom,expected.GetType());
                    if(name=="NpcParam"&&managed.Contains(row.ID)) {
                        if(cell.Def.InternalName=="phantomShaderId")expected=Convert.ChangeType(phantom,expected.GetType());
                        if(cell.Def.InternalName.StartsWith("spEffectID")&&replacements.TryGetValue(Convert.ToInt32(expected),out int id))expected=Convert.ChangeType(id,expected.GetType());
                    }
                    if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(expected,afterRow.Cells[ci].Value))throw new Exception($"Unrelated field changed: {name}/{row.ID}/{cell.Def.InternalName}");
                }
            }
            table.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_(name).Bytes=table.Write();
        }
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        foreach(var file in SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path).Files)
            if(!file.Bytes.SequenceEqual(reg.Files.Single(f=>f.Name==file.Name).Bytes))throw new Exception("Spirit visual serialization mismatch");
        Console.WriteLine($"PASS: {managed.Count} custom spirits patched; enemy NPCs, original effects, combat properties and other tables preserved");
    }
    public static void Inspect(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        PARAM Read(string name) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,(name=="SpEffectParam"?"SpEffect":name=="SpEffectVfxParam"?"SpEffectVfx":name)+".xml")));return p;
        }
        var npc=Read("NpcParam");
        foreach(var mapPath in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")) {
            var map=MSBE.Read(mapPath);
            if(Path.GetFileName(mapPath).StartsWith("m45")||map.Models.Enemies.Any(m=>m.Name=="c4420"))Console.WriteLine($"Lobster map {Path.GetFileName(mapPath)}: model={map.Models.Enemies.Any(m=>m.Name=="c4420")} parts={map.Parts.Enemies.Count(p=>p.ModelName=="c4420")}");
        }
        foreach(var row in npc.Rows.Where(r=>r.ID==121000000||r.Name?.StartsWith("Tower custom spirit ")==true).Take(7))
            Console.WriteLine($"{row.ID} {row.Name}: "+string.Join(", ",row.Cells.Where(c=>c.Def.InternalName.Contains("phantom",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.Contains("ghost",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.StartsWith("spEffectID")).Select(c=>$"{c.Def.InternalName}={c.Value}")));
        var buddies=Read("BuddyParam");
        var effects=Read("SpEffectParam");var vfx=Read("SpEffectVfxParam");
        foreach(var r in Read("PhantomParam").Rows.Where(r=>r.ID==200))Console.WriteLine("Phantom 200: "+string.Join(", ",r.Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
        Console.WriteLine("Effect flags: "+string.Join(", ",effects.Rows.Single(r=>r.ID==295000).Cells.Where(c=>c.Def.InternalName.Contains("target",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.Contains("condition",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.Contains("category",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.Contains("Time")||c.Def.InternalName=="stateInfo").Select(c=>$"{c.Def.InternalName}={c.Value}")));
        foreach(var n in npc.Rows.Where(r=>r.Name?.StartsWith("Tower custom spirit ")==true)) {
            var fx=n.Cells.Where(c=>c.Def.InternalName.StartsWith("spEffectID")).Select(c=>Convert.ToInt32(c.Value)).ToHashSet();
            foreach(var e in effects.Rows.Where(e=>e.ID!=295000&&fx.Contains(e.ID)))foreach(var v in vfx.Rows.Where(v=>Convert.ToInt32(v["phantomParamOverwriteType"].Value)!=0&&e.Cells.Any(c=>c.Def.InternalName.StartsWith("vfxId")&&Convert.ToInt32(c.Value)==v.ID)))
                Console.WriteLine($"Override {n.Name}: effect={e.ID} vfx={v.ID} type={v["phantomParamOverwriteType"].Value} phantom={v["phantomParamOverwriteId"].Value}");
        }
        foreach(var id in new[]{4302,5400,5800,295000,296000,297000,297200,20297000,20297740}) {
            var r=effects.Rows.Single(r=>r.ID==id);
            Console.WriteLine($"Effect {id}: "+string.Join(", ",r.Cells.Where(c=>c.Def.InternalName.Contains("vfx",StringComparison.OrdinalIgnoreCase)).Select(c=>$"{c.Def.InternalName}={c.Value}")));
            foreach(var v in vfx.Rows.Where(v=>r.Cells.Any(c=>c.Def.InternalName.Contains("vfx",StringComparison.OrdinalIgnoreCase)&&Convert.ToInt32(c.Value)>0&&Convert.ToInt32(c.Value)==v.ID)))
                Console.WriteLine($"VFX {v.ID}: "+string.Join(", ",v.Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
        }
        foreach(var row in buddies.Rows.Where(r=>r.ID<21000000).Take(8)) {
            var id=Convert.ToInt32(row["npcParamId"].Value);var n=npc.Rows.FirstOrDefault(r=>r.ID==id);if(n==null)continue;
            Console.WriteLine($"Native {row.ID} npc={id}: isGhost={n["isGhost"].Value}, phantomShaderId={n["phantomShaderId"].Value}");
        }
    }
}
