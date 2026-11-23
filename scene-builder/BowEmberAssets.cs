using SoulsFormats;

/// 弓箭流派余烬「狩猎印记」与「百毒淬羽」：
/// - SpEffectParam 9670012（中毒积累 40，复制中毒油脂命中 3176）、9670013（猩红腐败积累 40，复制腐败油脂命中 3311），开启 effectTargetSelfTarget=1；
/// - SpEffectParam 9670047（狩猎印记：挂在敌人身上持续 12 秒，全属性受到伤害 ×1.30，开启 effectTargetSelfTarget=1）；
/// - SpEffectParam 9670048、9670049（百毒淬羽：两行轮换，复制箭矢护符 321300 并保留 magicSubCategoryChange1=100 仅增强弓弩弹药伤害，运行时按目标异常种数改写倍率）；
/// - 拾取提示的显示物品 9900044（狩猎印记，沿用 9900026/65026 图标）与 9900045（百毒淬羽，沿用 9900001/65001 图标），见 TentAssets.Display。
static class BowEmberAssets {
    const int PoisonId=9670012,RotId=9670013,HunterMarkId=9670047;
    static readonly int[] VenomSlots={9670048,9670049};
    static readonly int[] AllEffects={PoisonId,RotId,HunterMarkId,9670048,9670049};
    const int Buildup=40;
    const float MarkSeconds=12f,MarkDamageTaken=1.30f;
    static void Set(PARAM.Row row,string key,object value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());

    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var before=file.Bytes.ToArray();
        var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,"SpEffect.xml"));
        var p=PARAM.Read(before);p.ApplyParamdef(def);

        var rows=new List<PARAM.Row>();
        // 中毒 / 猩红腐败积累：复制原版油脂命中效果，开启直接施加
        var poison=new PARAM.Row(p.Rows.Single(r=>r.ID==3176)){ID=PoisonId,Name="Tower ember poison buildup"};
        Set(poison,"poizonAttackPower",Buildup);Set(poison,"effectTargetSelfTarget",1);rows.Add(poison);
        var rot=new PARAM.Row(p.Rows.Single(r=>r.ID==3311)){ID=RotId,Name="Tower ember rot buildup"};
        Set(rot,"diseaseAttackPower",Buildup);Set(rot,"effectTargetSelfTarget",1);rows.Add(rot);

        // 狩猎印记：挂在被标记的敌人身上 12 秒，全属性受到伤害 +30%（DamageCutRate=1.30）
        var mark=EmberAssets.Enemy(p,HunterMarkId,"Tower ember hunter mark",MarkSeconds);
        Set(mark,"effectTargetSelfTarget",1);
        foreach(var cut in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(mark,cut+"DamageCutRate",MarkDamageTaken);
        rows.Add(mark);

        // 百毒淬羽：两行轮换，保留 321300（箭矢护符）的 magicSubCategoryChange1=100，仅对弓弩箭矢/弩箭生效
        var arrowTemplate=p.Rows.Single(r=>r.ID==321300);
        for(int i=0;i<VenomSlots.Length;i++) {
            var row=new PARAM.Row(arrowTemplate){ID=VenomSlots[i],Name=$"Tower ember venom fletch {i}"};
            foreach(var (key,value) in new (string,object)[]{
                ("iconId",-1),("effectEndurance",-1f),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0)
            })Set(row,key,value);
            foreach(var el in new[]{"physics","magic","fire","thunder","dark"})Set(row,el+"AttackRate",1f);
            foreach(var target in new[]{"Player","Enemy"})foreach(var el in new[]{"Physics","Magic","Fire","Thunder","Dark"})Set(row,$"atk{target}DmgCorrectRate_{el}",1f);
            rows.Add(row);
        }

        foreach(var row in rows) {
            var old=p.Rows.SingleOrDefault(r=>r.ID==row.ID);
            if(old!=null&&old.Name!=row.Name)throw new Exception($"ID collision: SpEffectParam/{row.ID} ({old.Name})");
            p.Rows.RemoveAll(r=>r.ID==row.ID);p.Rows.Add(row);
        }
        p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=p.Write();

        // 验证其他行逐格未变
        var original=PARAM.Read(before);original.ApplyParamdef(def);
        var actual=p.Rows.Where(r=>!AllEffects.Contains(r.ID)).ToDictionary(r=>r.ID);
        if(actual.Count!=original.Rows.Count(r=>!AllEffects.Contains(r.ID)))throw new Exception("Unrelated SpEffect row count changed");
        foreach(var r in original.Rows.Where(r=>!AllEffects.Contains(r.ID))) {
            var now=actual[r.ID];
            for(int ci=0;ci<r.Cells.Count;ci++)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(r.Cells[ci].Value,now.Cells[ci].Value))
                    throw new Exception($"Unrelated row changed: {r.ID}/{r.Cells[ci].Def.InternalName}");
        }
        Verify(p);

        var stage1=Path.Combine(output,"bow-stage-1");var stage2=Path.Combine(output,"bow-stage-2");
        Directory.CreateDirectory(stage1);Directory.CreateDirectory(stage2);
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(stage1,"regulation.bin"),reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(stage1,"regulation.bin"));
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        foreach(var lang in new[]{"zhocn","engus"}) {
            var src=Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx");
            if(File.Exists(src)){var dst=Path.Combine(stage1,$"msg/{lang}/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(dst)!);File.Copy(src,dst,true);}
        }
        TentAssets.Display(stage1,stage2,game,9900044,9900025,"Footwork","HunterMark","余烬：狩猎印记","Ember: Hunter's Mark");
        TentAssets.Display(stage2,output,game,9900045,9900001,"Bloodlord","VenomFletch","余烬：百毒淬羽","Ember: Venomed Fletching");
        Directory.Delete(stage1,true);Directory.Delete(stage2,true);
        Console.WriteLine("BowEmberAssets: SpEffectParam 9670012/9670013/9670047/9670048/9670049 and display goods 9900044/9900045 verified.");
    }

    public static void Verify(PARAM p) {
        var poi=p.Rows.Single(r=>r.ID==PoisonId);
        if(Convert.ToInt32(poi["poizonAttackPower"].Value)!=Buildup||Convert.ToInt32(poi["effectTargetSelfTarget"].Value)!=1||Convert.ToInt32(poi["stateInfo"].Value)!=2)
            throw new Exception("Invalid poison buildup row 9670012");
        var rot=p.Rows.Single(r=>r.ID==RotId);
        if(Convert.ToInt32(rot["diseaseAttackPower"].Value)!=Buildup||Convert.ToInt32(rot["effectTargetSelfTarget"].Value)!=1||Convert.ToInt32(rot["stateInfo"].Value)!=5)
            throw new Exception("Invalid rot buildup row 9670013");
        var mark=p.Rows.Single(r=>r.ID==HunterMarkId);
        if(Convert.ToInt32(mark["effectTargetSelfTarget"].Value)!=1||Math.Abs(Convert.ToSingle(mark["effectEndurance"].Value)-MarkSeconds)>0.01f||Math.Abs(Convert.ToSingle(mark["slashDamageCutRate"].Value)-MarkDamageTaken)>0.001f)
            throw new Exception("Invalid hunter mark row 9670047");
        foreach(var id in VenomSlots) {
            var v=p.Rows.Single(r=>r.ID==id);
            if(Convert.ToInt32(v["magicSubCategoryChange1"].Value)!=100||Convert.ToInt32(v["spCategory"].Value)!=0)
                throw new Exception($"Invalid venom fletch row {id}");
        }
    }
}
