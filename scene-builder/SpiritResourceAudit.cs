using SoulsFormats;
using System.Text;
using System.Text.RegularExpressions;
using System.Xml.Linq;

static class SpiritResourceAudit {
    public static void Verify(string input) {
        var bases=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="spirit").Select(p=>int.Parse(p[1])).Distinct().ToArray();
        var ids=bases.SelectMany(id=>Enumerable.Range(id,40)).ToArray();
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var goods=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="EquipParamGoods").Bytes);
        goods.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs","EquipParamGoods.xml")));
        var rows=goods.Rows.ToDictionary(r=>r.ID);
        foreach(int id in ids)if(!rows.ContainsKey(id))throw new Exception($"Missing spirit param {id}");
        foreach(string lang in new[]{"zhocn","zhotw","engus"}) {
            var bnd=BND4.Read(Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx"));
            foreach(string name in new[]{"GoodsName","GoodsInfo","GoodsCaption"}) {
                var texts=bnd.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/')).StartsWith(name)).SelectMany(f=>FMG.Read(f.Bytes).Entries).Where(e=>!string.IsNullOrWhiteSpace(e.Text)).Select(e=>e.ID).ToHashSet();
                var missing=ids.Where(id=>!texts.Contains(id)).ToArray();
                if(missing.Length>0)throw new Exception($"{lang}/{name}: missing {missing.Length}: {string.Join(',',missing.Take(20))}");
            }
            Console.WriteLine($"PASS {lang}: {bases.Length} spirits x40 levels x3 text tables");
        }
        var icons=ids.Select(id=>Convert.ToInt32(rows[id]["iconId"].Value)).Distinct().ToArray();
        var roots=new[]{input}.Concat(Directory.Exists(Path.Combine(input,"locales"))?Directory.GetDirectories(Path.Combine(input,"locales")):Array.Empty<string>());
        foreach(var root in roots)foreach(string quality in new[]{"hi","low"}) {
            var dir=Path.Combine(root,"menu",quality);if(!Directory.Exists(dir))continue;
            var tpf=TPF.Read(Path.Combine(dir,"01_common.tpf.dcx"));var bnd=BND4.Read(Path.Combine(dir,"01_common.sblytbnd.dcx"));
            var found=new HashSet<int>();
            foreach(var f in bnd.Files.Where(f=>f.Name.EndsWith(".layout"))) {
                var xml=XElement.Parse(Encoding.UTF8.GetString(f.Bytes));
                var name=Path.GetFileNameWithoutExtension((string)xml.Attribute("imagePath")??"");
                foreach(var e in xml.Elements()) {
                    var m=Regex.Match((string)e.Attribute("name")??"",@"MENU_ItemIcon_(\d+)\.png");
                    if(!m.Success||!icons.Contains(int.Parse(m.Groups[1].Value)))continue;
                    int id=int.Parse(m.Groups[1].Value);var tex=tpf.Textures.SingleOrDefault(t=>t.Name==name)??throw new Exception($"{dir}: missing texture {name}");
                    var d=tex.Bytes;int w=BitConverter.ToInt32(d,16),h=BitConverter.ToInt32(d,12),x=(int)e.Attribute("x"),y=(int)e.Attribute("y"),cw=(int)e.Attribute("width"),ch=(int)e.Attribute("height");
                    if(x<0||y<0||cw<=0||ch<=0||x+cw>w||y+ch>h)throw new Exception($"Icon {id} exceeds atlas bounds");
                    if(id>=65300&&id<65398) {
                        if(d.Length!=128+w*h*4)throw new Exception("Unexpected custom spirit texture format");
                        bool visible=false;for(int yy=y;yy<y+ch;yy++)for(int xx=x;xx<x+cw;xx++)visible|=d[128+(yy*w+xx)*4+3]!=0;
                        if(!visible)throw new Exception($"Spirit icon {id} is transparent");
                    }
                    if(!found.Add(id))throw new Exception($"Duplicate icon {id}");
                }
            }
            var missing=icons.Except(found).ToArray();if(missing.Length>0)throw new Exception($"{dir}: missing icons {string.Join(',',missing)}");
            Console.WriteLine($"PASS {Path.GetRelativePath(input,dir)}: {icons.Length} spirit icons, texture references and bounds verified");
        }
    }
}
