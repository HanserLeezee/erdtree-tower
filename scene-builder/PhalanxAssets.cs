using SoulsFormats;

/// 余烬「卡利亚剑阵」：触发效果 9670044（复制 1630，behaviorId = 9670044）→ BehaviorParam_PC 9670044（复制 2120，召唤原版卡利亚圆阵 Bullet 10430100）；
/// 拾取提示的显示物品 9900042（复制「魔力辉剑」9900040，沿用 65040 原生图标，见 TentAssets.Display）。
static class PhalanxAssets {
    const int Effect=9670044,Behavior=9670044,CarianPhalanxBullet=10430100;
    static void Set(PARAM.Row row,string key,object value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        var spFile=File_("SpEffectParam");var behFile=File_("BehaviorParam_PC");
        var spBefore=spFile.Bytes.ToArray();var behBefore=behFile.Bytes.ToArray();
        var spDef=PARAMDEF.XmlDeserialize(Path.Combine(defs,"SpEffect.xml"));
        var behDef=PARAMDEF.XmlDeserialize(Path.Combine(defs,"BehaviorParam.xml"));
        var sp=PARAM.Read(spBefore);sp.ApplyParamdef(spDef);
        var beh=PARAM.Read(behBefore);beh.ApplyParamdef(behDef);
        var effect=new PARAM.Row(sp.Rows.Single(r=>r.ID==1630)){ID=Effect,Name="Tower ember phalanx trigger"};
        foreach(var (key,value) in new (string,object)[]{("behaviorId",Behavior),("iconId",-1),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0)})Set(effect,key,value);
        var oldSp=sp.Rows.SingleOrDefault(r=>r.ID==Effect);
        if(oldSp!=null&&oldSp.Name!=effect.Name)throw new Exception($"ID collision: SpEffectParam/{Effect}");
        sp.Rows.RemoveAll(r=>r.ID==Effect);sp.Rows.Add(effect);sp.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        spFile.Bytes=sp.Write();

        var action=new PARAM.Row(beh.Rows.Single(r=>r.ID==2120)){ID=Behavior,Name="Tower ember phalanx"};
        Set(action,"refType",1);Set(action,"refId",CarianPhalanxBullet);
        var oldBeh=beh.Rows.SingleOrDefault(r=>r.ID==Behavior);
        if(oldBeh!=null&&oldBeh.Name!=action.Name)throw new Exception($"ID collision: BehaviorParam_PC/{Behavior}");
        beh.Rows.RemoveAll(r=>r.ID==Behavior);beh.Rows.Add(action);beh.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        behFile.Bytes=beh.Write();

        void CheckUnchanged(byte[] beforeBytes,PARAM current,PARAMDEF def,int skipId,string table) {
            var orig=PARAM.Read(beforeBytes);orig.ApplyParamdef(def);
            var map=current.Rows.Where(r=>r.ID!=skipId).ToDictionary(r=>r.ID);
            if(map.Count!=orig.Rows.Count(r=>r.ID!=skipId))throw new Exception($"Unrelated row count changed: {table}");
            foreach(var r in orig.Rows.Where(r=>r.ID!=skipId)) {
                var now=map[r.ID];
                for(int ci=0;ci<r.Cells.Count;ci++)
                    if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(r.Cells[ci].Value,now.Cells[ci].Value))
                        throw new Exception($"Unrelated row changed: {table}/{r.ID}/{r.Cells[ci].Def.InternalName}");
            }
        }
        CheckUnchanged(spBefore,sp,spDef,Effect,"SpEffectParam");
        CheckUnchanged(behBefore,beh,behDef,Behavior,"BehaviorParam_PC");

        var stage=Path.Combine(output,"phalanx-stage");Directory.CreateDirectory(stage);
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(stage,"regulation.bin"),reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(stage,"regulation.bin"));
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine($"Phalanx: SpEffectParam/{Effect} -> BehaviorParam_PC/{Behavior} -> Bullet/{CarianPhalanxBullet} written; other rows unchanged.");
        foreach(var lang in new[]{"zhocn","engus"}) {var src=Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx");if(File.Exists(src)){var dst=Path.Combine(stage,$"msg/{lang}/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(dst)!);File.Copy(src,dst,true);}}
        TentAssets.Display(stage,output,game,9900042,9900040,"Glintblade","Phalanx","余烬：卡利亚剑阵","Ember: Carian Phalanx");
        Directory.Delete(stage,true);
    }
}
