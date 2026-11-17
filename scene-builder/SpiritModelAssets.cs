using SoulsFormats;

static class SpiritModelAssets {
    public static void Patch(string input,string output,bool verify=false) {
        int checkedMaps=0,changed=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")) {
            var map=MSBE.Read(path);
            if(!map.Parts.Enemies.Any(p=>p.Name.StartsWith("tower_")))continue;
            checkedMaps++;
            var model=map.Models.Enemies.SingleOrDefault(m=>m.Name=="c4420");
            if(model!=null)continue;
            if(verify)throw new Exception($"Lobster summon model missing: {Path.GetFileName(path)}");
            var before=map.Write();
            map.Models.Enemies.Add(new MSBE.Model.Enemy{Name="c4420",SibPath=@"N:\GR\data\Model\chr\c4420\sib\c4420.sib"});
            var dest=Path.Combine(output,"map/mapstudio",Path.GetFileName(path));Directory.CreateDirectory(Path.GetDirectoryName(dest)!);map.Write(dest);
            var check=MSBE.Read(dest);
            var added=check.Models.Enemies.Single(m=>m.Name=="c4420");
            if(!added.SibPath.EndsWith(@"\c4420.sib"))throw new Exception("Lobster resource path mismatch");
            check.Models.Enemies.Remove(added);
            if(!check.Write().SequenceEqual(before))throw new Exception("Unrelated map contents changed");
            changed++;
        }
        if(checkedMaps==0)throw new Exception("No tower maps inspected");
        var manifest=File.ReadAllLines(Path.Combine(input,"arenas-resources.txt"));
        for(int i=0;i<manifest.Length;i++) {
            var fields=manifest[i].Split('|');if(fields.Length!=3)continue;
            var staged=Path.Combine(output,fields[0]);
            var bytes=File.ReadAllBytes(!verify&&File.Exists(staged)?staged:Path.Combine(input,fields[0]));
            ulong hash=14695981039346656037;foreach(byte b in bytes)hash=unchecked((hash^b)*1099511628211);
            string expected=$"{fields[0]}|{bytes.Length}|{hash:x16}";
            if(verify&&manifest[i]!=expected)throw new Exception($"Arena resource checksum mismatch: {fields[0]}");
            manifest[i]=expected;
        }
        if(!verify){Directory.CreateDirectory(output);File.WriteAllLines(Path.Combine(output,"arenas-resources.txt"),manifest);}
        Console.WriteLine($"PASS: lobster model coverage checked across {checkedMaps} tower maps; {changed} repaired; no actors or events added");
    }
}
