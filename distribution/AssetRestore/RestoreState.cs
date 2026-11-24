#nullable disable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Text.RegularExpressions;

namespace Tower.Assets
{
    [DataContract]
    public sealed class RestoredFileEntry
    {
        [DataMember] public string Path { get; set; }
        [DataMember] public long Size { get; set; }
        [DataMember] public long LastWriteTicks { get; set; }
        [DataMember] public string DecodedHash { get; set; }
        [DataMember] public string FileHash { get; set; }
        [DataMember] public int CompressionLevel { get; set; }
    }

    [DataContract]
    public sealed class RestoredIconEntry
    {
        [DataMember] public int Id { get; set; }
        [DataMember] public long Size { get; set; }
        [DataMember] public long LastWriteTicks { get; set; }
        [DataMember] public string Sha256 { get; set; }
    }

    [DataContract]
    public sealed class RestoreStateData
    {
        [DataMember] public int Version { get; set; }
        [DataMember] public RestoredFileEntry[] Files { get; set; }
        [DataMember] public RestoredIconEntry[] Icons { get; set; }
    }

    /// <summary>
    /// 本地资源校验状态缓存：记录已通过 DecodedHash / Sha256 验证的文件大小、修改时间与哈希。
    /// 正常启动时若大小、时间戳及清单预期内容哈希都未变，则直接放行，避免每次重扫几百 MB 与上千张小图标。
    /// </summary>
    public sealed class RestoreState
    {
        public const int CurrentVersion = 1;
        readonly string statePath;
        readonly Dictionary<string, RestoredFileEntry> files = new Dictionary<string, RestoredFileEntry>(StringComparer.OrdinalIgnoreCase);
        readonly Dictionary<int, RestoredIconEntry> icons = new Dictionary<int, RestoredIconEntry>();
        readonly object gate = new object();
        bool dirty;

        RestoreState(string statePath) { this.statePath = statePath; }

        public static RestoreState Load(string statePath, Action<string> noLinks = null)
        {
            var state = new RestoreState(statePath);
            if (!File.Exists(statePath)) return state;
            noLinks?.Invoke(statePath);
            try
            {
                using (var stream = File.OpenRead(statePath))
                {
                    var data = (RestoreStateData)new DataContractJsonSerializer(typeof(RestoreStateData)).ReadObject(stream);
                    if (data != null && data.Version == CurrentVersion)
                    {
                        if (data.Files != null)
                            foreach (var entry in data.Files)
                                if (entry != null && !string.IsNullOrEmpty(entry.Path))
                                    state.files[entry.Path] = entry;
                        if (data.Icons != null)
                            foreach (var entry in data.Icons)
                                if (entry != null)
                                    state.icons[entry.Id] = entry;
                    }
                }
            }
            catch (Exception)
            {
                // 状态缓存损坏时静默回退到完整校验，不影响功能。
            }
            return state;
        }

        public bool IsFileUpToDate(string targetPath, AssetRecipe recipe, bool ignoreCache)
        {
            var info = new FileInfo(targetPath);
            if (!info.Exists || info.Length == 0) return false;
            RestoredFileEntry cached;
            lock (gate) files.TryGetValue(recipe.Path, out cached);
            if (!ignoreCache && cached != null &&
                string.Equals(cached.DecodedHash, recipe.DecodedHash, StringComparison.OrdinalIgnoreCase) &&
                cached.Size == info.Length &&
                cached.LastWriteTicks == info.LastWriteTimeUtc.Ticks)
            {
                return true;
            }
            string actualHash = AssetHash.FileHash(targetPath);
            // 1) 兼容旧版 Level 6 或完整包中已存在的文件（精确匹配 TargetHash）
            if (string.Equals(actualHash, recipe.TargetHash, StringComparison.OrdinalIgnoreCase))
            {
                RecordFile(recipe.Path, info, recipe.DecodedHash, actualHash, 6);
                return true;
            }
            // 2) 已由本工具以快速压缩生成并记录过哈希的文件（例如文件被复制导致时间戳变化）
            if (cached != null &&
                string.Equals(cached.DecodedHash, recipe.DecodedHash, StringComparison.OrdinalIgnoreCase) &&
                string.Equals(actualHash, cached.FileHash, StringComparison.OrdinalIgnoreCase))
            {
                RecordFile(recipe.Path, info, recipe.DecodedHash, actualHash, cached.CompressionLevel);
                return true;
            }
            return false;
        }

        public bool IsIconUpToDate(string iconPath, IconRecipe recipe, bool ignoreCache)
        {
            var info = new FileInfo(iconPath);
            if (!info.Exists || info.Length != 102400) return false;
            RestoredIconEntry cached;
            lock (gate) icons.TryGetValue(recipe.Id, out cached);
            if (!ignoreCache && cached != null &&
                string.Equals(cached.Sha256, recipe.Sha256, StringComparison.OrdinalIgnoreCase) &&
                cached.Size == info.Length &&
                cached.LastWriteTicks == info.LastWriteTimeUtc.Ticks)
            {
                return true;
            }
            if (string.Equals(AssetHash.FileHash(iconPath), recipe.Sha256, StringComparison.OrdinalIgnoreCase))
            {
                RecordIcon(recipe.Id, info, recipe.Sha256);
                return true;
            }
            return false;
        }

        public void RecordFile(string relativePath, FileInfo info, string decodedHash, string fileHash, int compressionLevel)
        {
            lock (gate)
            {
                files[relativePath] = new RestoredFileEntry
                {
                    Path = relativePath,
                    Size = info.Length,
                    LastWriteTicks = info.LastWriteTimeUtc.Ticks,
                    DecodedHash = decodedHash,
                    FileHash = fileHash,
                    CompressionLevel = compressionLevel
                };
                dirty = true;
            }
        }

        public void RecordIcon(int id, FileInfo info, string sha256)
        {
            lock (gate)
            {
                icons[id] = new RestoredIconEntry
                {
                    Id = id,
                    Size = info.Length,
                    LastWriteTicks = info.LastWriteTimeUtc.Ticks,
                    Sha256 = sha256
                };
                dirty = true;
            }
        }

        public void Save(Action<string> noLinks = null)
        {
            RestoreStateData snapshot;
            lock (gate)
            {
                if (!dirty) return;
                snapshot = new RestoreStateData
                {
                    Version = CurrentVersion,
                    Files = files.Values.OrderBy(f => f.Path, StringComparer.OrdinalIgnoreCase).ToArray(),
                    Icons = icons.Values.OrderBy(i => i.Id).ToArray()
                };
            }
            string temporary = statePath + ".tower-part";
            noLinks?.Invoke(statePath);
            noLinks?.Invoke(temporary);
            Directory.CreateDirectory(Path.GetDirectoryName(statePath));
            try
            {
                using (var stream = File.Create(temporary))
                    new DataContractJsonSerializer(typeof(RestoreStateData)).WriteObject(stream, snapshot);
                if (File.Exists(statePath)) File.Replace(temporary, statePath, null);
                else File.Move(temporary, statePath);
                lock (gate) dirty = false;
            }
            finally
            {
                if (File.Exists(temporary)) File.Delete(temporary);
            }
        }
    }

    /// <summary>
    /// 与 Tower-Loader.ps1 的 Add-TowerLanguagePackage 保持一致的英文资源按需判定。
    /// 中文玩家未选英文界面时无需重建 locales/en-US/menu/hi/01_common.tpf.dcx。
    /// </summary>
    public static class AssetLanguage
    {
        static readonly Regex OptionLine = new Regex(@"^\s*language\s*=\s*([^#;\s]+)\s*(?:[#;].*)?$", RegexOptions.IgnoreCase | RegexOptions.Compiled);
        static readonly Regex SteamLang = new Regex(@"""UserConfig""\s*\{[^}]*""language""\s*""([^""]+)""", RegexOptions.IgnoreCase | RegexOptions.Singleline | RegexOptions.Compiled);
        static readonly HashSet<string> EnglishTags = new HashSet<string>(StringComparer.OrdinalIgnoreCase) { "en", "en-us", "engus", "english" };

        public static bool NeedsEnglish(string packageRoot, string gameDirectory)
        {
            string language = "auto";
            string options = Path.Combine(packageRoot, "tower-options.ini");
            if (File.Exists(options))
            {
                try
                {
                    foreach (var line in File.ReadAllLines(options))
                    {
                        var match = OptionLine.Match(line);
                        if (match.Success) language = match.Groups[1].Value.Trim().ToLowerInvariant();
                    }
                }
                catch (IOException) { }
            }
            if (language == "auto" || language == "game")
            {
                string steamApps = Parent3(gameDirectory);
                if (!string.IsNullOrEmpty(steamApps))
                {
                    string manifest = Path.Combine(steamApps, "appmanifest_1245620.acf");
                    if (File.Exists(manifest))
                    {
                        try
                        {
                            var match = SteamLang.Match(File.ReadAllText(manifest));
                            if (match.Success) language = match.Groups[1].Value.Trim().ToLowerInvariant();
                        }
                        catch (IOException) { }
                    }
                }
            }
            return EnglishTags.Contains(language);
        }

        static string Parent3(string path)
        {
            if (string.IsNullOrEmpty(path)) return null;
            var dir = new DirectoryInfo(path);
            for (int i = 0; i < 3; i++)
            {
                if (dir == null) return null;
                dir = dir.Parent;
            }
            return dir?.FullName;
        }
    }
}
