using SoulsFormats;

static class HoarahSfxAssets {
    static string Name(BinderFile f)=>Path.GetFileName(f.Name.Replace('\\','/'));
    static int Merge(BND4 target,BND4 shared) {
        var names=target.Files.ToDictionary(Name);var missing=new List<BinderFile>();
        foreach(var file in shared.Files) {
            if(names.TryGetValue(Name(file),out var existing)) {
                if(!existing.Bytes.SequenceEqual(file.Bytes))throw new Exception("Shared effect conflict: "+Name(file));
            }else missing.Add(file);
        }
        var ids=target.Files.Select(f=>f.ID).ToHashSet();int shift=0;
        while(missing.Any(f=>ids.Contains(checked(f.ID+shift))))shift++;
        foreach(var file in missing)target.Files.Add(new BinderFile(file.Flags,checked(file.ID+shift),file.Name,file.Bytes.ToArray()){CompressionInfo=file.CompressionInfo});
        target.Files.Sort((a,b)=>a.ID.CompareTo(b.ID));return missing.Count;
    }
    public static void Test() {
        var target=new BND4{Files=[new(Binder.FileFlags.Flag1,0,"x\\existing.fxr",[1]),new(Binder.FileFlags.Flag1,1,"x\\other.fxr",[2])]};
        var shared=new BND4{Files=[new(Binder.FileFlags.Flag1,0,"x\\existing.fxr",[1]),new(Binder.FileFlags.Flag1,1,"x\\new.fxr",[3]),new(Binder.FileFlags.Flag1,400001,"x\\new.ffxreslist",[4])]};
        if(Merge(target,shared)!=2||target.Files.Select(f=>f.ID).Distinct().Count()!=4)throw new Exception("SFX merge collision");
        if(Merge(target,shared)!=0)throw new Exception("SFX merge is not idempotent");
        shared.Files[0].Bytes=[9];bool rejected=false;
        try{Merge(target,shared);}catch(Exception){rejected=true;}
        if(!rejected)throw new Exception("Conflicting original effect accepted");
        Console.WriteLine("Shared SFX merge: ID collision, idempotence and conflict rejection passed");
    }
    public static void Patch(string game,string input,string output,string cache) {
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var shared=BND4.Read(Path.Combine(cache,"sfxbnd_c4720.ffxbnd.dcx"));
        const string relative="sfx/sfxbnd_commoneffects.ffxbnd.dcx";
        var target=BND4.Read(Path.Combine(input,relative));
        var original=target.Files.ToArray();int count=Merge(target,shared);
        var names=target.Files.Select(Name).ToHashSet();int dependencies=0;
        foreach(var list in shared.Files.Where(f=>f.Name.EndsWith(".ffxreslist"))) {
            foreach(var resource in System.Text.Encoding.UTF8.GetString(list.Bytes).Split(new[]{'\r','\n'},StringSplitOptions.RemoveEmptyEntries)) {
                var name=resource.Trim().Replace(".tif",".tpf").Replace(".sib",".flver");
                if(!names.Contains(name))throw new Exception("Missing shared effect resource: "+name);
                dependencies++;
            }
        }
        string path=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(path)!);target.Write(path);
        var actual=BND4.Read(path);var byId=actual.Files.ToDictionary(f=>f.ID);var byName=actual.Files.ToDictionary(Name);
        foreach(var file in original) {
            var check=byId[file.ID];
            if(check.Name!=file.Name||check.Flags!=file.Flags||!check.Bytes.SequenceEqual(file.Bytes))throw new Exception("Unrelated common effect changed: "+file.Name);
        }
        foreach(var file in shared.Files)if(!byName[Name(file)].Bytes.SequenceEqual(file.Bytes))throw new Exception("Shared effect changed: "+file.Name);
        if(actual.Files.Count!=original.Length+count)throw new Exception("Unexpected SFX file count");
        Console.WriteLine($"Added {count} shared resources; preserved {original.Length} existing entries; verified {dependencies} dependency references and all shared bytes");
    }
    public static void Inspect(string game,string output) {
        Directory.CreateDirectory(output);
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var wanted=new[]{"/sfx/sfxbnd_c4720.ffxbnd.dcx","/chr/c4721.anibnd.dcx","/chr/c4720.anibnd.dcx"};
        ulong Hash(string path){ulong h=0;foreach(var ch in path)h=unchecked(h*0x85+ch);return h;}
        foreach(var archive in new[]{"DLC","Data0","Data1","Data2","Data3"}) {
            var missing=wanted.Where(p=>!File.Exists(Path.Combine(output,Path.GetFileName(p)))).ToArray();if(missing.Length==0)break;
            using var header=UXM.CryptographyUtility.DecryptRsa(Path.Combine(game,archive+".bhd"),UXM.ArchiveKeys.EldenRingKeys[archive]);
            var files=BHD5.Read(header,BHD5.Game.EldenRing).Buckets.SelectMany(b=>b).ToDictionary(f=>f.FileNameHash);
            using var data=File.OpenRead(Path.Combine(game,archive+".bdt"));
            foreach(var path in missing)if(files.TryGetValue(Hash(path),out var f))File.WriteAllBytes(Path.Combine(output,Path.GetFileName(path)),f.ReadFile(data));
        }
        var sfx=BND4.Read(Path.Combine(output,"sfxbnd_c4720.ffxbnd.dcx"));
        var effects=sfx.Files.Where(f=>f.Name.EndsWith(".fxr")).Select(f=>BitConverter.ToInt32(f.Bytes,12)).ToHashSet();
        File.WriteAllLines(Path.Combine(output,"effects.txt"),sfx.Files.Select(f=>$"{f.ID}|{f.Name}|{f.Bytes.Length}"));
        foreach(var model in new[]{"c4720","c4721"}) {
            var binder=BND4.Read(Path.Combine(output,model+".anibnd.dcx"));
            var lines=new List<string>();
            foreach(var file in binder.Files.Where(f=>f.Name.EndsWith(".tae"))) {
                var tae=TAE.Read(file.Bytes);
                foreach(var animation in tae.Animations)foreach(var ev in animation.Events) {
                    var bytes=ev.GetParameterBytes(tae.BigEndian);
                    for(int i=0;i+4<=bytes.Length;i+=4) {
                        var id=BitConverter.ToInt32(bytes,i);
                        if(effects.Contains(id))lines.Add($"{file.Name}|anim={animation.ID}|type={ev.Type}|offset={i}|effect={id}");
                    }
                }
            }
            File.WriteAllLines(Path.Combine(output,model+"-effects.txt"),lines);
            Console.WriteLine($"{model}: {lines.Count} animation references to shared effects");
        }
        Console.WriteLine($"Shared package: {effects.Count} FXRs, {sfx.Files.Count} total resources");
    }
}
