using SoulsFormats;

static class SomberAshAssets {
    record Ash(int Id,int Art,int Text,int[] Weapons);
    const string Manifest="somber-ashes.tsv";
    static int I(PARAM.Row row,string k)=>Convert.ToInt32(row[k].Value);
    static void Set(PARAM.Row row,string k,int value)=>row[k].Value=Convert.ChangeType(value,row[k].Value.GetType());
    public static bool Generated(int id)=>id is >=90000 and <100000 or >=900000 and <910000;
    static bool Dlc(int id)=>id>=60000000||id%1000000>=500000;
    static Dictionary<int,string> TextTable(BND4 binder,string name) {
        var result=new Dictionary<int,string>();
        foreach(var file in binder.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name).Split("_dlc")[0]==name).OrderBy(f=>f.ID))
            foreach(var e in FMG.Read(file.Bytes).Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text)))
                result[e.ID]=EquipmentStyleAssets.Plain(e.Text);
        return result;
    }

    public static void Build(string game,string input,string output,string defs) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new ArgumentException("Separate staging directory required");
        Directory.CreateDirectory(output);
        var source=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        BinderFile FileOf(BND4 b,string n)=>b.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==n);
        PARAM Read(BND4 b,string n){var p=PARAM.Read(FileOf(b,n).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,n+".xml")));return p;}
        var w=Read(source,"EquipParamWeapon");
        var arts=Read(source,"SwordArtsParam").Rows.ToDictionary(r=>r.ID);
        var gemTable=Read(reg,"EquipParamGem");
        var gems=gemTable.Rows.ToDictionary(r=>r.ID);
        var previous=Path.Combine(input,Manifest);
        var owned=File.Exists(previous)?File.ReadLines(previous).Skip(1).Select(s=>int.Parse(s.Split('\t')[0])).ToHashSet():new HashSet<int>();
        var catalog=File.ReadAllLines(Path.Combine(input,"shop-catalog.txt")).Where(l=>{
            var p=l.Split('|');return p[0]!="4"||!owned.Contains(int.Parse(p[1]));
        }).ToList();
        var sold=catalog.Select(l=>l.Split('|')).Where(p=>p[0]=="4").Select(p=>gems[int.Parse(p[1])]).ToArray();
        string Rel(string lang)=>Path.Combine("msg",lang,"item_dlc02.msgbnd.dcx");
        var names=TextTable(BND4.Read(Path.Combine(input,Rel("zhocn"))),"WeaponName");
        var weapons=w.Rows.Where(r=>r.ID%10000==0&&I(r,"materialSetId")==2200&&names.TryGetValue(r.ID,out var name)
            &&!string.IsNullOrWhiteSpace(name)&&!name.Contains("ERROR",StringComparison.OrdinalIgnoreCase)).ToArray();
        var additions=new List<Ash>();int reused=0;
        foreach(var group in weapons.GroupBy(r=>I(r,"swordArtsParamId")).OrderBy(g=>g.Key)){
            int art=group.Key;
            if(art<=0||art==10)continue;
            bool dlc=group.All(r=>Dlc(r.ID));
            if(sold.Any(r=>I(r,"swordArtsParamId")==art&&(dlc||r.ID<100000))){reused++;continue;}
            int id=(dlc?900000:90000)+art;
            if(!Generated(id)||gems.ContainsKey(id)&&!owned.Contains(id))throw new Exception($"Ash ID collision: {art} -> {id}");
            var donors=group.OrderBy(r=>r.ID).ToArray();
            var row=new PARAM.Row(gems[10000]){ID=id,Name=$"Tower somber ash {art}"};
            Set(row,"swordArtsParamId",art);Set(row,"defaultWepAttr",0);Set(row,"sortId",id);
            Set(row,"iconId",I(donors[0],"iconId"));
            foreach(var c in row.Cells){
                string key=c.Def.InternalName;
                if(key.StartsWith("configurableWepAttr"))Set(row,key,key=="configurableWepAttr00"?1:0);
                if(key.StartsWith("canMountWep_"))Set(row,key,1);
            }
            gemTable.Rows.RemoveAll(r=>r.ID==id);gemTable.Rows.Add(row);
            additions.Add(new(id,art,I(arts[art],"textId"),donors.Select(r=>r.ID).ToArray()));
        }
        if(additions.Count==0||owned.Except(additions.Select(a=>a.Id)).Any())throw new Exception("Unexpected change to somber ash coverage");
        gemTable.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        FileOf(reg,"EquipParamGem").Bytes=gemTable.Write();
        string dest=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(dest,reg);
        if(new FileInfo(dest).Length>=4799*1024)throw new Exception("Regulation size limit");
        var actual=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(dest);
        if(reg.Files.Zip(actual.Files).Any(pair=>!pair.First.Bytes.SequenceEqual(pair.Second.Bytes)))throw new Exception("Regulation roundtrip failed");
        foreach(string lang in new[]{"zhocn","zhotw","engus"}){
            var b=BND4.Read(Path.Combine(input,Rel(lang)));
            var text=new[]{"GemName","GemInfo","GemCaption","ArtsName","ArtsCaption","WeaponName"}.ToDictionary(k=>k,k=>TextTable(b,k));
            string Get(string key,int id){
                var value=text[key].GetValueOrDefault(id)?.Trim();
                return !string.IsNullOrWhiteSpace(value)&&!value.Contains("[ERROR]")?value:throw new Exception($"Missing {lang} {key}/{id}");
            }
            string prefix=Get("GemName",10000);
            int colon=prefix.IndexOfAny([':','：']);
            if(colon<0)throw new Exception($"Unknown ash naming in {lang}: {prefix}");
            prefix=prefix[..(colon+1)]+(lang=="engus"?" ":"");
            foreach(var (table,key) in new[]{(35,"GemName"),(36,"GemInfo"),(37,"GemCaption")}){
                var file=b.Files.Single(f=>f.ID==table);var fmg=FMG.Read(file.Bytes);
                foreach(var a in additions){
                    if(!owned.Contains(a.Id)&&text[key].ContainsKey(a.Id))throw new Exception($"Text collision {key}/{a.Id}");
                    string skill=Get("ArtsName",a.Text);
                    string restriction=lang switch {"engus"=>"Usable on all weapon types.","zhotw"=>"可裝在所有類型的武器上。",_=>"可装在所有类型的武器上。"};
                    string value=table switch {35=>prefix+skill,36=>restriction,_=>Get("ArtsCaption",a.Text)+"\n\n"+restriction};
                    fmg.Entries.RemoveAll(e=>e.ID==a.Id);fmg.Entries.Add(new FMG.Entry(a.Id,value));
                    if(table==35&&lang=="zhocn")catalog.Add($"4|{a.Id}|1200|{value}");
                }
                fmg.Entries.Sort((x,y)=>x.ID.CompareTo(y.ID));file.Bytes=fmg.Write();
            }
            var path=Path.Combine(output,Rel(lang));Directory.CreateDirectory(Path.GetDirectoryName(path)!);b.Write(path);
            var written=BND4.Read(path);
            foreach(string key in new[]{"GemName","GemInfo","GemCaption"}){
                var entries=TextTable(written,key);
                foreach(var a in additions)if(!entries.ContainsKey(a.Id))throw new Exception($"Missing {lang}/{key}/{a.Id}");
            }
        }
        File.WriteAllLines(Path.Combine(output,"shop-catalog.txt"),catalog);
        File.WriteAllLines(Path.Combine(output,Manifest),new[]{"Gem\tArt\tSourceWeapons\tAllowedMounts"}.Concat(
            additions.Select(a=>$"{a.Id}\t{a.Art}\t{string.Join(',',a.Weapons)}\t*")));
        Console.WriteLine($"{weapons.Length} somber weapons; {reused} existing skills; {additions.Count} new ashes ({additions.Count(a=>a.Id<100000)} base game); {weapons.Count(r=>I(r,"swordArtsParamId")==10)} weapons without a skill.");
    }
}
