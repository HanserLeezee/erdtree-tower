using SoulsFormats;
using SoulsFormats.Cryptography;

static class StarSurgeVisual {
    const int Id=9810999;
    const string Name="Tower Starsurge Haima impact";
    public static void Build(string input,string output){
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate output required");
        var reg=RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        foreach(var (table,def) in new[]{("SpEffectParam","SpEffect"),("SpEffectVfxParam","SpEffectVfx")}){
            var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==table);
            var param=PARAM.Read(file.Bytes);
            param.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs",def+".xml")));
            var old=param.Rows.SingleOrDefault(r=>r.ID==Id);
            if(old!=null && old.Name!=Name)throw new Exception($"ID collision in {table}");
            if(old!=null)param.Rows.Remove(old);
            var effect=table=="SpEffectParam";
            var row=effect?EmberAssets.Enemy(param,Id,Name,0.05f):new PARAM.Row(param.Rows.Single(r=>r.ID==1605000)){ID=Id,Name=Name};
            void Set(string key,int value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            if(effect)Set("vfxId",Id);
            else {
                foreach(var key in new[]{"midstSfxId","midstSeId","initSeId","finishSfxId","finishSeId","initDmyId"})Set(key,-1);
                Set("initSfxId",523798);Set("isInitFullbody",0);Set("playCategory",0);Set("playPriority",0);
            }
            param.Rows.Add(row);param.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=param.Write();
        }
        Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");RegulationDecryptor.EncryptERRegulation(path,reg);
        if(!RegulationDecryptor.DecryptERRegulation(path).Files.Zip(reg.Files).All(p=>p.First.Bytes.SequenceEqual(p.Second.Bytes)))throw new Exception("Round-trip mismatch");
    }
}
