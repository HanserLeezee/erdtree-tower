using SoulsFormats;

static class EmberSpellDamage {
    internal static readonly HashSet<int> Ids = new[]{15,19,20,21,60,61,70,71}
        .Concat(Enumerable.Range(31,8)).Concat(Enumerable.Range(100,32))
        .Select(id=>EmberAssets.First+id).ToHashSet();
    static readonly string[] Fields=["magParamChange","miracleParamChange"];

    internal static void Apply(IEnumerable<PARAM.Row> rows) {
        var selected=rows.Where(r=>Ids.Contains(r.ID)).ToArray();
        if(selected.Length!=Ids.Count)throw new Exception("Missing general ember damage rows");
        foreach(var row in selected) {
            if(Convert.ToInt32(row["stateInfo"].Value)!=0||Convert.ToInt32(row["wepParamChange"].Value)!=0)
                throw new Exception($"Unexpected restricted damage row: {row.ID}");
            foreach(var field in Fields)row[field].Value=Convert.ChangeType(1,row[field].Value.GetType());
        }
    }
    static void Verify(PARAM table) {
        foreach(var id in Ids)foreach(var field in Fields)
            if(Convert.ToInt32(table.Rows.Single(r=>r.ID==id)[field].Value)!=1)
                throw new Exception($"Ember {id} excludes spell damage: {field}");
    }
    internal static void Patch(string input,string output,bool verifyOnly=false) {
        var binder=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=binder.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs","SpEffect.xml"));
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        if(verifyOnly){Verify(original);Console.WriteLine($"Verified {Ids.Count} spell-compatible damage effects");return;}
        var table=PARAM.Read(file.Bytes);table.ApplyParamdef(def);Apply(table.Rows);Verify(table);
        var others=binder.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        file.Bytes=table.Write();Directory.CreateDirectory(output);
        var destination=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(destination,binder);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(destination);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))
            if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated table changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);Verify(actual);
        if(actual.Rows.Count!=original.Rows.Count)throw new Exception("Effect row count changed");
        var rows=actual.Rows.ToDictionary(r=>r.ID);var changed=0;
        foreach(var row in original.Rows)foreach(var cell in row.Cells) {
            var key=cell.Def.InternalName;
            if(System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][key].Value))continue;
            if(!Ids.Contains(row.ID)||!Fields.Contains(key))throw new Exception($"Unrelated field changed: {row.ID} {key}");
            changed++;
        }
        Console.WriteLine($"Verified {Ids.Count} effects; {changed} spell flags changed; all other fields and tables preserved");
    }
}
