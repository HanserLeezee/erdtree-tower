using SoulsFormats;

// 自制骨灰是从敌人 NpcParam 复制的，带着敌人的联机设定：多人修正、房主同步、npcType、不算 PvP 伤害，以及金眼掉落几率与爬塔敌人强度档。
// 原版 106 种骨灰这些字段完全一致；联机领奖励时无缝联机线程在销毁远程骨灰复制体时崩溃（剑狮在场，eldenring+0x4b93e2 读空模块）。
// 这里把联机相关字段统一成原版友军模板 121000000，并去掉敌人专用效果；模型自己的动作效果不动。
static class SpiritNetAssets {
    static readonly string[] Fields={"multiPlayCorrectionParamId","isCalculatePvPDamage","isHostSyncChr","npcType","networkWarpDist"};
    static bool EnemyOnly(int id)=>id is >=5260 and <=5263||TowerScalingAssets.AreaScaling(id);
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    public static void Patch(string input,string output,bool verify=false) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var original=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam");
        var npc=PARAM.Read(file.Bytes);npc.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,"NpcParam.xml")));
        var rows=npc.Rows.ToDictionary(r=>r.ID);var ally=rows[121000000];
        var managed=File.ReadAllLines(Path.Combine(input,"custom-spirit-definitions.tsv")).Where(l=>l.Length>0)
            .Select((line,family)=>100000000+int.Parse(line.Split('|')[1])*10000+8000+family).ToList();
        if(managed.Count<56||managed.Any(id=>!rows.TryGetValue(id,out var r)||r.Name?.StartsWith("Tower custom spirit ")!=true))throw new Exception("Custom spirit NPC set incomplete");
        int fields=0,effects=0;
        foreach(var row in managed.Select(id=>rows[id])) {
            foreach(var field in Fields)if(!Equals(row[field].Value,ally[field].Value)) {
                if(verify)throw new Exception($"Spirit net field not aligned: {row.ID}/{field}");
                row[field].Value=ally[field].Value;fields++;
            }
            for(int slot=0;slot<32;slot++)if(EnemyOnly(I(row,$"spEffectID{slot}"))) {
                if(verify)throw new Exception($"Enemy-only effect on spirit: {row.ID}/spEffectID{slot}");
                row[$"spEffectID{slot}"].Value=Convert.ChangeType(-1,row[$"spEffectID{slot}"].Value.GetType());effects++;
            }
        }
        if(verify){Console.WriteLine($"PASS: {managed.Count} custom spirit NPCs use the native ally net settings; no enemy-only effects");return;}
        var before=PARAM.Read(original[file.Name]);before.ApplyParamdef(npc.AppliedParamdef);
        var set=managed.ToHashSet();
        foreach(var old in before.Rows)foreach(var cell in old.Cells) {
            var name=cell.Def.InternalName;var now=rows[old.ID][name].Value;
            bool allowed=set.Contains(old.ID)&&(Fields.Contains(name)||name.StartsWith("spEffectID")&&EnemyOnly(Convert.ToInt32(cell.Value))&&Convert.ToInt32(now)==-1);
            if(!allowed&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,now))throw new Exception($"Unrelated field changed: {old.ID}/{name}");
        }
        file.Bytes=npc.Write();
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        foreach(var f in SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path).Files)
            if(!f.Bytes.SequenceEqual(f.Name==file.Name?file.Bytes:original[f.Name]))throw new Exception("Spirit net serialization mismatch");
        Console.WriteLine($"PASS: {managed.Count} custom spirits aligned to native ally net settings; {fields} fields and {effects} enemy-only effects changed; other rows and tables preserved");
    }
}
