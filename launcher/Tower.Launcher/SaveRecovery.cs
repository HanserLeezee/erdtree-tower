using System;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Tower.Launcher
{
    static class SaveRecovery
    {
        public static string[] Folders(string root) => new[] { root,
            Path.Combine(root, "seamless-entry", "runtime"),
            Path.Combine(root, "seamless-entry", "onlinefix-runtime"),
            Path.Combine(root, "seamless-entry", "party-runtime") }.Select(Path.GetFullPath).ToArray();

        static void Regular(string path)
        {
            for (var item = new FileInfo(path) as FileSystemInfo; item != null;
                item = item is FileInfo file ? file.Directory : ((DirectoryInfo)item).Parent)
                if (item.Exists && (item.Attributes & FileAttributes.ReparsePoint) != 0)
                    throw new IOException("修复路径不能经过目录联接或符号链接，请在实际安装目录操作。");
        }

        public static string Prepare(string root, string selected, Func<bool> gameRunning)
        {
            root = Path.GetFullPath(root); selected = Path.GetFullPath(selected);
            var folder = Path.GetDirectoryName(selected)!;
            if (!Folders(root).Contains(folder, StringComparer.OrdinalIgnoreCase))
                throw new IOException("请选择当前整合包内的角色记录。");
            var match = Regex.Match(Path.GetFileName(selected), @"^tower-(\d+)-(\d+)\.json$");
            if (!match.Success || !int.TryParse(match.Groups[1].Value, out var slot) || slot > 9 ||
                !uint.TryParse(match.Groups[2].Value, out var character))
                throw new IOException("请选择 tower-槽位-角色编号.json，不能选择游戏存档或其他配置。");
            Regular(selected);
            if (gameRunning()) throw new IOException("请先退出游戏，再修复地图卡档。");
            using var launchLock = new FileStream(Path.Combine(root, ".tower-launch.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
            if (gameRunning()) throw new IOException("游戏正在运行，修复已取消。");
            var stem = Path.GetFileNameWithoutExtension(selected);
            var paused = Path.ChangeExtension(selected, "paused");
            if (File.Exists(Path.ChangeExtension(selected, "reset"))) throw new IOException("该角色有未完成的职业重置，不能按地图卡档修复。请保留日志排查。");
            if (File.Exists(Path.ChangeExtension(selected, "party-award"))) throw new IOException("该角色有联机奖励发放记录，暂不自动修复，避免重复发奖。请保留日志排查。");
            if (File.Exists(paused)) throw new IOException("该角色已有暂存旅程，未覆盖。请先单独进入游戏，在大赐福继续旅程。");
            if (new FileInfo(selected).Length > 128 * 1024) throw new IOException("旅程记录过大，无法安全修复。");
            var run = File.ReadAllText(selected);
            using (var document = JsonDocument.Parse(run))
            {
                var data = document.RootElement;
                if (!data.TryGetProperty("phase", out var phase) || phase.ValueKind != JsonValueKind.String || phase.GetString() != "Map")
                    throw new IOException("此功能仅修复选地图阶段，不会跳过战斗、奖励或事件。");
                if (!data.TryGetProperty("nodes", out var nodes) || nodes.ValueKind != JsonValueKind.Array || nodes.GetArrayLength() == 0 ||
                    !data.TryGetProperty("visited", out var visited) || visited.ValueKind != JsonValueKind.Array ||
                    !data.TryGetProperty("hp", out var hp) || !hp.TryGetUInt32(out var health) || health == 0)
                    throw new IOException("旅程记录不完整，已停止修复。请保留日志排查。");
            }
            var backup = Path.Combine(root, "save-recovery-backups", DateTime.Now.ToString("yyyyMMdd-HHmmss") + "-" + Guid.NewGuid().ToString("N"));
            Regular(backup);
            Directory.CreateDirectory(backup);
            foreach (var file in Directory.EnumerateFiles(folder, stem + ".*", SearchOption.TopDirectoryOnly))
            {
                Regular(file);
                File.Copy(file, Path.Combine(backup, Path.GetFileName(file)));
            }
            File.WriteAllText(Path.Combine(backup, "恢复说明.txt"),
                "原记录目录：" + folder + "\r\n本次只新增 " + Path.GetFileName(paused) +
                "，原始旅程记录未修改。游戏完整校验暂存后才会载入。\r\n若尚未继续旅程，退出游戏后移走该 .paused 文件即可撤销本次修复。不要覆盖其他角色。\r\n");
            var snapshot = JsonSerializer.Serialize(new { version = 1, owner = new long[] { character, slot }, run });
            var temp = paused + ".repair-" + Guid.NewGuid().ToString("N");
            try
            {
                File.WriteAllText(temp, snapshot);
                if (gameRunning()) throw new IOException("游戏已启动，修复已取消；备份保留在 " + backup);
                File.Move(temp, paused);
            }
            finally { if (File.Exists(temp)) File.Delete(temp); }
            return backup;
        }
    }
}
