using SoulsFormats;
using SoulsFormats.Cryptography;

static class ShopEmberReceipts {
    static (string Name,string Effect) DisplayText(string lang,int id)=>(lang,id) switch {
        ("zhocn",9900042)=>("余烬：卡利亚剑阵","普攻命中敌人时\n回复 5% 最大专注（至少 4 点）\n30% 概率召唤卡利亚剑阵\n冷却 12 秒"),
        ("zhocn",9900043)=>("余烬：重咏之威","施放魔法或祷告时\n基础消耗每有 1 点专注\n伤害提高 1.5%"),
        ("zhocn",9900044)=>("余烬：狩猎印记","弓弩命中同一目标 4 次标记猎物\n持续 12 秒，猎物受伤 +30%\n击杀被标记目标回复 15% 生命\n与 20% 专注"),
        ("zhocn",9900045)=>("余烬：百毒淬羽","弓弩命中随机附加 40 点异常\n目标每存在 1 种异常状态\n弓弩伤害 +15%"),
        ("zhotw",9900042)=>("餘燼：卡利亞劍陣","普攻命中敵人時\n回覆 5% 最大專注（至少 4 點）\n30% 機率召喚卡利亞劍陣\n冷卻 12 秒"),
        ("zhotw",9900043)=>("餘燼：重詠之威","施放魔法或禱告時\n基礎消耗每有 1 點專注\n傷害提高 1.5%"),
        ("zhotw",9900044)=>("餘燼：狩獵印記","弓弩命中同一目標 4 次標記獵物\n持續 12 秒，獵物受傷 +30%\n擊殺被標記目標回覆 15% 生命\n與 20% 專注"),
        ("zhotw",9900045)=>("餘燼：百毒淬羽","弓弩命中隨機附加 40 點異常\n目標每存在 1 種異常狀態\n弓弩傷害 +15%"),
        ("engus",9900042)=>("Ember: Carian Phalanx","On light melee hit:\nrestore 5% max FP (min 4),\n30% chance to summon Carian Phalanx\n(12s cooldown)."),
        ("engus",9900043)=>("Ember: Weight of Casting","Sorceries and incantations gain\n+1.5% damage per 1 base FP cost."),
        ("engus",9900044)=>("Ember: Hunter's Mark","4 bow/crossbow hits on the same target\nmark it as Prey for 12s (+30% damage taken).\nKilling a marked target restores\n15% max HP and 20% max FP."),
        ("engus",9900045)=>("Ember: Venomed Fletching","Bow/crossbow hits randomly inflict 40 buildup;\ngain +15% bow/crossbow damage per active\nailment on the target."),
        _=>throw new ArgumentOutOfRangeException(nameof(id)),
    };

    public static void Build(string input,string output){
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Use a separate output directory");
        Directory.CreateDirectory(output);
        var reg=RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamGoods");
        var goods=PARAM.Read(file.Bytes);
        goods.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs","EquipParamGoods.xml")));
        var displays=goods.Rows.Where(r=>r.ID>=9900000 && r.ID<=9900045 && r.Name?.StartsWith("Tower ember display ")==true).ToArray();
        if(!Enumerable.Range(9900042,4).All(id=>displays.Any(r=>r.ID==id))||displays.Length<45)throw new Exception("Incomplete ember display rows");
        var ids=displays.Select(r=>r.ID).ToHashSet();
        foreach(var display in displays){
            int id=display.ID&0x7fffff;
            string name="Shop receipt "+display.Name;
            var existing=goods.Rows.SingleOrDefault(r=>r.ID==id);
            if(existing!=null && existing.Name!=name)throw new Exception($"Goods collision {id}");
            if(existing!=null)goods.Rows.Remove(existing);
            goods.Rows.Add(new PARAM.Row(display){ID=id,Name=name});
        }
        goods.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=goods.Write();
        string target=Path.Combine(output,"regulation.bin");
        RegulationDecryptor.EncryptERRegulation(target,reg);
        if(!RegulationDecryptor.DecryptERRegulation(target).Files.Zip(reg.Files).All(p=>p.First.Bytes.SequenceEqual(p.Second.Bytes)))throw new Exception("Regulation round-trip failed");
        foreach(var lang in new[]{"engus","zhocn","zhotw"}){
            var named=new HashSet<int>();
            foreach(var path in Directory.GetFiles(Path.Combine(input,"msg",lang),"item*.msgbnd.dcx")){
                var binder=BND4.Read(path);bool changed=false;
                foreach(var f in binder.Files){
                    var key=Path.GetFileNameWithoutExtension(f.Name).Split("_dlc")[0];
                    if(key is not ("GoodsName" or "GoodsInfo" or "GoodsCaption"))continue;
                    var text=FMG.Read(f.Bytes);
                    if(Path.GetFileName(path)=="item_dlc02.msgbnd.dcx"){
                        foreach(int id in Enumerable.Range(9900042,4)){
                            var value=DisplayText(lang,id);
                            string label=key=="GoodsName"?value.Name:value.Effect;
                            foreach(int entryId in new[]{id,id&0x7fffff}){
                                var entry=text.Entries.SingleOrDefault(e=>e.ID==entryId);
                                if(entry==null)text.Entries.Add(new FMG.Entry(entryId,label));
                                else entry.Text=label;
                            }
                        }
                    }
                    var entries=text.Entries.Where(e=>ids.Contains(e.ID)).ToArray();
                    if(entries.Length==0)continue;
                    foreach(var e in entries){
                        int id=e.ID&0x7fffff;
                        if(key=="GoodsName"&&!string.IsNullOrWhiteSpace(e.Text))named.Add(e.ID);
                        var prior=text.Entries.SingleOrDefault(x=>x.ID==id);
                        if(prior!=null && prior.Text!=e.Text)throw new Exception($"Text collision {lang}/{key}/{id}");
                        if(prior==null)text.Entries.Add(new FMG.Entry(id,e.Text));
                    }
                    text.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));f.Bytes=text.Write();changed=true;
                }
                if(changed){
                    var dest=Path.Combine(output,Path.GetRelativePath(input,path));
                    Directory.CreateDirectory(Path.GetDirectoryName(dest)!);binder.Write(dest);
                }
            }
            if(!ids.SetEquals(named))throw new Exception($"Missing receipt names in {lang}: {string.Join(",",ids.Except(named))}");
        }
        Console.WriteLine($"Verified {ids.Count} distinct ember receipts and all three languages");
    }
}
