using SoulsFormats;

/// 余烬「重咏之威」：两行轮换的法术/祷告专属增伤 9670045、9670046（复制原版魔力块护符 330010：stateInfo=71、wepParamChange=3，
/// 同时开启 magParamChange=1 与 miracleParamChange=1，只增强魔法与祷告、不影响近战武器；运行时按基础专注消耗改写倍率），
/// 拾取提示的显示物品 9900043（复制「辉石的余韵」9900023，沿用 65023 原生图标，见 TentAssets.Display）。
static class HeavyCastAssets {
    static readonly int[] Effects={9670045,9670046};
    static void Set(PARAM.Row row,string key,object value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var before=file.Bytes.ToArray();
        var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,"SpEffect.xml"));
        var p=PARAM.Read(before);p.ApplyParamdef(def);
        var template=p.Rows.Single(r=>r.ID==330010);
        for(int i=0;i<Effects.Length;i++) {
            var row=new PARAM.Row(template){ID=Effects[i],Name=$"Tower ember heavy cast {i}"};
            foreach(var (key,value) in new (string,object)[]{
                ("iconId",-1),("effectEndurance",-1f),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0),
                ("magParamChange",1),("miracleParamChange",1)
            })Set(row,key,value);
            foreach(var el in new[]{"physics","magic","fire","thunder","dark"})Set(row,el+"AttackRate",1f);
            foreach(var target in new[]{"Player","Enemy"})foreach(var el in new[]{"Physics","Magic","Fire","Thunder","Dark"})Set(row,$"atk{target}DmgCorrectRate_{el}",1f);
            var old=p.Rows.SingleOrDefault(r=>r.ID==row.ID);
            if(old!=null&&old.Name!=row.Name)throw new Exception($"ID collision: SpEffectParam/{row.ID}");
            p.Rows.RemoveAll(r=>r.ID==row.ID);p.Rows.Add(row);
        }
        p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=p.Write();
        var original=PARAM.Read(before);original.ApplyParamdef(def);
        var actual=p.Rows.Where(r=>!Effects.Contains(r.ID)).ToDictionary(r=>r.ID);
        if(actual.Count!=original.Rows.Count(r=>!Effects.Contains(r.ID)))throw new Exception("Unrelated row count changed");
        foreach(var r in original.Rows.Where(r=>!Effects.Contains(r.ID))) {
            var now=actual[r.ID];
            for(int ci=0;ci<r.Cells.Count;ci++)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(r.Cells[ci].Value,now.Cells[ci].Value))
                    throw new Exception($"Unrelated row changed: {r.ID}/{r.Cells[ci].Def.InternalName}");
        }
        var stage=Path.Combine(output,"heavycast-stage");Directory.CreateDirectory(stage);
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(stage,"regulation.bin"),reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(stage,"regulation.bin"));
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine("HeavyCast: SpEffectParam 9670045/9670046 written (sorcery+incantation only, rewritten at runtime); other rows unchanged.");
        foreach(var lang in new[]{"zhocn","engus"}) {var src=Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx");if(File.Exists(src)){var dst=Path.Combine(stage,$"msg/{lang}/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(dst)!);File.Copy(src,dst,true);}}
        TentAssets.Display(stage,output,game,9900043,9900023,"Glintstone","HeavyCast","余烬：重咏之威","Ember: Weight of Casting");
        Directory.Delete(stage,true);
    }
}
