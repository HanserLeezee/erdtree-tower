using SoulsFormats;

/// 余烬「捕灵壶」：捕捉成功时挂在敌人身上的一次性扣血特效 9670029（中性敌方模板，持续 0 秒、扣满最大生命，由游戏自己判定死亡），
/// 以及拾取提示用的显示物品 9900039（复制「灵唤铃的号令」的显示行，沿用它的图标）。
/// regulation 部分不需要 Oodle；物品名文本（msgbnd）要 Oodle，游戏目录里有 oo2core_6_win64.dll 时才一起改。
static class CaptureAssets {
    const int Effect=9670029,Goods=9900039,Template=9900015;
    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        foreach(var (table,id) in new[]{("SpEffectParam",Effect),("EquipParamGoods",Goods)}) {
            var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==table);
            var before=file.Bytes.ToArray();
            var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,(table=="SpEffectParam"?"SpEffect":table)+".xml"));
            var p=PARAM.Read(before);p.ApplyParamdef(def);
            PARAM.Row row;
            if(table=="SpEffectParam") {
                row=EmberAssets.Enemy(p,Effect,"Tower ember capture",0f);
                foreach(var (key,value) in new (string,object)[]{("changeHpRate",100f),("motionInterval",0f),("vfxId",-1),("iconId",-1)})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            } else {
                var template=p.Rows.Single(r=>r.ID==Template);
                if(template.Name!="Tower ember display SpiritCommand")throw new Exception($"Unexpected template row: {template.Name}");
                row=new PARAM.Row(template){ID=Goods,Name="Tower ember display Capture"};
            }
            var old=p.Rows.SingleOrDefault(r=>r.ID==id);
            if(old!=null&&old.Name!=row.Name)throw new Exception($"ID collision: {table}/{id}");
            p.Rows.RemoveAll(r=>r.ID==id);p.Rows.Add(row);p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
            file.Bytes=p.Write();
            var original=PARAM.Read(before);original.ApplyParamdef(def);
            var actual=p.Rows.Where(r=>r.ID!=id).ToDictionary(r=>r.ID);
            if(actual.Count!=original.Rows.Count(r=>r.ID!=id))throw new Exception("Unrelated row count changed");
            foreach(var r in original.Rows.Where(r=>r.ID!=id))foreach(var c in r.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,actual[r.ID][c.Def.InternalName].Value))throw new Exception($"Unrelated row changed: {table}/{r.ID}");
            Console.WriteLine($"Capture: {table}/{id} written; other rows unchanged.");
        }
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        var oodle=Path.Combine(game,"oo2core_6_win64.dll");
        if(!File.Exists(oodle)) {Console.WriteLine("Capture: oo2core_6_win64.dll not found; item name text skipped (run on Windows with the game directory).");return;}
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(oodle);
        foreach(var lang in new[]{"zhocn","engus"}) {
            var relative=$"msg/{lang}/item_dlc02.msgbnd.dcx";var msg=BND4.Read(Path.Combine(input,relative));
            var names=msg.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/')).Split("_dlc")[0]=="GoodsName").ToArray();
            if(names.Length==0)throw new Exception("GoodsName table missing");
            foreach(var table in names) {
                var fmg=FMG.Read(table.Bytes);fmg.Entries.RemoveAll(e=>e.ID==Goods);
                fmg.Entries.Add(new FMG.Entry(Goods,lang=="zhocn"?"余烬：捕灵壶":"Ember: Soul Snare Pot"));table.Bytes=fmg.Write();
            }
            var dest=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);msg.Write(dest);
            var roundtrip=BND4.Read(dest);
            foreach(var f in msg.Files)if(!f.Bytes.SequenceEqual(roundtrip.Files.Single(x=>x.ID==f.ID).Bytes))throw new Exception("Text roundtrip mismatch");
        }
        Console.WriteLine("Capture: item name text 9900039 written for zhocn and engus.");
    }
}
