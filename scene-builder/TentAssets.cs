using SoulsFormats;

/// 余烬「帐篷」：只新增拾取提示用的显示物品 9900038（复制「圣杯余响」的显示行，沿用它的图标）。
/// 规则全在运行时（Run::tent_refill），不需要特效行。
/// regulation 部分不需要 Oodle，Linux 上也能跑；物品名文本（msgbnd）要 Oodle，游戏目录里有 oo2core_6_win64.dll 时才一起改。
static class TentAssets {
    public static void Patch(string input,string output,string game)=>Display(input,output,game,9900038,9900009,"Echo","Tent","余烬：帐篷","Ember: Tent");
    /// 余烬「魔力辉剑」：显示物品 9900040，复制「辉石的余韵」的显示行 9900023
    public static void PatchGlintblade(string input,string output,string game)=>Display(input,output,game,9900040,9900023,"Glintstone","Glintblade","余烬：魔力辉剑","Ember: Glintblade");
    /// 只新增拾取提示用的显示物品：复制已有余烬的显示行（沿用它的图标），其他行逐格不变；物品名文本要 Oodle
    public static void Display(string input,string output,string game,int Goods,int Template,string templateKey,string key,string zh,string en) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="EquipParamGoods");
        var before=file.Bytes.ToArray();
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,"EquipParamGoods.xml"));
        var goods=PARAM.Read(before);goods.ApplyParamdef(def);
        var template=goods.Rows.Single(r=>r.ID==Template);
        if(template.Name!="Tower ember display "+templateKey)throw new Exception($"Unexpected template row: {template.Name}");
        var old=goods.Rows.SingleOrDefault(r=>r.ID==Goods);
        if(old!=null&&old.Name!="Tower ember display "+key)throw new Exception($"ID collision: {Goods}");
        goods.Rows.RemoveAll(r=>r.ID==Goods);
        goods.Rows.Add(new PARAM.Row(template){ID=Goods,Name="Tower ember display "+key});
        goods.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        file.Bytes=goods.Write();
        // 其他行逐格不变
        var original=PARAM.Read(before);original.ApplyParamdef(def);
        var actual=goods.Rows.Where(r=>r.ID!=Goods).ToDictionary(r=>r.ID);
        if(actual.Count!=original.Rows.Count(r=>r.ID!=Goods))throw new Exception("Unrelated row count changed");
        foreach(var row in original.Rows.Where(r=>r.ID!=Goods))foreach(var c in row.Cells)
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,actual[row.ID][c.Def.InternalName].Value))throw new Exception($"Unrelated row changed: {row.ID}");
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine($"{key}: display goods {Goods} copied from {Template} (icon {template["iconId"].Value}); other rows unchanged.");
        var oodle=Path.Combine(game,"oo2core_6_win64.dll");
        if(!File.Exists(oodle)) {Console.WriteLine($"{key}: oo2core_6_win64.dll not found; item name text skipped (run on Windows with the game directory).");return;}
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(oodle);
        foreach(var lang in new[]{"zhocn","engus"}) {
            var relative=$"msg/{lang}/item_dlc02.msgbnd.dcx";var msg=BND4.Read(Path.Combine(input,relative));
            var names=msg.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/')).Split("_dlc")[0]=="GoodsName").ToArray();
            if(names.Length==0)throw new Exception("GoodsName table missing");
            foreach(var table in names) {
                var fmg=FMG.Read(table.Bytes);fmg.Entries.RemoveAll(e=>e.ID==Goods);
                fmg.Entries.Add(new FMG.Entry(Goods,lang=="zhocn"?zh:en));table.Bytes=fmg.Write();
            }
            var dest=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);msg.Write(dest);
            var roundtrip=BND4.Read(dest);
            foreach(var f in msg.Files)if(!f.Bytes.SequenceEqual(roundtrip.Files.Single(x=>x.ID==f.ID).Bytes))throw new Exception("Text roundtrip mismatch");
        }
        Console.WriteLine($"{key}: item name text {Goods} written for zhocn and engus.");
    }
}
