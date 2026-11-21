using SoulsFormats;

static class ActRosterAssets {
    static readonly (string map,uint entity)[] Originals=[
        ("m12_09_00_00",12090800),("m25_00_00_00",25000800),("m61_49_48_00",2049480800),
        ("m41_00_00_00",41000800),("m60_49_38_00",1049380800),("m60_51_57_00",1051570271),
        ("m60_42_36_00",1042360800),("m13_00_00_00",13000850)];
    public static void Build(string game,string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Use separate staging");
        var slots=File.ReadAllLines(Path.Combine(input,"party-extra-slots.tsv")).Select(l=>l.Split('|').Select(int.Parse).ToArray()).Where(p=>p[5] is >=48030228 and <=48030252).ToArray();
        if(slots.Length!=25)throw new Exception("Missing act roster slots");
        using var archives=new ArenaAssets.Archives(game);
        var original=Originals.Select(o=>archives.Map(o.map)!.Parts.Enemies.Single(e=>e.EntityID==o.entity)).ToArray();
        var templates=Enumerable.Range(0,3).SelectMany(i=>MSBE.Read(Path.Combine(input,$"map/mapstudio/m45_0{i}_00_00.msb.dcx")).Parts.Enemies).ToLookup(e=>e.NPCParamID);
        var parts=BossAssets.Parts(input);
        var actors=slots.Select(p=>{
            var index=(p[1]-950000000)/2;
            var source=p[1] is >=950000328 and <=950000343?original[index-164]:templates[p[1]].First();
            if(source.ThinkParamID!=p[3]||source.ModelName!=$"c{p[2]:D4}"||(p[1] is >=950000328 and <=950000343&&source.NPCParamID!=parts.Single(b=>b.Index==index).Source))throw new Exception("Actor template mismatch: "+p[5]);
            var actor=(MSBE.Part.Enemy)source.DeepCopy();actor.EntityID=(uint)p[5];actor.NPCParamID=p[1];
            actor.EntityGroupIDs=new uint[8];actor.TalkID=-1;actor.ChrActivateCondParamID=0;actor.BackupEventAnimID=-1;
            actor.SpEffectSetParamID=new int[actor.SpEffectSetParamID.Length];return (p[0],actor);
        }).ToArray();
        EnemyExpansionAssets.PatchActors(input,output,actors);
        BossAssets.Append(input,output,firstIndex:164);
        var path=Path.Combine(output,"regulation.bin");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam");
        var npcs=PARAM.Read(file.Bytes);npcs.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml"));
        var tiers=TowerScalingAssets.Tiers(input);
        foreach(var p in slots)TowerScalingAssets.Apply(npcs[p[1]],tiers[p[0]][p[1] is >=950000000 and <=950000343 && p[1]%2==0?2:1]);
        foreach(var (id,source) in new[]{(950000344,35600172),(950000345,35700172)}){
            var row=new PARAM.Row(npcs[source]){ID=id};
            foreach(var (key,value) in new[]{("itemLotId_enemy",-1),("itemLotId_map",-1),("getSoul",0),("disableRespawn",0),("isSoulGetByBoss",0),("hp",(int)BalanceAssets.ScaleHp(Convert.ToInt64(row["hp"].Value)))})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            TowerScalingAssets.Apply(row,tiers[2][2]);npcs.Rows.RemoveAll(r=>r.ID==id);npcs.Rows.Add(row);
        }
        npcs.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        file.Bytes=npcs.Write();SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        Text(input,output,parts);
        GodskinArenaAssets.RemoveLegacy(input,output);
    }
    static void Text(string input,string output,BossAssets.Part[] parts) {
        string[] en=["Regal Ancestor Spirit","Metyr, Mother of Fingers","Commander Gaius","Demi-Human Swordmaster Onze","Commander O'Neil","Castle Sol Banished Knight (Twin Swords)","Tree Sentinel","Godskin Duo"];
        string[] tw=["祖靈之王","「指頭之母」梅蒂爾","老將蓋烏斯","亞人劍聖翁吉","老將歐尼爾","索爾城失鄉騎士（雙劍）","大樹守衛","神皮雙人組"];
        var combos=File.ReadAllLines(Path.Combine(input,"boss-affix-combos.txt")).Select(l=>int.Parse(l.Split('|')[0])).ToArray();
        var previews=MelinaAssets.PreviewNames(input);
        foreach(var lang in new[]{"zhocn","zhotw","engus"})foreach(var pack in new[]{"item","menu"}) {
            var rel=$"msg/{lang}/{pack}_dlc02.msgbnd.dcx";var binder=BND4.Read(Path.Combine(input,rel));
            var target=pack=="item"?binder.Files.First(f=>f.Name.Contains("NpcName")):binder.Files.Single(f=>f.ID==MelinaAssets.PreviewMenu);
            var fmg=FMG.Read(target.Bytes);var texts=fmg.Entries.ToDictionary(e=>e.ID,e=>e.Text);
            foreach(var p in parts.Where(p=>p.Index>=164)){
                var name=lang=="engus"?en[p.Index-164]:lang=="zhotw"?tw[p.Index-164]:p.Name;
                if(pack=="menu"){int first=MelinaAssets.PreviewFirst+16*Array.IndexOf(previews,p.Name);for(int n=1;n<=15;n++)texts[first+n]=$"{name} ×{n}";continue;}
                texts[9600000+p.Index]=name;
                foreach(var c in combos){
                    int Affix(int i)=>c>=215?9750000+(c-215)*256+i:c<111?9700000+c*200+i:i<144?9722200+(c-111)*144+i:9740000+(c-111)*64+i-144;
                    var reference=texts[Affix(159)];var old=texts[9600159];if(!reference.EndsWith(old))throw new Exception($"Invalid affix reference {lang}/{c}: {reference} / {old}");
                    texts[Affix(p.Index)]=reference[..^old.Length]+name;
                }
            }
            fmg.Entries.Clear();foreach(var p in texts.OrderBy(p=>p.Key))fmg.Entries.Add(new(p.Key,p.Value));
            target.Bytes=fmg.Write();var dest=Path.Combine(output,rel);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);binder.Write(dest);
        }
    }
}
