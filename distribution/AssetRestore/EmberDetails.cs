using System;
using System.Diagnostics;
using System.IO;
using System.Linq;

namespace Tower.Assets
{
    internal static class EmberDetails
    {
        public static void Restore(string package, string game, RestoreState state, bool verify, Action<string> noLinks)
        {
            foreach (string quality in new[] { "hi", "low" })
                foreach (string suffix in new[] { "dt", "hd" })
                    RestoreFile(package, game, state, verify, noLinks, quality, suffix);
        }

        static void RestoreFile(string package, string game, RestoreState state, bool verify, Action<string> noLinks, string quality, string suffix)
        {
            string path = "menu/" + quality + "/00_solo.tpfb" + suffix;
            string target = Path.Combine(package, "scene-assets", path);
            string delta = Path.Combine(package, "asset-patches", "ember-" + quality + "-" + suffix + ".patch");
            noLinks(target);
            noLinks(delta);
            using (var patch = new BinaryReader(File.OpenRead(delta)))
            {
                if (patch.ReadString() != "TowerDetail1") throw new InvalidDataException(delta);
                string archive = patch.ReadString();
                long offset = patch.ReadInt64();
                int shared = patch.ReadInt32();
                long length = patch.ReadInt64();
                string hash = patch.ReadString();
                if (!new[] {"Data0.bdt","Data1.bdt","Data2.bdt","Data3.bdt","DLC.bdt"}.Contains(archive)
                    || offset < 0 || shared < 0 || shared > length || length > int.MaxValue || hash.Length != 64)
                    throw new InvalidDataException(delta);
                var recipe = new AssetRecipe { Path = path, TargetHash = hash, DecodedHash = hash };
                if (state.IsFileUpToDate(target, recipe, verify)) return;
                if (Process.GetProcessesByName("eldenring").Length != 0) throw new IOException("请先退出游戏再生成余烬详情图。");
                Progress.Report("building", 0, 0, "正在生成本地资源", path);
                Directory.CreateDirectory(Path.GetDirectoryName(target));
                string temp = target + "." + Guid.NewGuid().ToString("N") + ".part";
                try
                {
                    using (var input = File.OpenRead(Path.Combine(game, archive)))
                    using (var output = new FileStream(temp, FileMode.CreateNew, FileAccess.Write))
                    {
                        if (offset > input.Length - shared) throw new EndOfStreamException(archive);
                        input.Position = offset;
                        var buffer = new byte[1024 * 1024];
                        for (int remaining = shared; remaining > 0;)
                        {
                            int count = input.Read(buffer, 0, Math.Min(buffer.Length, remaining));
                            if (count == 0) throw new EndOfStreamException(archive);
                            output.Write(buffer, 0, count);
                            remaining -= count;
                        }
                        output.SetLength(length);
                        while (patch.BaseStream.Position < patch.BaseStream.Length)
                        {
                            int position = patch.ReadInt32(), size = patch.ReadInt32();
                            if (size <= 0 || size > 4096 || position < 0 || position > length - size) throw new InvalidDataException(delta);
                            var bytes = patch.ReadBytes(size);
                            if (bytes.Length != size) throw new EndOfStreamException(delta);
                            output.Position = position;
                            output.Write(bytes, 0, size);
                        }
                    }
                    if (AssetHash.FileHash(temp) != hash) throw new InvalidDataException("余烬详情图与当前游戏资源不匹配：" + path);
                    if (File.Exists(target)) File.Replace(temp, target, null);
                    else File.Move(temp, target);
                    state.RecordFile(path, new FileInfo(target), hash, hash, 0);
                }
                finally { if (File.Exists(temp)) File.Delete(temp); }
            }
        }
    }
}
