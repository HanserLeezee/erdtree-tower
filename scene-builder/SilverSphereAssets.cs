using SoulsFormats;

static class SilverSphereAssets {
    const int Think=133308039,FirstBuddy=1601560000,FirstTrigger=611560000;
    const string Old="召唤三只大型银色泪滴，以弹跳冲撞敌人。",Info="召唤一只大型银色泪滴，以弹跳冲撞敌人。";
    public static void ConfigureBuddy(PARAM.Row row) {row["x_offset"].Value=4.5f;row["z_offset"].Value=0f;}
    public static void ConfigureThink(PARAM.Row row) {
        foreach(var name in new[]{"disableLocalSteering","thinkAttr_doAdmirer"})row[name].Value=Convert.ChangeType(0,row[name].Value.GetType());
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Use a separate output directory");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name){var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,name+".xml")));return p;}
        var buddy=Read("BuddyParam");var think=Read("NpcThinkParam");
        var beforeBuddy=buddy.Rows.ToDictionary(r=>r.ID,r=>new PARAM.Row(r));
        var beforeThink=think.Rows.ToDictionary(r=>r.ID,r=>new PARAM.Row(r));
        var npc=Read("NpcParam").Rows.Single(r=>r.ID==Think);
        if(Convert.ToInt32(npc["teamType"].Value)!=47||Convert.ToSingle(npc["hitRadius"].Value)!=2.25f)throw new Exception("Unexpected sphere NPC layout");
        var ai=think.Rows.Single(r=>r.ID==Think);
        if(Convert.ToInt32(ai["battleGoalID"].Value)!=333000||Convert.ToInt32(ai["isBuddyAI"].Value)!=1)throw new Exception("Unexpected sphere AI");
        ConfigureThink(ai);
        var removed=new HashSet<int>();var edited=new HashSet<int>();
        for(int level=0;level<40;level++) {
            int id=FirstBuddy+level*1000,trigger=FirstTrigger+level*1000;
            var group=buddy.Rows.Where(r=>Convert.ToInt32(r["triggerSpEffectId"].Value)==trigger).ToArray();
            if(group.Length is not (1 or 3)||group.Any(r=>r.ID<id||r.ID>id+2||Convert.ToInt32(r["npcParamId"].Value)!=Think))throw new Exception($"Unexpected sphere group +{level}");
            var main=group.Single(r=>r.ID==id);ConfigureBuddy(main);edited.Add(id);
            foreach(var extra in group.Where(r=>r.ID!=id)){buddy.Rows.Remove(extra);removed.Add(extra.ID);}
        }
        void Check(PARAM p,Dictionary<int,PARAM.Row> before,string table) {
            foreach(var row in p.Rows)foreach(var cell in row.Cells) {
                var name=cell.Def.InternalName;
                bool allowed=table=="BuddyParam"?edited.Contains(row.ID)&&name is "x_offset" or "z_offset":row.ID==Think&&name is "disableLocalSteering" or "thinkAttr_doAdmirer";
                if(!allowed&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,before[row.ID][name].Value))throw new Exception($"Unrelated change: {table}/{row.ID}/{name}");
            }
            if(before.Keys.Except(p.Rows.Select(r=>r.ID)).Any(id=>table!="BuddyParam"||!removed.Contains(id)))throw new Exception("Unrelated row removed");
        }
        Check(buddy,beforeBuddy,"BuddyParam");Check(think,beforeThink,"NpcThinkParam");
        File_("BuddyParam").Bytes=buddy.Write();File_("NpcThinkParam").Bytes=think.Write();
        Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var reread=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var file in reg.Files)if(!reread.Files.Single(f=>f.Name==file.Name).Bytes.SequenceEqual(file.Bytes))throw new Exception("Serialization mismatch");
        foreach(var name in new[]{"custom-spirit-definitions.tsv","reward-details.tsv"}) {
            var source=Path.Combine(input,name);if(!File.Exists(source))continue;
            var lines=File.ReadAllLines(source).Select(line=>{
                line=line.Replace(Old,Info);
                if(name=="custom-spirit-definitions.tsv"&&line.StartsWith("309000|")){var p=line.Split('|');p[10]="1";line=string.Join('|',p);}
                return line;
            });File.WriteAllLines(Path.Combine(output,name),lines);
        }
        foreach(var folder in new[]{Path.Combine(input,"msg"),Path.Combine(input,"locales")}.Where(Directory.Exists))
        foreach(var source in Directory.EnumerateFiles(folder,"*.msgbnd.dcx",SearchOption.AllDirectories)) {
            var bnd=BND4.Read(source);bool changed=false;
            foreach(var file in bnd.Files) {
                if(!file.Name.EndsWith(".fmg",StringComparison.OrdinalIgnoreCase))continue;
                var fmg=FMG.Read(file.Bytes);bool edit=false;
                foreach(var entry in fmg.Entries.Where(e=>e.ID>=309000&&e.ID<=309039&&e.Text!=null)) {
                    var text=entry.Text.Replace(Old,Info).Replace("Summons three Large Silver Tears that bounce into foes.","Summons a Large Silver Tear that bounces into foes.")
                        .Replace("召喚三隻大型銀色淚滴", "召喚一隻大型銀色淚滴");
                    if(text!=entry.Text){entry.Text=text;edit=true;}
                }
                if(edit){file.Bytes=fmg.Write();changed=true;}
            }
            if(changed){var dest=Path.Combine(output,Path.GetRelativePath(input,source));Directory.CreateDirectory(Path.GetDirectoryName(dest)!);bnd.Write(dest);}
        }
        Console.WriteLine($"PASS: 40 single sphere groups, {removed.Count} extra rows removed; only summon offsets and two sphere AI fields changed; serialized tables verified.");
    }
}
