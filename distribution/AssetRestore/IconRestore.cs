using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;

namespace Tower.Assets
{
    internal static class IconRestore
    {
        public static List<IconRecipe> FindPending(string package, AssetManifest manifest, bool validation, Action<string> noLinks, RestoreState state, bool reportProgress)
        {
            var pending = new List<IconRecipe>();
            if (manifest.Icons == null || manifest.Icons.Length == 0) return pending;
            if (manifest.Icons.Length > 10000 || manifest.Icons.Any(i => i.Id < 0 || i.Id >= 990000) ||
                manifest.Icons.Select(i => i.Id).Distinct().Count() != manifest.Icons.Length) throw new InvalidDataException("Invalid icon manifest");
            int checkedIcons = 0;
            if (reportProgress) Progress.Report("checking", 0, manifest.Icons.Length, "正在校验物品图标");
            string folder = Path.Combine(package, "scene-assets", "reward-icons");
            foreach (var icon in manifest.Icons)
            {
                string path = Path.Combine(folder, icon.Id + ".rgba"); noLinks(path);
                if (!state.IsIconUpToDate(path, icon, ignoreCache: validation)) pending.Add(icon);
                checkedIcons++;
                if (reportProgress && (checkedIcons % 50 == 0 || checkedIcons == manifest.Icons.Length))
                    Progress.Report("checking", checkedIcons, manifest.Icons.Length, "正在校验物品图标");
            }
            return pending;
        }

        public static void Restore(string package, string game, AssetManifest manifest, bool validation, Action<string> noLinks, RestoreState state, byte[] preloadedAtlas = null)
        {
            if (manifest.Icons == null || manifest.Icons.Length == 0) return;
            var pending = FindPending(package, manifest, validation, noLinks, state, reportProgress: true);
            if (pending.Count == 0) { Console.WriteLine("物品图标校验通过。"); return; }
            if (!validation && Process.GetProcessesByName("eldenring").Length != 0) throw new IOException("请先保存退出游戏，再生成物品图标。");
            Console.WriteLine("正在从本地资源生成 " + pending.Count + " 张物品图标…");
            byte[] atlas = preloadedAtlas;
            if (atlas == null)
            {
                OodleDcx.Load(game, manifest.OodleHash);
                string atlasPath = Path.Combine(package, "scene-assets", "menu", "hi", "01_common.tpf.dcx");
                var menuRecipe = manifest.Files.Single(f => f.Path == "menu/hi/01_common.tpf.dcx");
                atlas = OodleDcx.Decode(File.ReadAllBytes(atlasPath));
                if (AssetHash.Of(atlas) != menuRecipe.DecodedHash) throw new InvalidDataException("Icon source archive hash mismatch");
            }
            string folder = Path.Combine(package, "scene-assets", "reward-icons");
            Directory.CreateDirectory(folder);
            int generated = 0;
            int total = pending.Count;
            var progressGate = new object();
            Progress.Report("icons", 0, total, "正在生成物品图标", "首次生成后会保存在本地");
            var options = new ParallelOptions { MaxDegreeOfParallelism = Math.Max(1, Math.Min(4, Environment.ProcessorCount)) };
            try
            {
                Parallel.ForEach(pending, options, icon =>
                {
                    var bytes = IconCodec.Decode(atlas, icon.Offset, icon.Stride);
                    if (AssetHash.Of(bytes) != icon.Sha256) throw new InvalidDataException("Icon pixels differ: " + icon.Id);
                    string path = Path.Combine(folder, icon.Id + ".rgba"), temporary = path + ".tower-part";
                    noLinks(temporary);
                    try
                    {
                        File.WriteAllBytes(temporary, bytes);
                        if (File.Exists(path)) File.Replace(temporary, path, null); else File.Move(temporary, path);
                    }
                    finally { if (File.Exists(temporary)) File.Delete(temporary); }
                    state.RecordIcon(icon.Id, new FileInfo(path), icon.Sha256);
                    lock (progressGate)
                    {
                        generated++;
                        if (generated % 25 == 0 || generated == total)
                            Progress.Report("icons", generated, total, "正在生成物品图标", "已完成像素校验并保存");
                    }
                });
            }
            catch (AggregateException error)
            {
                throw error.Flatten().InnerExceptions[0];
            }
            finally
            {
                state.Save(noLinks);
            }
            Console.WriteLine("物品图标生成完成，像素校验通过。");
        }
    }
}
