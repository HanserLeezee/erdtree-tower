using SoulsFormats;

/// 余烬「由心及物」：两行轮换的最大生命倍率 9670042、9670043（玩家增益的中性模板，常驻；倍率由运行时按最大专注改写），
/// 拾取提示的显示物品 9900041（复制「亚兹勒的冥想」的显示行，见 TentAssets.Display）。
static class MindAssets {
    static readonly int[] Effects={9670042,9670043};
    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var before=file.Bytes.ToArray();
        var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,"SpEffect.xml"));
        var p=PARAM.Read(before);p.ApplyParamdef(def);
        for(int i=0;i<Effects.Length;i++) {
            var row=EmberAssets.Player(p,Effects[i],$"Tower ember mind hp {i}",1f,-1f);
            row["maxHpRate"].Value=Convert.ChangeType(1f,row["maxHpRate"].Value.GetType());
            var old=p.Rows.SingleOrDefault(r=>r.ID==row.ID);
            if(old!=null&&old.Name!=row.Name)throw new Exception($"ID collision: SpEffectParam/{row.ID}");
            p.Rows.RemoveAll(r=>r.ID==row.ID);p.Rows.Add(row);
        }
        p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=p.Write();
        var original=PARAM.Read(before);original.ApplyParamdef(def);
        var actual=p.Rows.Where(r=>!Effects.Contains(r.ID)).ToDictionary(r=>r.ID);
        if(actual.Count!=original.Rows.Count(r=>!Effects.Contains(r.ID)))throw new Exception("Unrelated row count changed");
        foreach(var r in original.Rows.Where(r=>!Effects.Contains(r.ID)))foreach(var c in r.Cells)
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,actual[r.ID][c.Def.InternalName].Value))throw new Exception($"Unrelated row changed: {r.ID}");
        var stage=Path.Combine(output,"mind-stage");Directory.CreateDirectory(stage);
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(stage,"regulation.bin"),reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(stage,"regulation.bin"));
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine("Mind: SpEffectParam 9670042/9670043 written (maxHpRate 1.0, rewritten at runtime); other rows unchanged.");
        // 文本仍从原输入读取，regulation 换成刚写出的那份
        foreach(var lang in new[]{"zhocn","engus"}) {var src=Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx");if(File.Exists(src)){var dst=Path.Combine(stage,$"msg/{lang}/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(dst)!);File.Copy(src,dst,true);}}
        TentAssets.Display(stage,output,game,9900041,9900022,"Meditation","Mind","余烬：由心及物","Ember: Mind over Matter");
        Directory.Delete(stage,true);
    }
}
