using SoulsFormats;

/// 无缝联机 Boss 多人缩放修复：
/// 1) 原版 MultiPlayCorrectionParam 的 Boss 行（300/400/203000..498000/10000300/10000400）将 bOverrideSpEffect 设为 0 或 2，
///    引擎不会在角色启用或人数变动时自动调用 ApplyMultiPlayCorrection，必须等原版 EMEVD 雾门/血条事件（2003[11]）。
///    爬塔联机敌人由 PartyEnemyEvents 直接 SetCharacterEnable、血条由原生 DLL 写内存绑定，导致所有 Boss 与 Boss 衍生精英
///    在无缝联机下完全吃不到 ersc.dll 的 boss_health_scaling / boss_damage_scaling / boss_posture_scaling（始终 1.0× 单人血量）。
///    统一改为 bOverrideSpEffect = 1，使引擎与普通怪一样在激活/人数同步时触发 ersc.dll 的 Boss 缩放（2147202000+ / 2147203000+）。
/// 2) 修正「癫火之王」米德拉（950000304/950000305）的 multiPlayCorrectionParamId：10000200 -> 10000300（与其余 DLC Boss 一致）。
static class BossCoopScalingAssets {
    static readonly int[] MidraNpcs = { 950000304, 950000305 };
    const int MidraBossMpc = 10000300;

    static int I(PARAM.Row row, string field) => Convert.ToInt32(row[field].Value);
    static void Set(PARAM.Row row, string field, int value) =>
        row[field].Value = Convert.ChangeType(value, row[field].Value.GetType());

    public static void Patch(string input, string output, bool verify = false) {
        if (!verify && Path.GetFullPath(input) == Path.GetFullPath(output))
            throw new Exception("Separate staging output required");
        var reg = SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input, "regulation.bin"));
        var original = reg.Files.ToDictionary(f => f.Name, f => f.Bytes.ToArray());
        var defs = Environment.GetEnvironmentVariable("TOWER_PARAMDEFS") ?? "work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name) => reg.Files.Single(f => Path.GetFileNameWithoutExtension(f.Name.Replace('\\', '/')) == name);
        PARAM Read(string name) {
            var p = PARAM.Read(File_(name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs, name + ".xml")));
            return p;
        }
        var mpc = Read("MultiPlayCorrectionParam");
        var npc = Read("NpcParam");
        int patchedMpc = 0;
        foreach (var row in mpc.Rows) {
            int cur = I(row, "bOverrideSpEffect");
            if (!verify && cur != 1) {
                Set(row, "bOverrideSpEffect", 1);
                patchedMpc++;
            }
            if (I(row, "bOverrideSpEffect") != 1)
                throw new Exception($"MultiPlayCorrectionParam[{row.ID}].bOverrideSpEffect is {I(row, "bOverrideSpEffect")}, expected 1");
        }
        int patchedNpc = 0;
        foreach (int id in MidraNpcs) {
            var row = npc.Rows.Single(r => r.ID == id);
            if (!verify && I(row, "multiPlayCorrectionParamId") != MidraBossMpc) {
                Set(row, "multiPlayCorrectionParamId", MidraBossMpc);
                patchedNpc++;
            }
            if (I(row, "multiPlayCorrectionParamId") != MidraBossMpc)
                throw new Exception($"NpcParam[{id}].multiPlayCorrectionParamId is {I(row, "multiPlayCorrectionParamId")}, expected {MidraBossMpc}");
        }
        if (verify) {
            Console.WriteLine($"PASS: all {mpc.Rows.Count} MultiPlayCorrectionParam rows have bOverrideSpEffect=1; Midra {string.Join("/", MidraNpcs)} use mpc={MidraBossMpc}");
            return;
        }
        foreach (var (name, table) in new[] { ("MultiPlayCorrectionParam", mpc), ("NpcParam", npc) }) {
            var before = PARAM.Read(original[File_(name).Name]);
            before.ApplyParamdef(table.AppliedParamdef);
            var rows = table.Rows.ToDictionary(r => r.ID);
            if (before.Rows.Count != table.Rows.Count) throw new Exception($"Row count changed in {name}");
            foreach (var row in before.Rows) {
                var afterRow = rows[row.ID];
                for (int ci = 0; ci < row.Cells.Count; ci++) {
                    var field = row.Cells[ci].Def.InternalName;
                    var expected = row.Cells[ci].Value;
                    if (name == "MultiPlayCorrectionParam" && field == "bOverrideSpEffect")
                        expected = Convert.ChangeType(1, expected.GetType());
                    if (name == "NpcParam" && MidraNpcs.Contains(row.ID) && field == "multiPlayCorrectionParamId")
                        expected = Convert.ChangeType(MidraBossMpc, expected.GetType());
                    if (!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(expected, afterRow.Cells[ci].Value))
                        throw new Exception($"Unrelated field changed: {name}/{row.ID}/{field}");
                }
            }
            table.Rows.Sort((a, b) => a.ID.CompareTo(b.ID));
            File_(name).Bytes = table.Write();
        }
        Directory.CreateDirectory(output);
        var path = Path.Combine(output, "regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path, reg);
        var check = SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach (var f in check.Files) {
            if (!f.Bytes.SequenceEqual(reg.Files.Single(x => x.Name == f.Name).Bytes))
                throw new Exception($"Regulation roundtrip mismatch: {f.Name}");
            var baseName = Path.GetFileNameWithoutExtension(f.Name.Replace('\\', '/'));
            if (baseName is not ("MultiPlayCorrectionParam" or "NpcParam") && !f.Bytes.SequenceEqual(original[f.Name]))
                throw new Exception($"Unrelated param changed: {f.Name}");
        }
        Console.WriteLine($"PASS: patched {patchedMpc}/{mpc.Rows.Count} MultiPlayCorrectionParam rows (bOverrideSpEffect=1) and {patchedNpc} Midra NpcParam rows (mpc={MidraBossMpc}); other params preserved");
    }
}
