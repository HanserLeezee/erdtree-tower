using SoulsFormats;

static class EnemyNameFix {
    public static void Build(string input,string output) {
        foreach(var source in Directory.EnumerateFiles(Path.Combine(input,"msg"),"*.msgbnd.dcx",SearchOption.AllDirectories)){
            var b=BND4.Read(source);int changed=0;
            foreach(var file in b.Files){
                var name=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];
                bool goods=name is "GoodsName" or "GoodsInfo" or "GoodsCaption";
                if(!goods&&name!="NpcName"&&file.ID!=MelinaAssets.PreviewMenu)continue;
                var text=FMG.Read(file.Bytes);bool edited=false;
                foreach(var entry in text.Entries){
                    if(entry.Text==null)continue;
                    if(goods&&entry.ID is not (>=309000 and <=309039 or >=316000 and <=316039))continue;
                    if(!goods&&(name=="NpcName"?entry.ID<9610000:entry.ID<MelinaAssets.PreviewFirst))continue;
                    var value=entry.Text.Replace("一只大型银色泪滴","一个铁球").Replace("一隻大型銀色淚滴","一個鐵球")
                        .Replace("大型银色泪滴","铁球").Replace("大型銀色淚滴","鐵球").Replace("a Large Silver Tear","an Iron Ball").Replace("Large Silver Tear","Iron Ball")
                        .Replace("死诞者","幽魂随从").Replace("死誕者","幽魂隨從");
                    if(value!=entry.Text){entry.Text=value;edited=true;changed++;}
                }
                if(edited)file.Bytes=text.Write();
            }
            if(changed==0)continue;
            string rel=Path.GetRelativePath(input,source),dest=Path.Combine(output,rel);
            Directory.CreateDirectory(Path.GetDirectoryName(dest)!);b.Write(dest);
            if(!BND4.Read(dest).Files.Zip(b.Files).All(pair=>pair.First.Bytes.SequenceEqual(pair.Second.Bytes)))throw new Exception(rel);
            Console.WriteLine($"{rel}: {changed} updated entries");
        }
    }
}
