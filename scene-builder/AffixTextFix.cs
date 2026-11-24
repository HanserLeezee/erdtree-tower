using SoulsFormats;
using System.Text.RegularExpressions;
static class AffixTextFix {
    public const int WeaponCaptionLines=7,CaptionLines=WeaponCaptionLines,ProtectorCaptionLines=12,AccessoryCaptionLines=12,EnglishWrapWidth=72;
    static readonly Dictionary<string,string> Labels=new(){["STR"]="力气",["DEX"]="灵巧",["INT"]="智力",["FAI"]="信仰",["ARC"]="感应"};
    static readonly Regex Build=new(@"(?<![A-Za-z])(?:STR|DEX|INT|FAI|ARC)(?:/(?:STR|DEX|INT|FAI|ARC))*(?![A-Za-z])");
    public static string Convert(string text)=>Build.Replace(text,m=>m.Value.Split('/').Length==5?"全属性":string.Join('/',m.Value.Split('/').Select(t=>Labels[t])));
    static bool Variant(int id)=>id>=100000000&&id<120000000||id>=160000000&&id<200000000;
    static bool ArmorVariant(int id)=>id>=10000000&&id<15000000||id>=20000000&&id<40000000;
    static bool Advanced(int id)=>id>=160000000&&id<200000000;
    static List<string> WrapLines(string paragraph,int wrapWidth) {
        if(wrapWidth<=0)return paragraph.Split('\n').ToList();
        var words=Regex.Split(paragraph.Trim(),@"\s+").Where(w=>w.Length>0).ToArray();
        if(words.Length==0)return new List<string>{""};
        var lines=new List<string>();var current=words[0];
        for(int i=1;i<words.Length;i++) {
            if(current.Length+1+words[i].Length<=wrapWidth)current+=" "+words[i];
            else{lines.Add(current);current=words[i];}
        }
        lines.Add(current);return lines;
    }
    // 词条块置顶，背景故事放下方并按原版文本框行数上限截取（武器 7 行、防具与护符 12 行）
    public static string FitTop(string affix,string lore,int maxLines=CaptionLines,int wrapWidth=0) {
        affix=affix.Trim('\n');lore=(lore??"").Trim('\n');
        int affixLines=string.IsNullOrEmpty(affix)?0:affix.Split('\n').Length;
        int budget=maxLines-affixLines-1;
        if(budget<=0||string.IsNullOrWhiteSpace(lore))return affix;
        int used=0;var kept=new List<string>();
        foreach(var paragraph in lore.Split("\n\n")) {
            var lines=WrapLines(paragraph,wrapWidth);int need=lines.Count+(kept.Count>0?1:0);
            if(used+need>budget){if(kept.Count==0&&budget>0)kept.Add(string.Join('\n',lines.Take(budget)));break;}
            kept.Add(string.Join('\n',lines));used+=need;
        }
        return kept.Count==0?affix:affix+"\n\n"+string.Join("\n\n",kept);
    }
    internal static bool IsAffixTop(string caption) {
        int split=caption.IndexOf("\n\n",StringComparison.Ordinal);
        if(split<0)return true;
        string head=caption[..split];
        if(head.StartsWith("<font ",StringComparison.Ordinal)&&head.EndsWith("</font>",StringComparison.Ordinal))return true;
        if(head.StartsWith("补正组合：",StringComparison.Ordinal)||head.StartsWith("防具类型：",StringComparison.Ordinal)
            ||head.StartsWith("Scaling:",StringComparison.Ordinal)||head.StartsWith("Armor Type:",StringComparison.Ordinal))return true;
        return !head.Contains('。')&&head.Split('\n').All(l=>l=="保留全部固有效果"||l=="Retains all intrinsic effects"||Regex.IsMatch(l,@"[+-]\d"));
    }
    // 兼容旧版「上故事、下词条」与新版「上词条、下故事」，统一输出词条置顶且符合行数上限的说明
    public static string Fit(string caption,int maxLines=CaptionLines,int wrapWidth=0) {
        if(string.IsNullOrEmpty(caption)||!caption.Contains("\n\n"))return caption;
        if(IsAffixTop(caption)) {
            int head=caption.IndexOf("\n\n",StringComparison.Ordinal);
            return FitTop(caption[..head],caption[(head+2)..],maxLines,wrapWidth);
        }
        int tail=caption.LastIndexOf("\n\n",StringComparison.Ordinal);
        return FitTop(caption[(tail+2)..],caption[..tail],maxLines,wrapWidth);
    }
    // 词条池重做（affix-pool-patch）：按对照表替换词条行（兼容词条置顶与旧版末尾两种布局）
    static Dictionary<int,(string old,string @new)> Remap=new();
    public static Dictionary<int,(string,string)> LoadRemap(string input) {
        var path=Path.Combine(input,"affix-effect-remap.tsv");if(!File.Exists(path))return new();
        return File.ReadAllLines(path).Select(l=>l.Split('\t')).ToDictionary(p=>int.Parse(p[0]),p=>(p[1].Replace("\\n","\n"),p[2].Replace("\\n","\n")));
    }
    static string RemapBlock(string block,int fmg,int id,(string old,string @new) r) {
        var suffix=Regex.Match(block,@"(?:</font>)+$").Value;
        var body=block[..^suffix.Length];
        if(body.EndsWith(r.@new))return block;
        if(!body.EndsWith(r.old))throw new Exception($"Affix effect lines not found in text {fmg}/{id}");
        return body[..^r.old.Length]+r.@new+suffix;
    }
    static string Remapped(int fmg,int id,string text) {
        if(text==null||!(fmg is 21 or 22 or 25 or 26)||((fmg is 21 or 25)&&!Variant(id))||((fmg is 22 or 26)&&!ArmorVariant(id))||!Remap.TryGetValue(id,out var r))return text;
        if((fmg is 25 or 26)&&text.Contains("\n\n")&&IsAffixTop(text)) {
            int split=text.IndexOf("\n\n",StringComparison.Ordinal);
            return RemapBlock(text[..split],fmg,id,r)+text[split..];
        }
        return RemapBlock(text,fmg,id,r);
    }
    static string Expected(int fmg,int id,string text) {
        text=Remapped(fmg,id,text);
        if(text==null)return null;
        if(fmg==25&&Variant(id))return Fit(Advanced(id)?Convert(text):text,WeaponCaptionLines);
        if(fmg==26&&ArmorVariant(id))return Fit(text,ProtectorCaptionLines);
        return (fmg is 11 or 21)&&Advanced(id)?Convert(text):text;
    }
    public static void Test(string input) {
        var saved=Remap;Remap=new(){[100000000]=("力量 +1","战技伤害 +6%"),[10000000]=("力量 +1","战技伤害 +6%")};
        foreach(var text in new[]{"力量 +1","<font color=\"#85BFFF\">力量 +1</font>"}) {
            var expected=text.Replace("力量 +1","战技伤害 +6%");
            if(Remapped(21,100000000,text)!=expected||Remapped(21,100000000,expected)!=expected)throw new Exception("Colored affix remap must preserve markup and be idempotent");
        }
        foreach(var text in new[]{"背景\n\n<font color=\"#85BFFF\">力量 +1</font>","<font color=\"#85BFFF\">力量 +1</font>\n\n背景"}) {
            var expected=text.Replace("力量 +1","战技伤害 +6%");
            if(Remapped(25,100000000,text)!=expected||Remapped(25,100000000,expected)!=expected)throw new Exception("Top/bottom weapon caption remap must preserve markup and be idempotent");
        }
        if(Remapped(25,10000000,"Base weapon lore")!="Base weapon lore")throw new Exception("Armor remaps must not affect base weapons with the same ID");
        if(!Variant(119230000)||!Variant(198470000)||!Advanced(198470000))throw new Exception("DLC weapon variants must be included");
        if(!ArmorVariant(10000000)||!ArmorVariant(31647000)||ArmorVariant(5370300))throw new Exception("Armor variant range mismatch");
        var coloredSample="<font color=\"#B484FF\">防具类型：魔法\n偏重元素防护，不提供物理减伤\n魔法伤害 +8%\n最大专注 +5%</font>";
        var longLore="段一甲\n段一乙\n段一丙\n\n能强化黄金律法的祷告。\n\n段三甲\n段三乙\n\n段四甲\n段四乙";
        var fittedColor=Fit(longLore+"\n\n"+coloredSample,ProtectorCaptionLines);
        if(!fittedColor.StartsWith(coloredSample,StringComparison.Ordinal)||fittedColor.Split('\n').Length>ProtectorCaptionLines||Fit(fittedColor,ProtectorCaptionLines)!=fittedColor)
            throw new Exception("Colored armor caption top layout failed or is not idempotent");
        if(!fittedColor.Contains("能强化黄金律法的祷告。"))throw new Exception("Special effect paragraph should fit below 4-line armor affix");
        var enFitted=FitTop("<font color=\"#B484FF\">Armor Type: Magic\nFocuses on elemental negation\nSorcery damage +8%</font>",
            "A radiant mask modeled after the Golden Order. Crafted by Goldmask, the austere scholar of Fundamentalism.\n\nStrengthens Golden Order incantations.\n\nThis overwhelming inspiration descended upon him at the end of his search.",
            ProtectorCaptionLines,EnglishWrapWidth);
        if(enFitted.Split('\n').Length>ProtectorCaptionLines||enFitted.Split('\n').Skip(4).Any(l=>l.Length>EnglishWrapWidth))
            throw new Exception("English wrapped armor caption exceeded budget");
        Remap=saved;
        int weapons=0,weaponTrimmed=0,armors=0,armorTrimmed=0;
        foreach(var file in new[]{"affix-details.tsv","affix-v2-details.tsv"})foreach(var p in File.ReadAllLines(Path.Combine(input,file)).Select(l=>l.Split('\t'))) {
            long encoded=long.Parse(p[0]);bool isWeapon=encoded<0x10000000;
            var caption=Convert(p[4].Replace("\\n","\n"));int split=caption.LastIndexOf("\n\n",StringComparison.Ordinal);
            var lore=caption[..split];var info=caption[(split+2)..];
            int limit=isWeapon?WeaponCaptionLines:ProtectorCaptionLines;
            var fitted=Fit(caption,limit);
            if(fitted.Split('\n').Length>limit||!fitted.StartsWith(info,StringComparison.Ordinal)||Fit(fitted,limit)!=fitted)
                throw new Exception($"Caption fit failed: {p[0]}");
            if(isWeapon){weapons++;if(fitted.Length<caption.Length)weaponTrimmed++;}
            else{armors++;if(fitted.Length<caption.Length)armorTrimmed++;}
            if(p[0]=="179510000")Console.WriteLine(fitted);
        }
        int talismans=0,talismanTrimmed=0;
        var talismanPath=Path.Combine(input,"talisman-passive-details.tsv");
        if(File.Exists(talismanPath))foreach(var p in File.ReadAllLines(talismanPath).Select(l=>l.Split('\t'))) {
            var caption=p[4].Replace("\\n","\n");int split=caption.LastIndexOf("\n\n",StringComparison.Ordinal);
            var info=caption[(split+2)..];var fitted=FitTop(info,caption[..split],AccessoryCaptionLines);
            if(fitted.Split('\n').Length>AccessoryCaptionLines||!fitted.StartsWith(info,StringComparison.Ordinal))
                throw new Exception($"Talisman caption fit failed: {p[0]}");
            talismans++;if(fitted.Length<caption.Length)talismanTrimmed++;
        }
        Console.WriteLine($"Checked {weapons} weapons ({weaponTrimmed} trimmed <= {WeaponCaptionLines}L), {armors} armors ({armorTrimmed} trimmed <= {ProtectorCaptionLines}L), {talismans} talismans ({talismanTrimmed} trimmed <= {AccessoryCaptionLines}L); all affixes on top and idempotent");
        // 对照表：新词条行都已写进奖励详情，且没有残留旧的属性点词条
        var remap=LoadRemap(input);var details=new[]{"affix-details.tsv","affix-v2-details.tsv"}.SelectMany(f=>File.ReadAllLines(Path.Combine(input,f))).Select(l=>l.Split('\t')).ToDictionary(p=>long.Parse(p[0])&0x0fffffff,p=>p[3].Replace("\\n","\n"));
        foreach(var (id,(_,fresh)) in remap)if(!details.TryGetValue(id,out var info)||!info.EndsWith(fresh))throw new Exception($"Remap target missing from details: {id}");
        var stale=details.Values.Count(v=>System.Text.RegularExpressions.Regex.IsMatch(v,@"(?:^|\n)(力量|灵巧|智力|信仰|感应) \+\d"));
        if(stale>0)throw new Exception($"{stale} details still list attribute-point affixes");
        Console.WriteLine($"Checked {remap.Count} affix text remaps against the reward details");
    }
    /// 敌人显示名改名（旧名 → 新名）：enemy-pools.txt 里改了名字的敌人，写文字时把已有的那一条换成新名
    static readonly Dictionary<string,string> PoolRenames=new(){{"小姓","随从"}};
    public const int PoolNameFirst=9610000,BarNameFirst=9700000,BarNameStride=200,BossNameFirst=9600000,BarMarker=9620099;
    // 新增 8 种 Boss 词条后的组合：接在最早的 111 个组合之后，编号区间与标记见 src/boss_affixes.rs
    public const int LegacyCombos=111,BarNameSecond=9722200,BarNameSecondStride=144,BarMarkerV2=9620098,BarTagTest=9799999;
    // 之后追加的 Boss（名称序号 140 起，追忆与 DLC）：序号 144 起的新组合名称放在第三段，写好后加入第三个标记（见 src/boss_affixes.rs）
    public const int LateBossFirst=140,BarNameThird=9740000,BarNameThirdStride=64,BarMarkerV3=9620097;
    public const int PoolBarNameFirst=98000000,PoolBarNameStride=256,BarMarkerV4=9620100;
    public const int PreAdaptiveCombos=215,BarNameAdaptive=9750000,BarNameAdaptiveStride=256,BarMarkerV5=9620101,BarMarkerV6=9620102;
    // 物品名颜色标签测试：把爬塔里用不到的原版道具“饶舌头颅”（2207）的名字换成带颜色标签的文字，调试菜单可以发放，用来确认原版菜单是否支持标签
    public const int TagTestGoods=2207;
    public const string TagTestGoodsName="<font color=\"#F2C85C\">传说</font>·<font color=\"#68CC92\">灵巧</font>/<font color=\"#EECE6C\">信仰</font>·颜色标签测试";
    // 与 src/boss_affixes.rs 的 bar_name 相同：最早的组合 9700000 + 组合序号 × 200 + Boss 名称序号，之后的 9722200 + (组合序号 − 111) × 144 + Boss 名称序号，
    // 名称序号 144 起放在第三段 9740000 + (组合序号 − 111) × 64 + (Boss 名称序号 − 144)
    static IEnumerable<(int id,string text)> AffixBarNames(string input,Dictionary<int,string> names) {
        var combos=File.ReadAllLines(Path.Combine(input,"boss-affix-combos.txt")).Select(l=>l.Split('|')).ToArray();
        var majors=File.ReadAllLines(Path.Combine(input,"boss-pool.txt")).Select(l=>l.Split('|')).Select(p=>int.Parse(p[2])).Distinct();
        foreach(var index in majors) {
            if(index>=BarNameSecondStride+BarNameThirdStride||index>=BarNameStride)throw new Exception("Boss name index exceeds affix bar stride");
            var boss=names.GetValueOrDefault(BossNameFirst+index)??throw new Exception($"Missing boss name {BossNameFirst+index}");
            foreach(var combo in combos) {
                int number=int.Parse(combo[0]);
                int id=number>=PreAdaptiveCombos?BarNameAdaptive+(number-PreAdaptiveCombos)*BarNameAdaptiveStride+index
                    :number<LegacyCombos?BarNameFirst+number*BarNameStride+index
                    :index<BarNameSecondStride?BarNameSecond+(number-LegacyCombos)*BarNameSecondStride+index
                    :BarNameThird+(number-LegacyCombos)*BarNameThirdStride+index-BarNameSecondStride;
                yield return (id,$"{combo[1]} {boss}");
            }
        }
        var pool=PoolNames(input);
        if(pool.Count>PoolBarNameStride)throw new Exception("Enemy name index exceeds affix bar stride");
        for(int index=0;index<pool.Count;index++)foreach(var combo in combos)
            yield return (PoolBarNameFirst+int.Parse(combo[0])*PoolBarNameStride+index,$"{combo[1]} {pool[index]}");
        // 调试菜单“血条颜色标签测试”用的文字：确认 Boss 血条是否支持 <font color> 标签（与 native boss_affixes::BAR_TAG_TEST_NAME 相同）
        yield return (BarTagTest,"<font color=\"#E04A3A\">狂暴的</font>·<font color=\"#F2C85C\">黄金律法的</font>·<font color=\"#5CDECC\">迅猛的</font> 颜色标签测试");
    }
    static bool AddMarker(string input,string output) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs","SpEffect.xml"));var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        // 第一个标记：最早的组合已有名称；第二个标记：新增词条后的全部组合都已有名称
        var missing=new[]{(BarMarker,"Tower boss affix bar names marker"),(BarMarkerV2,"Tower boss affix bar names marker v2"),(BarMarkerV3,"Tower boss affix bar names marker v3"),(BarMarkerV4,"Tower elite affix bar names marker v4"),(BarMarkerV5,"Tower adaptive affix bar names marker v5"),(BarMarkerV6,"Tower cost-v6 affix bar names marker v6")}.Where(m=>!effects.Rows.Any(r=>r.ID==m.Item1)).ToList();
        if(missing.Count==0)return false;
        foreach(var (id,name) in missing) {
            var marker=new PARAM.Row(effects.Rows.Single(r=>r.ID==BossAffixAssets.First+95)){ID=id,Name=name};
            marker["effectEndurance"].Value=0f;marker["vfxId"].Value=-1;effects.Rows.Add(marker);
        }
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());file.Bytes=effects.Write();
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        if(actual.Rows.Count!=effects.Rows.Count||new[]{BarMarker,BarMarkerV2,BarMarkerV3,BarMarkerV4,BarMarkerV5,BarMarkerV6}.Any(id=>!actual.Rows.Any(r=>r.ID==id)))throw new Exception("Bar name marker not written");
        return true;
    }
    // 侍从的敌人预览（menu_dlc02 第 33 表，9510000+序号×16+数量）：与 melina-patch 写入的内容相同，新增敌人时补上；
    // 同表的问号房赌运事件菜单文字（RecoveryText.GambleEntries）也在这里写入
    static int UpdatePreviewNames(string input,string output) {
        const string msg="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,msg));var file=menu.Files.Single(f=>f.ID==MelinaAssets.PreviewMenu);var fmg=FMG.Read(file.Bytes);
        var before=menu.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text));
        var expected=MelinaAssets.PreviewTexts(MelinaAssets.PreviewNames(input)).ToDictionary(t=>t.id,t=>t.text);
        // 同一张表里还有问号房赌运事件的对话菜单文字，一并补上
        foreach(var (id,text) in RecoveryText.GambleEntries)expected[id]=text;
        foreach(var (id,text) in FlaskBatchText.Chinese)expected[id]=text;
        int changed=0;
        foreach(var (id,text) in expected) {
            var entry=fmg.Entries.FirstOrDefault(e=>e.ID==id);
            if(entry?.Text==text)continue;
            fmg.Entries.RemoveAll(e=>e.ID==id);fmg.Entries.Add(new FMG.Entry(id,text));changed++;
        }
        if(changed==0)return 0;
        fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=fmg.Write();
        var dest=Path.Combine(output,msg);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);menu.Write(dest);
        var check=BND4.Read(dest).Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text));
        foreach(var (table,entries) in before)foreach(var (id,text) in entries)
            if(check[table].GetValueOrDefault(id)!=(table==MelinaAssets.PreviewMenu&&expected.TryGetValue(id,out var t)?t:text))throw new Exception($"Unexpected menu text change: {table}/{id}");
        foreach(var (id,text) in expected)if(check[MelinaAssets.PreviewMenu].GetValueOrDefault(id)!=text)throw new Exception($"Enemy preview name missing: {id}");
        return changed;
    }
    // 与 native/src/enemies.rs 的 POOL_NAMES 相同：按 enemy-pools.txt 行序去重
    public static List<string> PoolNames(string input) {var names=new List<string>();foreach(var p in File.ReadAllLines(Path.Combine(input,"enemy-pools.txt")).Select(l=>l.Split('|')))if(!names.Contains(p[4]))names.Add(p[4]);return names;}
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        Remap=LoadRemap(input);
        var messages=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));var changed=0;var added=0;
        changed+=AffinityNameAssets.Apply(input,messages);
        var before=messages.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.Select(e=>(e.ID,e.Text)).ToArray());
        {
            var goods=messages.Files.Single(f=>f.ID==10);var fmg=FMG.Read(goods.Bytes);
            var entry=fmg.Entries.FirstOrDefault(e=>e.ID==TagTestGoods)??throw new Exception("Tag test goods name missing");
            if(entry.Text!=TagTestGoodsName){entry.Text=TagTestGoodsName;goods.Bytes=fmg.Write();changed++;}
        }
        foreach(var file in messages.Files.Where(f=>f.ID is 11 or 21 or 22 or 25 or 26)) {
            var fmg=FMG.Read(file.Bytes);
            foreach(var entry in fmg.Entries) {var text=Expected(file.ID,entry.ID,entry.Text);if(text!=entry.Text){entry.Text=text;changed++;}}
            file.Bytes=fmg.Write();
        }
        var retired=File.ReadAllLines(Path.Combine(input,"retired-spell-rows.txt")).Where(l=>l.StartsWith("EquipParamGoods|")).Select(l=>int.Parse(l.Split('|')[1])).ToHashSet();var dropped=0;
        foreach(var file in messages.Files.Where(f=>f.ID is 10 or 20 or 24)) {
            var fmg=FMG.Read(file.Bytes);dropped+=fmg.Entries.RemoveAll(e=>retired.Contains(e.ID));file.Bytes=fmg.Write();
        }
        var poolNames=PoolNames(input);var npcFile=messages.Files.First(f=>f.Name.Contains("NpcName"));var npcNames=FMG.Read(npcFile.Bytes);
        // Boss 名称（9600000 + 名称序号）：原有的由完整构建写入；之后追加的 Boss（boss-pool.txt 末尾）在这里补上
        var bossNames=BossAssets.BarNames(input);
        var legacyBossNames=BossAssets.Parts(input).ToDictionary(p=>BossNameFirst+p.Index,p=>p.Name);
        var legacyBarNames=AffixBarNames(input,legacyBossNames).ToDictionary(p=>p.id,p=>p.text);
        var renamedBars=new Dictionary<int,string>();
        foreach(var (id,name) in bossNames) {
            var existing=npcNames.Entries.FirstOrDefault(e=>e.ID==id);
            if(existing==null){npcNames.Entries.Add(new FMG.Entry(id,name));added++;}
            else if(existing.Text!=name) {
                if(existing.Text!=legacyBossNames[id])throw new Exception($"Boss name collision: {id}");
                existing.Text=name;renamedBars.Add(id,name);changed++;
            }
        }
        for(int i=0;i<poolNames.Count;i++) {
            var existing=npcNames.Entries.FirstOrDefault(e=>e.ID==PoolNameFirst+i);
            if(existing==null){npcNames.Entries.Add(new FMG.Entry(PoolNameFirst+i,poolNames[i]));added++;}
            else if(existing.Text!=poolNames[i]) {
                // 只允许 PoolRenames 里登记过的改名（编号不变，只换显示名）；其余不一致仍视为编号错位
                if(!PoolRenames.TryGetValue(existing.Text??"",out var renamed)||renamed!=poolNames[i])throw new Exception($"Enemy bar name collision: {PoolNameFirst+i}");
                existing.Text=poolNames[i];renamedBars.Add(PoolNameFirst+i,poolNames[i]);changed++;
            }
        }
        var barNames=AffixBarNames(input,npcNames.Entries.ToDictionary(e=>e.ID,e=>e.Text)).ToList();
        foreach(var (id,text) in barNames) {
            var existing=npcNames.Entries.FirstOrDefault(e=>e.ID==id);
            if(existing==null){npcNames.Entries.Add(new FMG.Entry(id,text));added++;}
            else if(existing.Text!=text) {
                var normalized=existing.Text?.Replace("涅槃的","不屈的",StringComparison.Ordinal);
                if(normalized!=text&&(!legacyBarNames.TryGetValue(id,out var legacy)||normalized!=legacy))throw new Exception($"Affix bar name collision: {id}: '{existing.Text}' -> '{text}'");
                existing.Text=text;renamedBars.Add(id,text);changed++;
            }
        }
        npcNames.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));npcFile.Bytes=npcNames.Write();
        bool marker=AddMarker(input,output);
        int previews=UpdatePreviewNames(input,output);
        if(changed==0&&added==0&&dropped==0){Console.WriteLine((marker?"Item text already updated; wrote regulation.bin with the affix bar name marker":"Item text and affix bar name marker already present; nothing written")+$"; {previews} enemy preview names updated");return;}
        var dir=Path.Combine(output,"msg/zhocn");Directory.CreateDirectory(dir);var path=Path.Combine(dir,"item_dlc02.msgbnd.dcx");messages.Write(path);
        var written=BND4.Read(path).Files;var check=written.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text));
        foreach(var (fmg,entries) in before)foreach(var (id,text) in entries) {
            if(fmg==npcFile.ID&&renamedBars.TryGetValue(id,out var renamed)) {
                if(check[fmg][id]!=renamed)throw new Exception($"Affix rename mismatch: {id}");
                continue;
            }
            if((fmg is 10 or 20 or 24)&&retired.Contains(id)){if(check[fmg].ContainsKey(id))throw new Exception($"Retired spell text remains: {fmg}/{id}");continue;}
            if(check[fmg][id]!=(fmg==10&&id==TagTestGoods?TagTestGoodsName:(fmg is 11 or 21 or 22 or 25 or 26)?Expected(fmg,id,text):text))throw new Exception($"Unexpected text change: {fmg}/{id}");
            if(fmg==25&&Variant(id)&&check[fmg][id]?.Split('\n').Length>WeaponCaptionLines)throw new Exception($"Weapon caption still exceeds {WeaponCaptionLines} lines: {id}");
            if(fmg==26&&ArmorVariant(id)&&check[fmg][id]?.Split('\n').Length>ProtectorCaptionLines)throw new Exception($"Armor caption still exceeds {ProtectorCaptionLines} lines: {id}");
        }
        var npcCheck=FMG.Read(written.First(f=>f.Name.Contains("NpcName")).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
        for(int i=0;i<poolNames.Count;i++)if(npcCheck.GetValueOrDefault(PoolNameFirst+i)!=poolNames[i])throw new Exception($"Enemy bar name missing: {poolNames[i]}");
        foreach(var (id,text) in barNames)if(npcCheck.GetValueOrDefault(id)!=text)throw new Exception($"Affix bar name missing: {id}");
        foreach(var (id,name) in bossNames)if(npcCheck.GetValueOrDefault(id)!=name)throw new Exception($"Boss name missing: {id}");
        var known=before[npcFile.ID].Select(e=>e.ID).Concat(barNames.Select(b=>b.id)).Concat(bossNames.Keys).ToHashSet();
        foreach(var (id,_) in npcCheck)if(!known.Contains(id)&&!(id>=PoolNameFirst&&id<PoolNameFirst+poolNames.Count))throw new Exception($"Unexpected NPC name added: {id}");
        foreach(var line in File.ReadAllLines(Path.Combine(input,"affix-v2-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="weapon")) {
            if(!EquipmentStyleAssets.SameName(check[11].GetValueOrDefault(int.Parse(line[1])),line[4]))throw new Exception($"Weapon name differs from catalog: {line[1]}");
        }
        Console.WriteLine($"Updated {previews} enemy preview names; updated {changed} affix weapon text entries (localized scaling labels, captions fit {CaptionLines} lines) and added {added} bar names (enemies {PoolNameFirst}-{PoolNameFirst+poolNames.Count-1}, {barNames.Count} affix boss titles from {BarNameFirst}){(marker?"; wrote regulation.bin with the affix bar name marker":"")}, removed {dropped} retired spell text entries; all other text unchanged");
    }

    public static void PatchAdaptive(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        const string relative="msg/zhocn/item_dlc02.msgbnd.dcx";
        var messages=BND4.Read(Path.Combine(input,relative));
        var file=messages.Files.First(f=>f.Name.Contains("NpcName"));var names=FMG.Read(file.Bytes);
        var before=names.Entries.ToDictionary(e=>e.ID,e=>e.Text);
        var expected=AffixBarNames(input,before).Where(p=>p.text.Split(' ')[0].Contains("适应的")).ToDictionary(p=>p.id,p=>p.text);
        if(expected.Count==0)throw new Exception("Adaptive combo table is missing");
        foreach(var (id,text) in expected) {
            var existing=names.Entries.SingleOrDefault(e=>e.ID==id);
            if(existing!=null&&existing.Text!=text)throw new Exception($"Adaptive name collision: {id}");
            if(existing==null)names.Entries.Add(new FMG.Entry(id,text));
        }
        names.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=names.Write();
        var path=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(path)!);messages.Write(path);
        var written=BND4.Read(path);var actual=FMG.Read(written.Files.Single(f=>f.ID==file.ID).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
        foreach(var (id,text) in before)if(actual.GetValueOrDefault(id)!=text)throw new Exception($"Existing NPC name changed: {id}");
        foreach(var (id,text) in expected)if(actual.GetValueOrDefault(id)!=text)throw new Exception($"Adaptive NPC name missing: {id}");
        foreach(var other in messages.Files.Where(f=>f.ID!=file.ID))if(!written.Files.Single(f=>f.ID==other.ID).Bytes.SequenceEqual(other.Bytes))throw new Exception($"Unrelated FMG changed: {other.ID}");
        AddMarker(input,output);
        Console.WriteLine($"Verified {expected.Count} adaptive boss/elite names; all existing names and unrelated FMGs preserved");
    }

    static string ExtractTalismanAffix(string text,string baseLore,int wrapWidth) {
        if(string.IsNullOrEmpty(text))return "";
        if(!string.IsNullOrEmpty(baseLore)&&text.StartsWith(baseLore+"\n\n",StringComparison.Ordinal))
            return text[(baseLore.Length+2)..];
        int first=text.IndexOf("\n\n",StringComparison.Ordinal);
        if(first>0&&!string.IsNullOrEmpty(baseLore)) {
            string tailFirstLine=text[(first+2)..].Split('\n')[0];
            string wrappedHead=WrapLines(baseLore.Split("\n\n")[0],wrapWidth)[0];
            if(!string.IsNullOrWhiteSpace(tailFirstLine)&&(baseLore.StartsWith(tailFirstLine,StringComparison.Ordinal)||wrappedHead==tailFirstLine))
                return text[..first];
        }
        int last=text.LastIndexOf("\n\n",StringComparison.Ordinal);
        return last<0?text:text[(last+2)..];
    }

    static string RefreshArmorTagline(string text)=>text
        .Replace("偏重物理防护，不提供元素减伤","偏重物理防护，元素减伤较低")
        .Replace("偏重元素防护，不提供物理减伤","偏重元素防护，物理减伤较低")
        .Replace("偏重物理防護，不提供元素減傷","偏重物理防護，元素減傷較低")
        .Replace("偏重元素防護，不提供物理減傷","偏重元素防護，物理減傷較低")
        .Replace("Focused on physical defense; no elemental damage negation.","Focused on physical defense; lower elemental damage negation.")
        .Replace("Focused on elemental defense; no physical damage negation.","Focused on elemental defense; lower physical damage negation.");

    // 词条置顶迁移补丁：将 zhocn / zhotw / engus 的词条武器（WeaponCaption）、防具（ProtectorInfo/ProtectorCaption）与词条护符（AccessoryCaption）
    // 说明统一改为「彩色词条块置顶 + 下方原版背景按行截取」（武器 7 行、防具与护符 12 行），并同步刷新防具类型说明，幂等且保留所有其他文本。
    public static void PatchCaptionTop(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var weapons=new Dictionary<int,int>();
        var armors=new Dictionary<int,int>();
        foreach(var catalog in new[]{"affix-catalog.txt","affix-v2-catalog.txt"}) {
            var path=Path.Combine(input,catalog);if(!File.Exists(path))continue;
            foreach(var p in File.ReadAllLines(path).Select(l=>l.Split('|'))) {
                int id=int.Parse(p[1]),baseId=int.Parse(p[2]);
                if(p[0]=="weapon")weapons[id]=baseId;else if(p[0]=="armor")armors[id]=baseId;
            }
        }
        var talismans=new Dictionary<int,int>();
        var talismanPath=Path.Combine(input,"talisman-passives.tsv");
        if(File.Exists(talismanPath))foreach(var p in File.ReadAllLines(talismanPath).Select(l=>l.Split('\t')))
            talismans[int.Parse(p[0])]=int.Parse(p[1]);
        foreach(var (lang,wrapWidth) in new[]{("zhocn",0),("zhotw",0),("engus",EnglishWrapWidth)}) {
            string relative=Path.Combine("msg",lang,"item_dlc02.msgbnd.dcx");
            string sourcePath=Path.Combine(input,relative);
            if(!File.Exists(sourcePath))continue;
            var binder=BND4.Read(sourcePath);
            var beforeBytes=binder.Files.ToDictionary(f=>f.ID,f=>f.Bytes.ToArray());
            var merged=ItemTextTables.Read(binder);
            var weaponInfo=merged["WeaponInfo"].Entries.ToDictionary(e=>e.ID,e=>e.Text??"");
            var protectorInfo=merged["ProtectorInfo"].Entries.ToDictionary(e=>e.ID,e=>e.Text??"");
            var baseWeaponLore=merged["WeaponCaption"].Entries.ToDictionary(e=>e.ID,e=>e.Text??"");
            var baseArmorLore=merged["ProtectorCaption"].Entries.ToDictionary(e=>e.ID,e=>e.Text??"");
            var baseTalismanLore=merged["AccessoryCaption"].Entries.ToDictionary(e=>e.ID,e=>e.Text??"");
            var touched=new Dictionary<int,string>();
            var expected=new Dictionary<int,Dictionary<int,string>>();
            int wCount=0,aCount=0,tCount=0,changed=0;
            foreach(var file in binder.Files) {
                var table=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];
                if(table is not ("WeaponCaption" or "ProtectorInfo" or "ProtectorCaption" or "AccessoryCaption"))continue;
                var fmg=FMG.Read(file.Bytes);
                var map=new Dictionary<int,string>();
                foreach(var entry in fmg.Entries) {
                    string text=entry.Text??"";
                    string updated=text;
                    if(table=="WeaponCaption") {
                        int root=weapons.ContainsKey(entry.ID)?entry.ID:entry.ID/10000*10000;
                        if(weapons.TryGetValue(root,out int baseId)) {
                            if(!weaponInfo.TryGetValue(entry.ID,out var affix)||string.IsNullOrWhiteSpace(affix))
                                affix=weaponInfo.GetValueOrDefault(root,"");
                            if(string.IsNullOrWhiteSpace(affix))throw new Exception($"Missing WeaponInfo for {lang}/{entry.ID}");
                            string lore=baseWeaponLore.GetValueOrDefault(baseId,"");
                            updated=FitTop(affix,lore,WeaponCaptionLines,wrapWidth);
                            wCount++;
                        }
                    } else if(table=="ProtectorInfo"&&armors.ContainsKey(entry.ID)) {
                        updated=RefreshArmorTagline(text);
                    } else if(table=="ProtectorCaption"&&armors.TryGetValue(entry.ID,out int armorBase)) {
                        if(!protectorInfo.TryGetValue(entry.ID,out var affix)||string.IsNullOrWhiteSpace(affix))
                            throw new Exception($"Missing ProtectorInfo for {lang}/{entry.ID}");
                        affix=RefreshArmorTagline(affix);
                        string lore=baseArmorLore.GetValueOrDefault(armorBase,"");
                        updated=FitTop(affix,lore,ProtectorCaptionLines,wrapWidth);
                        aCount++;
                    } else if(table=="AccessoryCaption"&&talismans.TryGetValue(entry.ID,out int talismanBase)) {
                        string lore=baseTalismanLore.GetValueOrDefault(talismanBase,"");
                        string affix=ExtractTalismanAffix(text,lore,wrapWidth);
                        if(string.IsNullOrWhiteSpace(affix))throw new Exception($"Missing talisman passive affix for {lang}/{entry.ID}");
                        updated=FitTop(affix,lore,AccessoryCaptionLines,wrapWidth);
                        tCount++;
                    }
                    if(updated!=text){entry.Text=updated;changed++;}
                    map[entry.ID]=entry.Text;
                }
                file.Bytes=fmg.Write();
                touched[file.ID]=table;
                expected[file.ID]=map;
            }
            string destPath=Path.Combine(output,relative);
            Directory.CreateDirectory(Path.GetDirectoryName(destPath)!);
            binder.Write(destPath);
            var verify=BND4.Read(destPath);
            foreach(var file in verify.Files) {
                if(!touched.TryGetValue(file.ID,out var table)) {
                    if(!file.Bytes.SequenceEqual(beforeBytes[file.ID]))throw new Exception($"Unrelated FMG changed in {lang}: {file.ID}");
                    continue;
                }
                var actual=FMG.Read(file.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text??"");
                var exp=expected[file.ID];
                if(actual.Count!=exp.Count)throw new Exception($"FMG entry count changed in {lang}/{file.ID}");
                foreach(var (id,text) in exp) {
                    if(actual.GetValueOrDefault(id)!=text)throw new Exception($"Roundtrip mismatch in {lang}/{file.ID}/{id}");
                    if(table=="ProtectorInfo")continue;
                    int limit=table=="WeaponCaption"?WeaponCaptionLines:table=="ProtectorCaption"?ProtectorCaptionLines:AccessoryCaptionLines;
                    bool isTarget=table=="WeaponCaption"?(weapons.ContainsKey(id)||weapons.ContainsKey(id/10000*10000)):table=="ProtectorCaption"?armors.ContainsKey(id):talismans.ContainsKey(id);
                    if(isTarget) {
                        if(text.Split('\n').Length>limit)throw new Exception($"Caption exceeds {limit} lines in {lang}/{file.ID}/{id}");
                        if(Regex.Matches(text,"<font ").Count!=Regex.Matches(text,"</font>").Count)throw new Exception($"Unbalanced color markup in {lang}/{file.ID}/{id}");
                    }
                }
            }
            Console.WriteLine($"{lang}: caption-top verified (weapons={wCount}, armors={aCount}, talismans={tCount}, changed={changed}); unrelated tables byte-identical");
        }
    }
}
