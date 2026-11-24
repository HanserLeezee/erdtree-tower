using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.Serialization.Json;
using System.Text;

namespace Tower.Assets
{
    internal static class Program
    {
        const string MenuCommonPath = "menu/hi/01_common.tpf.dcx";
        const string EnglishMenuPath = "locales/en-US/menu/hi/01_common.tpf.dcx";
        static readonly string[] Targets = { MenuCommonPath, "sfx/sfxbnd_commoneffects.ffxbnd.dcx" };

        static int Main(string[] args)
        {
            Console.OutputEncoding = new UTF8Encoding(false);
            try
            {
                bool validation = args.Length == 3 && args[2] == "--build-validation";
                bool forceVerify = args.Length == 3 && args[2] == "--verify";
                if (args.Length != 2 && !validation && !forceVerify) throw new ArgumentException("Usage: Tower.AssetRestore <package> <game>");
                string package = Path.GetFullPath(args[0]);
                if (validation && (File.Exists(Path.Combine(package, "preview.me3")) || Directory.GetFiles(package, "*.dll").Length != 0))
                    throw new IOException("Build validation requires an isolated asset-only directory");
                Restore(package, Path.GetFullPath(args[1]), validation, forceVerify); return 0;
            }
            catch (Exception error) { Console.Error.WriteLine("资源准备失败：" + error.Message); return 1; }
        }

        static void NoLinks(string path)
        {
            for (var item = new DirectoryInfo(Path.GetDirectoryName(path)); item != null; item = item.Parent)
                if (item.Exists && (item.Attributes & FileAttributes.ReparsePoint) != 0) throw new IOException("Resource path contains a junction: " + item.FullName);
            if (File.Exists(path) && (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) throw new IOException("Resource path contains a link");
        }

        static string SourceKey(AssetRecipe file)
        {
            var sb = new StringBuilder();
            sb.Append(file.Archive).Append('|').Append(file.Offset).Append('|').Append(file.SourceSize).Append('|').Append(file.SourceHash);
            if (file.Repairs != null)
                foreach (var r in file.Repairs) sb.Append('|').Append(r.Offset).Append(':').Append(r.Data);
            return sb.ToString();
        }

        static int BuildOrder(AssetRecipe file)
        {
            // 先处理特效包，再处理英文贴图，最后处理主界面贴图（便于与英文贴图共享原始解包源，并把解码图集直接复用给 IconRestore）
            if (file.Path == MenuCommonPath) return 2;
            if (file.Path == EnglishMenuPath) return 1;
            return 0;
        }

        static void Restore(string package, string game, bool validation, bool forceVerify)
        {
            string folder = Path.Combine(package, "asset-patches"), manifestPath = Path.Combine(folder, "manifest.json");
            if (!File.Exists(manifestPath)) return;
            NoLinks(manifestPath);
            using (var guard = AssetLock.Acquire(Path.Combine(folder, ".restore.lock"), TimeSpan.FromMinutes(30),
                () => Progress.Report("checking", 0, 0, "正在等待已有资源生成任务", "完成后将重新校验资源，请勿重复启动")))
            {
                AssetManifest manifest;
                using (var stream = File.OpenRead(manifestPath)) manifest = (AssetManifest)new DataContractJsonSerializer(typeof(AssetManifest)).ReadObject(stream);
                var paths = manifest.Files == null ? new HashSet<string>() : new HashSet<string>(manifest.Files.Select(f => f.Path));
                var allowed = Targets.Concat(new[] { EnglishMenuPath });
                if (manifest.Version != 1 || manifest.Files == null || paths.Count != manifest.Files.Length ||
                    !paths.IsSupersetOf(Targets) || !paths.IsSubsetOf(allowed)) throw new InvalidDataException("Invalid asset manifest");

                var state = RestoreState.Load(Path.Combine(folder, ".restore-state.json"), NoLinks);
                bool needEnglish = validation || AssetLanguage.NeedsEnglish(package, game);
                var activeFiles = manifest.Files.Where(f => f.Path != EnglishMenuPath || needEnglish).ToList();

                var pending = new List<AssetRecipe>();
                int checkedFiles = 0;
                Progress.Report("checking", 0, activeFiles.Count, "正在校验本地资源");
                foreach (var file in activeFiles)
                {
                    Progress.Report("checking", checkedFiles, activeFiles.Count, "正在校验本地资源", Progress.Name(file.Path));
                    string target = Path.Combine(package, "scene-assets", file.Path); NoLinks(target);
                    if (validation || !state.IsFileUpToDate(target, file, ignoreCache: forceVerify)) pending.Add(file);
                    Progress.Report("checking", ++checkedFiles, activeFiles.Count, "正在校验本地资源", Progress.Name(file.Path));
                }
                if (pending.Count == 0)
                {
                    state.Save(NoLinks);
                    Console.WriteLine("本地资源校验通过。");
                    IconRestore.Restore(package, game, manifest, validation, NoLinks, state);
                    return;
                }
                if (!validation && Process.GetProcessesByName("eldenring").Length != 0) throw new IOException("请先保存退出游戏，再修复资源。");
                pending.Sort((a, b) => BuildOrder(a).CompareTo(BuildOrder(b)));

                OodleDcx.Load(game, manifest.OodleHash);
                var staged = new List<Tuple<string, string, AssetRecipe, string>>();
                byte[] preloadedAtlas = null;
                string cachedSourceKey = null;
                byte[] cachedSource = null;
                try
                {
                    for (int index = 0; index < pending.Count; index++)
                    {
                        var file = pending[index];
                        Console.WriteLine("正在本地生成 " + file.Path + "（快速模式）");
                        Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 读取游戏原始资源");
                        if (!new[] { "Data0.bdt", "Data1.bdt", "Data2.bdt", "Data3.bdt", "DLC.bdt" }.Contains(file.Archive) ||
                            file.SourceSize <= 0 || file.SourceSize > 512 * 1024 * 1024 || file.Offset < 0 ||
                            file.Patch != file.Path.Replace('/', '_') + ".patch") throw new InvalidDataException("Invalid source recipe");

                        string key = SourceKey(file);
                        byte[] source;
                        if (cachedSource != null && key == cachedSourceKey)
                        {
                            source = cachedSource;
                        }
                        else
                        {
                            cachedSource = null;
                            cachedSourceKey = null;
                            var raw = new byte[file.SourceSize];
                            using (var input = File.OpenRead(Path.Combine(game, file.Archive)))
                            {
                                if (file.Offset > input.Length - raw.Length) throw new InvalidDataException("游戏资源版本不匹配，请使用完整整合包。");
                                input.Position = file.Offset; int offset = 0;
                                while (offset < raw.Length) { int count = input.Read(raw, offset, raw.Length - offset); if (count == 0) throw new EndOfStreamException(); offset += count; }
                            }
                            if (AssetHash.Of(raw) != file.SourceHash) throw new InvalidDataException("游戏资源版本不匹配或已被修改，请使用完整整合包：" + file.Path);
                            foreach (var repair in file.Repairs)
                            {
                                var bytes = Convert.FromBase64String(repair.Data);
                                if (repair.Offset < 0 || repair.Offset > raw.Length - bytes.Length) throw new InvalidDataException("Invalid source repair");
                                Buffer.BlockCopy(bytes, 0, raw, repair.Offset, bytes.Length);
                            }
                            Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 解压原始资源");
                            source = OodleDcx.Decode(raw);
                            raw = null;
                        }

                        string patch = Path.Combine(folder, file.Patch); NoLinks(patch);
                        if (AssetHash.FileHash(patch) != file.PatchHash) throw new InvalidDataException("补丁损坏，请在启动器校验并修复文件。");
                        byte[] decoded;
                        Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 合并模组内容");
                        using (var stream = File.OpenRead(patch)) decoded = AssetPatch.Apply(source, stream, file.DecodedSize);
                        if (AssetHash.Of(decoded) != file.DecodedHash) throw new InvalidDataException("Asset patch verification failed");

                        bool keepSourceForNext = index + 1 < pending.Count && SourceKey(pending[index + 1]) == key;
                        if (keepSourceForNext)
                        {
                            cachedSource = source;
                            cachedSourceKey = key;
                        }
                        else
                        {
                            cachedSource = null;
                            cachedSourceKey = null;
                            source = null;
                        }

                        Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 快速压缩生成结果");
                        var result = OodleDcx.Encode(decoded, Convert.FromBase64String(file.Header), OodleDcx.FastCompressionLevel);
                        if (validation)
                        {
                            Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 校验解压一致性");
                            var roundTripped = OodleDcx.Decode(result);
                            if (AssetHash.Of(roundTripped) != file.DecodedHash) throw new InvalidDataException("快速压缩回读校验失败：" + file.Path);
                            roundTripped = null;
                        }
                        Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 校验并写入");
                        string resultHash = AssetHash.Of(result);
                        string target = Path.Combine(package, "scene-assets", file.Path), temporary = target + ".tower-part";
                        NoLinks(temporary); Directory.CreateDirectory(Path.GetDirectoryName(target));
                        File.WriteAllBytes(temporary, result);
                        result = null;
                        staged.Add(Tuple.Create(temporary, target, file, resultHash));

                        if (file.Path == MenuCommonPath && IconRestore.FindPending(package, manifest, validation, NoLinks, state, reportProgress: false).Count > 0)
                            preloadedAtlas = decoded;
                        else
                            decoded = null;

                        Progress.Report("building", staged.Count, pending.Count, "正在生成本地资源", Progress.Name(file.Path) + " · 已完成");
                        if (!keepSourceForNext && preloadedAtlas == null) GC.Collect();
                    }
                    cachedSource = null;
                    // 全部文件通过解码内容哈希校验后，再原子替换目标文件并落盘状态缓存
                    foreach (var item in staged)
                    {
                        if (File.Exists(item.Item2)) File.Replace(item.Item1, item.Item2, null);
                        else File.Move(item.Item1, item.Item2);
                        state.RecordFile(item.Item3.Path, new FileInfo(item.Item2), item.Item3.DecodedHash, item.Item4, OodleDcx.FastCompressionLevel);
                    }
                    state.Save(NoLinks);
                    Console.WriteLine("本地资源生成完成，校验通过。");
                    IconRestore.Restore(package, game, manifest, validation, NoLinks, state, preloadedAtlas);
                }
                finally { foreach (var item in staged) if (File.Exists(item.Item1)) File.Delete(item.Item1); }
            }
        }
    }
}
