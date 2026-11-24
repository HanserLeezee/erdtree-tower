using SoulsFormats;

/// 修复两类战技 0 伤害问题：
/// 1. 战灰「罗蕾塔的斩击」（Gem 11800 / Art 118）在 UnrestrictedAshAssets 放开全武器挂载后，
///    原版 BehaviorParam_PC 仅对 5 类长柄武器（1000/1600/1700/1800/1900）定义了 judge 441..444，
///    缺少 variationId=0 通用兜底，导致装在其余 42 类武器上时只出起跳动作、完全无伤害。
///    现克隆 301000441..301000444（使用全武器通用的 100..110 剑身判定点）补齐 300000441..300000444。
/// 2. 亵渎圣剑（3140000 掠夺火焰）、罗蕾塔的战镰（18100000）及同源失色武器在高级词条（affix-v2）
///    随机洗掉原属性后，原版 AtkParam_Pc 将其他元素倍率写死为 0，导致战技打出 0 伤害。
///    现按武器原版属性结构补齐其他元素通道的 atk*Correction（保持原版武器 100% 数值不变，不触发双元素叠加）。
static class SkillDamageFixAssets {
    static int I(PARAM.Row row,string key)=>Convert.ToInt32(row[key].Value);
    static void Set(PARAM.Row row,string key,int value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());

    // 全元素对称补齐（保留 Phys 原值，把非 0 元素倍率同步到魔/火/雷/圣四系；仅用于原版单元素武器，原版数值 0 变动）
    static readonly int[] SpreadAllElements=[
        // 亵渎圣剑（3140000）掠夺火焰火浪
        300314902,300314903,300314904,
        // 黄金律法大剑（3170000）
        300317900,300317906,300317910,300317911,
        // 玛莉卡的石槌（11150000）
        301115915,
        // 吞世权杖（12200000）
        301202900,
        // 家族头连枷（13020000）
        301302900,301302901,301302902,301302903,301302904,301302905,
        // 罗泽司的斧（14120000）
        301412902,301412903,301412904,301412940,301412945,
        // 尊腐骑士矛（16040000）
        301604902,
        // 死亡仪式矛（16120000）
        301612910,301612911,
        // 罗蕾塔的战镰（18100000）罗蕾塔的斩击
        301810905,301810906,301810910,301810911,
        // 接肢飞龙（21060000）
        302106900,302106901,302106902,302106903,302106904,302106905,302106906,302106910,
        // 通用战灰（漏了圣倍率）
        300000853,
    ];

    // 纯物理原版武器战技：把物理倍率同步到四元素通道，避免纯元素高级词条失伤（原版四元素为 0，数值 0 变动）
    static readonly int[] SpreadFromPhysics=[
        // 风暴鹰斧（14140000）
        301414900,301414901,301414902,301414915,301414916,301414930,301414931,
    ];

    // 原版双元素武器（夜与火之剑 2140000、蕾拉娜的双剑 67520000，原版同时拥有魔+火，雷/圣为 0）：
    // 魔法招式保持火=0、只补雷/圣；火焰招式保持魔=0、只补雷/圣，避免原版双倍叠加。
    static readonly int[] SpreadMagicToThunDark=[
        300214906,300214907,300214908,300214909,
        700000150,700000151,700000152,700000153,700000154,
        700000155,700000156,700000157,700000158,700000159,
        700000160,700000161,700000162,700000163,700000164,
    ];
    static readonly int[] SpreadFireToThunDark=[
        300214925,300214926,
        700000170,700000171,700000172,700000173,
        700000175,700000176,700000177,
        700000180,700000181,700000182,
    ];

    static readonly string[] ElemCors=["atkMagCorrection","atkFireCorrection","atkThunCorrection","atkDarkCorrection"];

    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        var behFile=File_("BehaviorParam_PC");var atkFile=File_("AtkParam_Pc");
        var behBefore=behFile.Bytes.ToArray();var atkBefore=atkFile.Bytes.ToArray();
        var behDef=PARAMDEF.XmlDeserialize(Path.Combine(defs,"BehaviorParam.xml"));
        var atkDef=PARAMDEF.XmlDeserialize(Path.Combine(defs,"AtkParam.xml"));
        var beh=PARAM.Read(behBefore);beh.ApplyParamdef(behDef);
        var atk=PARAM.Read(atkBefore);atk.ApplyParamdef(atkDef);

        var addedBehIds=new HashSet<int>();
        var addedAtkIds=new HashSet<int>();
        var touchedAtkIds=new HashSet<int>();

        // 1. 补齐战灰「罗蕾塔的斩击」通用 variationId=0 兜底（300000441..300000444）
        for(int judge=441;judge<=444;judge++) {
            int srcId=301000000+judge,dstId=300000000+judge;
            string name=$"Tower Loretta Slash fallback {judge}";
            var oldBeh=beh.Rows.SingleOrDefault(r=>r.ID==dstId);
            if(oldBeh!=null&&oldBeh.Name!=name)throw new Exception($"ID collision: BehaviorParam_PC/{dstId}");
            var newBeh=new PARAM.Row(beh.Rows.Single(r=>r.ID==srcId)){ID=dstId,Name=name};
            Set(newBeh,"variationId",0);Set(newBeh,"behaviorJudgeId",judge);Set(newBeh,"refType",0);Set(newBeh,"refId",dstId);
            beh.Rows.RemoveAll(r=>r.ID==dstId);beh.Rows.Add(newBeh);addedBehIds.Add(dstId);

            var oldAtk=atk.Rows.SingleOrDefault(r=>r.ID==dstId);
            if(oldAtk!=null&&oldAtk.Name!=name)throw new Exception($"ID collision: AtkParam_Pc/{dstId}");
            var newAtk=new PARAM.Row(atk.Rows.Single(r=>r.ID==srcId)){ID=dstId,Name=name};
            if(Convert.ToSingle(newAtk["hit0_Radius"].Value)<=0||I(newAtk,"hit0_DmyPoly1")!=100||I(newAtk,"hit0_DmyPoly2")!=110)
                throw new Exception($"Unexpected Loretta template hitbox on AtkParam_Pc/{srcId}");
            atk.Rows.RemoveAll(r=>r.ID==dstId);atk.Rows.Add(newAtk);addedAtkIds.Add(dstId);
        }

        // 2. 修复高级词条换属性导致 0 伤害的战技 AtkParam_Pc 倍率
        var atkMap=atk.Rows.ToDictionary(r=>r.ID);
        foreach(int id in SpreadAllElements) {
            var r=atkMap[id];
            int val=ElemCors.Select(k=>I(r,k)).Where(v=>v>0).Distinct().Single();
            foreach(var k in ElemCors)Set(r,k,val);
            touchedAtkIds.Add(id);
        }
        foreach(int id in SpreadFromPhysics) {
            var r=atkMap[id];
            int val=I(r,"atkPhysCorrection");
            if(val<=0)throw new Exception($"Expected positive atkPhysCorrection on {id}");
            foreach(var k in ElemCors)Set(r,k,val);
            touchedAtkIds.Add(id);
        }
        foreach(int id in SpreadMagicToThunDark) {
            var r=atkMap[id];
            int val=I(r,"atkMagCorrection");
            if(val<=0||I(r,"atkFireCorrection")!=0)throw new Exception($"Unexpected magic skill row {id}");
            Set(r,"atkThunCorrection",val);Set(r,"atkDarkCorrection",val);
            touchedAtkIds.Add(id);
        }
        foreach(int id in SpreadFireToThunDark) {
            var r=atkMap[id];
            int val=I(r,"atkFireCorrection");
            if(val<=0||I(r,"atkMagCorrection")!=0)throw new Exception($"Unexpected fire skill row {id}");
            Set(r,"atkThunCorrection",val);Set(r,"atkDarkCorrection",val);
            touchedAtkIds.Add(id);
        }

        beh.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        atk.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        behFile.Bytes=beh.Write();
        atkFile.Bytes=atk.Write();

        // 严格回归校验：未列入白名单的行与字段 100% 保持原样
        void VerifyBeh() {
            var orig=PARAM.Read(behBefore);orig.ApplyParamdef(behDef);
            var now=beh.Rows.Where(r=>!addedBehIds.Contains(r.ID)).ToDictionary(r=>r.ID);
            if(now.Count!=orig.Rows.Count(r=>!addedBehIds.Contains(r.ID)))throw new Exception("BehaviorParam_PC unrelated row count changed");
            foreach(var r in orig.Rows.Where(r=>!addedBehIds.Contains(r.ID))) {
                var cur=now[r.ID];
                for(int i=0;i<r.Cells.Count;i++)
                    if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(r.Cells[i].Value,cur.Cells[i].Value))
                        throw new Exception($"BehaviorParam_PC unrelated cell changed: {r.ID}/{r.Cells[i].Def.InternalName}");
            }
        }
        void VerifyAtk() {
            var orig=PARAM.Read(atkBefore);orig.ApplyParamdef(atkDef);
            var now=atk.Rows.Where(r=>!addedAtkIds.Contains(r.ID)).ToDictionary(r=>r.ID);
            if(now.Count!=orig.Rows.Count(r=>!addedAtkIds.Contains(r.ID)))throw new Exception("AtkParam_Pc unrelated row count changed");
            foreach(var r in orig.Rows.Where(r=>!addedAtkIds.Contains(r.ID))) {
                var cur=now[r.ID];
                for(int i=0;i<r.Cells.Count;i++) {
                    string col=r.Cells[i].Def.InternalName;
                    bool allowed=touchedAtkIds.Contains(r.ID)&&ElemCors.Contains(col);
                    if(!allowed&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(r.Cells[i].Value,cur.Cells[i].Value))
                        throw new Exception($"AtkParam_Pc unrelated cell changed: {r.ID}/{col}");
                }
            }
        }
        VerifyBeh();VerifyAtk();

        Directory.CreateDirectory(output);
        string dest=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(dest,reg);
        if(new FileInfo(dest).Length>=4799*1024)throw new Exception("Regulation size limit exceeded");
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(dest);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine($"SkillDamageFix: added {addedBehIds.Count} Loretta Slash fallback rows (300000441..444) and updated {touchedAtkIds.Count} AtkParam_Pc rows; size={new FileInfo(dest).Length} bytes.");
    }
}
