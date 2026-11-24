using SoulsFormats;
using System.Xml.Linq;
using System.Text;

static class EmberDetailIcons {
    static ulong PathHash(string path) {
        ulong hash=0;
        foreach(char c in path) hash=unchecked(hash*0x85+c);
        return hash;
    }

    public static void Build(string game,string assets,string output,bool recipes) {
        foreach(string quality in new[]{"hi","low"}) BuildQuality(game,assets,output,quality,recipes);
    }

    static void BuildQuality(string game,string assets,string output,string quality,bool recipes) {
        string folder="menu/"+quality;
        string stem=folder+"/00_solo.tpfb";
        var sources=new Dictionary<string,(string archive,long offset,int size)>();
        byte[] ReadBase(string suffix) {
            ulong hash=PathHash("/"+stem+suffix);
            foreach(string archive in new[]{"DLC","Data0","Data1","Data2","Data3"}) {
                using var header=UXM.CryptographyUtility.DecryptRsa(Path.Combine(game,archive+".bhd"),UXM.ArchiveKeys.EldenRingKeys[archive]);
                var entry=BHD5.Read(header,BHD5.Game.EldenRing).Buckets.SelectMany(b=>b).FirstOrDefault(e=>e.FileNameHash==hash);
                if(entry==null)continue;
                sources.Add(suffix,(archive+".bdt",entry.FileOffset,entry.PaddedFileSize));
                using var data=File.OpenRead(Path.Combine(game,archive+".bdt"));
                return entry.ReadFile(data);
            }
            throw new FileNotFoundException(stem+suffix);
        }
        var binder=BXF4.Read(ReadBase("hd"),ReadBase("dt"));
        const string atlas="SB_Icon_TowerEmbers";
        const string artFolder="menu/hi";
        var layouts=BND4.Read(Path.Combine(assets,artFolder,"01_common.sblytbnd.dcx"));
        var layout=layouts.Files.Single(f=>f.Name.EndsWith(atlas+".layout"));
        var icons=XElement.Parse(Encoding.UTF8.GetString(layout.Bytes)).Elements().ToArray();
        var sheet=TPF.Read(Path.Combine(assets,artFolder,"01_common.tpf.dcx")).Textures.Single(t=>t.Name==atlas);
        var original=binder.Files.ToArray();
        var template=original.First(f=>f.Name.EndsWith("MENU_Knowledge_00000.tpf.dcx"));
        int nextId=original.Max(f=>f.ID);
        var added=new Dictionary<string,byte[]>();
        foreach(var icon in icons) {
            string small=Path.GetFileNameWithoutExtension((string)icon.Attribute("name"));
            if(!small.StartsWith("MENU_ItemIcon_")||!int.TryParse(small[14..],out int id))throw new InvalidDataException(small);
            string name=$"MENU_Knowledge_{id:D5}",path=$@"00_Solo\{name}.tpf.dcx";
            if(original.Any(f=>string.Equals(f.Name,path,StringComparison.OrdinalIgnoreCase)))throw new InvalidDataException("Icon collision: "+path);
            int x=(int)icon.Attribute("x"),y=(int)icon.Attribute("y"),w=(int)icon.Attribute("width"),h=(int)icon.Attribute("height");
            byte[] source=sheet.Bytes;
            int stride=BitConverter.ToInt32(source,20);
            if(sheet.Format!=9||BitConverter.ToInt32(source,88)!=32||w!=160||h!=160)throw new InvalidDataException("Unexpected ember atlas format");
            byte[] dds=new byte[128+w*h*4];
            Array.Copy(source,dds,128);
            BitConverter.GetBytes(h).CopyTo(dds,12);
            BitConverter.GetBytes(w).CopyTo(dds,16);
            BitConverter.GetBytes(w*4).CopyTo(dds,20);
            for(int row=0;row<h;row++)Array.Copy(source,128+(y+row)*stride+x*4,dds,128+row*w*4,w*4);
            var tpf=TPF.Read(template.Bytes);
            tpf.Textures.Clear();
            tpf.Textures.Add(new TPF.Texture(name,sheet.Format,0,dds,TPF.TPFPlatform.PC){Mipmaps=1});
            binder.Files.Add(new BinderFile(Binder.FileFlags.Flag1,++nextId,path,tpf.Write()));
            added.Add(path,dds);
        }
        Directory.CreateDirectory(Path.Combine(output,folder));
        string target=Path.Combine(output,stem);
        binder.Write(target+"hd",target+"dt");
        using var check=new BXF4Reader(target+"hd",target+"dt");
        if(check.Files.Count!=original.Length+added.Count)throw new InvalidDataException("Incorrect detail icon count");
        for(int i=0;i<original.Length;i++)
            if(check.Files[i].Name!=original[i].Name||!check.ReadFile(i).AsSpan().SequenceEqual(original[i].Bytes))throw new InvalidDataException("Original detail icon changed");
        foreach(var entry in check.Files.Skip(original.Length)){
            var texture=TPF.Read(check.ReadFile(entry)).Textures.Single();
            if(!texture.Bytes.AsSpan().SequenceEqual(added[entry.Name]))throw new InvalidDataException(entry.Name);
        }
        Console.WriteLine($"{quality}: verified {added.Count} ember images and {original.Length} original entries");
        if(recipes)foreach(var (suffix,source) in sources) {
            using var targetStream=File.OpenRead(target+suffix);
            using var raw=File.OpenRead(Path.Combine(game,source.archive));
            raw.Position=source.offset;
            using var patch=new BinaryWriter(File.Create(Path.Combine(output,$"ember-{quality}-{suffix}.patch")));
            patch.Write("TowerDetail1");
            patch.Write(source.archive);
            patch.Write(source.offset);
            patch.Write((int)Math.Min(source.size,targetStream.Length));
            patch.Write(targetStream.Length);
            patch.Write(Tower.Assets.AssetHash.FileHash(target+suffix));
            byte[] block=new byte[4096],basis=new byte[4096];
            for(long pos=0;pos<targetStream.Length;pos+=block.Length) {
                int size=(int)Math.Min(block.Length,targetStream.Length-pos);
                targetStream.ReadExactly(block,0,size);
                int shared=(int)Math.Clamp(source.size-pos,0,size);
                raw.ReadExactly(basis,0,shared);
                if(shared==size && block.AsSpan(0,size).SequenceEqual(basis.AsSpan(0,size)))continue;
                patch.Write((int)pos);patch.Write(size);patch.Write(block,0,size);
            }
            if(patch.BaseStream.Length>16*1024*1024)throw new InvalidDataException("Oversized detail icon delta");
        }
    }
}
