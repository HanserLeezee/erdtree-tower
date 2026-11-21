using SoulsFormats;
using System.Globalization;
using System.Text.RegularExpressions;

static class WeaponTraitAssets {
    const int Effect = 9875000;
    static readonly string[][] Labels = [
        ["破盾：对格挡的削精提高 25%", "破盾：對格擋的削精提高 25%", "Guard Break: +25% base stamina damage against guards"],
        ["远射：射程修正 +30", "遠射：射程修正 +30", "Longshot: +30 projectile range correction"],
        ["冰风暴：满蓄力斩命中时释放冰风暴", "冰風暴：滿蓄力斬命中時釋放冰風暴", "Ice Storm: fully charged slashes unleash an ice storm on hit"],
        ["速咏：施法速度等效灵巧 +15", "速詠：施法速度等效靈巧 +15", "Quick Casting: +15 virtual Dexterity for casting speed"],
        ["蓄势：重击蓄力期间减伤 15%", "蓄勢：重擊蓄力期間減傷 15%", "Brace: take 15% less damage while charging a heavy attack"],
        ["坚守：成功格挡后 2 秒减伤 12%", "堅守：成功格擋後 2 秒減傷 12%", "Bulwark: take 12% less damage for 2s after guarding a hit"],
        ["无伤：满血时减伤 10%", "無傷：滿血時減傷 10%", "Unscathed: take 10% less damage at full HP"],
        ["背水：生命不高于 30% 时减伤 15%", "背水：生命不高於 30% 時減傷 15%", "Last Stand: take 15% less damage at or below 30% HP"],
        ["受身：受击失血后 2 秒减伤 10%", "受身：受擊失血後 2 秒減傷 10%", "Recovery: take 10% less damage for 2s after a damaging hit"]
    ];
    static int I(PARAM.Row r,string k)=>Convert.ToInt32(r[k].Value);
    static void Set(PARAM.Row r,string k,object v)=>r[k].Value=Convert.ChangeType(v,r[k].Value.GetType(),CultureInfo.InvariantCulture);
    const int Marker=9875100;
    static readonly int[][] Values=[
        [10,15,20,25],[12,18,24,30],[6,5,4,3],[6,9,12,15],[6,9,12,15],
        [6,8,10,12],[4,6,8,10],[6,9,12,15],[4,6,8,10]
    ];
    static string Label(int kind,int tier,int lang) {
        var label=Labels[kind][lang];
        var effect=label[(label.IndexOfAny(['：',':'])+1)..].TrimStart();
        if(kind==2)return effect+(lang==2?$" (cooldown: {Values[kind][tier]}s)":$"（冷却 {Values[kind][tier]} 秒）".Replace("冷却",lang==1?"冷卻":"冷却"));
        return effect.Replace(Values[kind][3].ToString(),Values[kind][tier].ToString());
    }
    static int Specialty(int family)=>family switch {11 or 12 or 13=>0,>=40 and <=44=>1,3 or 8 or 66=>2,33 or 34=>3,
        4 or 15 or 23=>4,30 or 31 or 32 or 62=>5,_=>-1};
    static ulong Mix(ulong n){unchecked{n=(n^(n>>30))*0xbf58476d1ce4e5b9UL;n=(n^(n>>27))*0x94d049bb133111ebUL;return n^(n>>31);}}
    public static void Build(string input,string output,string defs) {
        Directory.CreateDirectory(output);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var changed=new List<(BinderFile file,PARAM param)>();
        PARAM Table(string name,string def) {
            var f=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name);
            var p=PARAM.Read(f.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,def+".xml")));
            changed.Add((f,p));return p;
        }
        var weapons=Table("EquipParamWeapon","EquipParamWeapon");
        var originals=weapons.Rows.ToDictionary(r=>r.ID);
        var audit=File.ReadAllLines(Path.Combine(input,"affix-v2-audit.tsv")).Select(l=>l.Split('\t')).ToArray();
        var variants=audit.Skip(1).Where(a=>a[2]=="weapon").ToDictionary(a=>int.Parse(a[0]));
        var iceVariations=variants.Values.Where(a=>Specialty(int.Parse(a[1])/1000000)==2)
            .Select(a=>I(originals[int.Parse(a[0])],"behaviorVariationId")).ToHashSet();
        var oldTraits=File.Exists(Path.Combine(input,"weapon-traits.tsv"))?File.ReadLines(Path.Combine(input,"weapon-traits.tsv"))
            .Select(l=>l.Split('\t')).Where(a=>a.Length==2).ToDictionary(a=>int.Parse(a[0]),a=>int.Parse(a[1])):new();
        var effects=Table("SpEffectParam","SpEffect");
        var behavior=Table("BehaviorParam_PC","BehaviorParam");
        var attacks=Table("AtkParam_Pc","AtkParam");
        PARAM.Row Clone(PARAM p,int id,int source) {
            var row=new PARAM.Row(p.Rows.Single(r=>r.ID==source)){ID=id,Name="Tower weapon trait "+id};
            p.Rows.RemoveAll(r=>r.ID==id);p.Rows.Add(row);return row;
        }
        // The mark is carried back to the attacker by the same targeting flags as Prayerful Strike.
        var mark=Clone(effects,Effect+10,1630);
        Set(mark,"stateInfo",0);Set(mark,"behaviorId",-1);Set(mark,"effectEndurance",0.4f);
        var storm=Clone(effects,Effect+11,1630);Set(storm,"behaviorId",Effect+11);
        var bullet=Clone(behavior,Effect+11,2120);Set(bullet,"refId",3055);
        var eligible=behavior.Rows.Where(r=>iceVariations.Contains(I(r,"variationId"))&&I(r,"refType")==0).Select(r=>I(r,"refId")).ToHashSet();
        int tagged=0;
        var marked=new HashSet<int>();
        foreach(var atk in attacks.Rows.Where(r=>eligible.Contains(r.ID)&&I(r,"atkAttribute") is 0 or 3)){
            if(!Enumerable.Range(1,4).Any(n=>I(atk,"subCategory"+n)==100))continue;
            var slots=Enumerable.Range(0,5).Select(n=>"spEffectId"+n).ToArray();
            if(!slots.Any(k=>I(atk,k)==Effect+10))Set(atk,slots.First(k=>I(atk,k)<0),Effect+10);
            marked.Add(atk.ID);tagged++;
        }
        var working=behavior.Rows.Where(r=>I(r,"refType")==0&&marked.Contains(I(r,"refId"))).Select(r=>I(r,"variationId")).ToHashSet();
        var picks=new Dictionary<int,int[]>();
        var replaced=new Dictionary<int,(int old,int marker)[]>();
        string[] resident=["residentSpEffectId","residentSpEffectId1","residentSpEffectId2"];
        foreach(var (id,a) in variants){
            int tier=int.Parse(a[3]);
            int special=Specialty(int.Parse(a[1])/1000000);
            var pool=new List<int>{6,7,8};
            if(special>=0&&(special!=2||working.Contains(I(originals[id],"behaviorVariationId"))))pool.Add(special);
            int[] ids=a[8].Split(',',StringSplitOptions.RemoveEmptyEntries).Select(int.Parse).ToArray();
            var chosen=Enumerable.Repeat(-1,ids.Length).ToArray();
            var swaps=new List<(int,int)>();int oldRemaining=AffixAssets.Kinds;
            for(int slot=0;slot<ids.Length;slot++){
                int draw=(int)(Mix(((ulong)id<<4)+(uint)slot)%(uint)(oldRemaining+pool.Count));
                if(draw>=pool.Count){oldRemaining--;continue;}
                int k=pool[draw];pool.RemoveAt(draw);chosen[slot]=k;
                int marker=Marker+4*k+tier;
                swaps.Add((ids[slot],marker));ids[slot]=marker;
            }
            a[8]=string.Join(',',ids);picks[id]=chosen;replaced[id]=swaps.ToArray();
        }
        foreach(var row in weapons.Rows){
            int id=row.ID/10000*10000;if(!variants.TryGetValue(id,out var a))continue;
            int tier=int.Parse(a[3]);
            var source=originals[int.Parse(a[1])+row.ID-id];
            foreach(var (old,marker) in replaced[id]){
                var slot=resident.Single(k=>I(row,k)==old);
                Set(row,slot,marker);
            }
            foreach(int k in new[]{0,1}){
                bool on=picks[id].Contains(k);
                if(!on&&oldTraits.GetValueOrDefault(id,-1)!=k)continue;
                string field=k==0?"attackBaseStamina":"bowDistRate";
                int basis=I(source,field);
                Set(row,field,!on?basis:k==0?Math.Round(basis*(1+Values[k][tier]/100.0),MidpointRounding.AwayFromZero):basis+Values[k][tier]);
            }
        }
        for(int k=0;k<9;k++)for(int tier=0;tier<4;tier++){
            var marker=Clone(effects,Marker+k*4+tier,9620000);
            Set(marker,"toughnessDamageCutRate",1);Set(marker,"vfxId",-1);
        }
        for(int tier=0;tier<4;tier++)
        foreach(int kind in Enumerable.Range(3,7)){
            var row=Clone(effects,Effect+tier*16+kind,9620000);
            Set(row,"toughnessDamageCutRate",1);Set(row,"vfxId",-1);
            if(kind==3)Set(row,"dexterityCancelSystemOnlyAddDexterity",Values[kind][tier]);
            if(kind==9)Set(row,"deleteCriteriaDamage",1);
            if(kind is >=4 and <=8){
                float cut=1-Values[kind][tier]/100f;
                foreach(var e in new[]{"neutral","slash","blow","thrust","magic","fire","thunder","dark"})Set(row,e+"DamageCutRate",cut);
            }
        }
        foreach(var (file,param) in changed){param.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=param.Write();}
        var dest=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(dest,reg);
        if(new FileInfo(dest).Length>=4799*1024)throw new Exception("regulation.bin exceeds the game budget");
        string Update(int id,string text,string sep,int lang){
            if(oldTraits.TryGetValue(id,out int legacy))text=text.Replace(sep+Labels[legacy][lang],"");
            var slots=picks[id];
            if(!slots.Any(k=>k>=0))return text;
            var suffix=Regex.Match(text,@"(?:</font>)+$").Value;
            var lines=text[..^suffix.Length].Split(sep);
            int offset=lines.Length-slots.Length;
            if(offset<0)throw new Exception($"Missing weapon affix lines {id}");
            for(int i=0;i<slots.Length;i++)if(slots[i]>=0)lines[offset+i]=Label(slots[i],int.Parse(variants[id][3]),lang);
            return string.Join(sep,lines)+suffix;
        }
        var details=File.ReadAllLines(Path.Combine(input,"affix-v2-details.tsv"));
        for(int i=0;i<details.Length;i++){
            var parts=details[i].Split('\t');int id=int.Parse(parts[0]);
            if(!variants.ContainsKey(id))continue;
            foreach(int col in new[]{3,4})parts[col]=Update(id,parts[col],"\\n",0);
            details[i]=string.Join('\t',parts);
        }
        File.WriteAllLines(Path.Combine(output,"affix-v2-details.tsv"),details);
        var remap=File.ReadAllLines(Path.Combine(input,"affix-effect-remap.tsv"));
        for(int i=0;i<remap.Length;i++){
            var parts=remap[i].Split('\t');
            if(!picks.TryGetValue(int.Parse(parts[0]),out var slots)||!slots.Any(k=>k>=0))continue;
            var lines=parts[2].Split("\\n");
            if(lines.Length!=slots.Length)throw new Exception($"Unexpected remap slot count {parts[0]}");
            for(int j=0;j<slots.Length;j++)if(slots[j]>=0)lines[j]=Label(slots[j],int.Parse(variants[int.Parse(parts[0])][3]),0);
            parts[2]=string.Join("\\n",lines);remap[i]=string.Join('\t',parts);
        }
        File.WriteAllLines(Path.Combine(output,"affix-effect-remap.tsv"),remap);
        File.WriteAllLines(Path.Combine(output,"affix-v2-audit.tsv"),audit.Select(a=>string.Join('\t',a)));
        var rolls=picks.Where(e=>e.Value.Any(k=>k>=0)).OrderBy(e=>e.Key).ToArray();
        File.WriteAllLines(Path.Combine(output,"weapon-traits.tsv"),rolls.Select(e=>$"{e.Key}\t{e.Value.Where(k=>k>=0).Sum(k=>1<<k)}\t{variants[e.Key][3]}"));
        File.WriteAllLines(Path.Combine(output,"weapon-trait-text.tsv"),from k in Enumerable.Range(0,9) from t in Enumerable.Range(0,4)
            select string.Join('\t',Enumerable.Range(0,3).Select(l=>Label(k,t,l))));
        foreach(var (lang,index) in new[]{("zhocn",0),("zhotw",1),("engus",2)}){
            string rel=Path.Combine("msg",lang,"item_dlc02.msgbnd.dcx");
            var b=BND4.Read(Path.Combine(input,rel));
            foreach(int id in new[]{21,25}){
                var f=b.Files.Single(f=>f.ID==id);var t=FMG.Read(f.Bytes);
                var entries=t.Entries.Where(e=>variants.ContainsKey(e.ID)).ToArray();
                if(entries.Length!=variants.Count)throw new Exception($"Missing {lang} weapon text in {id} ({entries.Length}/{variants.Count})");
                foreach(var e in entries){e.Text=Update(e.ID,e.Text,"\n",index);}
                f.Bytes=t.Write();
            }
            Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,rel)));b.Write(Path.Combine(output,rel));
        }
        Console.WriteLine($"{rolls.Length} weapons rolled new traits; {tagged} charged attack rows");
    }
}
