using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace Tower.Launcher
{
    /// <summary>
    /// 游戏崩溃或退出时 Windows 错误报告（WER LocalDumps）可能写出完整内存转储，
    /// DumpType=2 时每个 .dmp 有好几 GB，长期累积会塞满 C 盘。这里只扫描与法环相关的转储。
    /// </summary>
    static class CrashDumps
    {
        // WER 的命名是 eldenring.exe.1234.dmp / AppCrash_eldenring.exe_…；Seamless 等自带的转储一般以 eldenring 开头
        static readonly string[] Markers = { "eldenring", "start_protected_game.exe", "me3.exe", "me3-launcher.exe" };

        public sealed class Report
        {
            public List<string> Files { get; } = new List<string>();
            public long Bytes { get; set; }
        }

        /// <summary>转储所在目录：WER 默认的 CrashDumps、WER 报告队列/归档，以及游戏目录顶层</summary>
        public static IEnumerable<string> Roots(string localAppData, string? gameDir)
        {
            yield return Path.Combine(localAppData, "CrashDumps");
            yield return Path.Combine(localAppData, "Microsoft", "Windows", "WER", "ReportQueue");
            yield return Path.Combine(localAppData, "Microsoft", "Windows", "WER", "ReportArchive");
            if (!string.IsNullOrEmpty(gameDir)) yield return gameDir!;
        }

        /// <summary>文件名（或 WER 报告文件夹名）带有法环相关进程名的 .dmp/.hdmp/.mdmp</summary>
        public static bool Related(string root, string file)
        {
            var ext = Path.GetExtension(file).ToLowerInvariant();
            if (ext != ".dmp" && ext != ".hdmp" && ext != ".mdmp") return false;
            var relative = file.Substring(root.Length).TrimStart('\\', '/');
            var parts = relative.Split('\\', '/');
            // 根目录或下一层（WER 报告文件夹）；不往 mod / 存档等更深的目录里找
            if (parts.Length > 2) return false;
            return Markers.Any(m => relative.IndexOf(m, StringComparison.OrdinalIgnoreCase) >= 0);
        }

        // net48 没有 EnumerationOptions：手动只往下走一层，跳过联接点
        static List<string> Files(string root)
        {
            var files = new List<string>();
            try { files.AddRange(Directory.GetFiles(root)); } catch (Exception) { return files; }
            string[] folders;
            try { folders = Directory.GetDirectories(root); } catch (Exception) { return files; }
            foreach (var folder in folders)
            {
                try
                {
                    if ((File.GetAttributes(folder) & FileAttributes.ReparsePoint) != 0) continue;
                    files.AddRange(Directory.GetFiles(folder));
                }
                catch (Exception) { }
            }
            return files;
        }

        public static Report Scan(IEnumerable<string> roots)
        {
            var report = new Report();
            foreach (var root in roots.Where(Directory.Exists).Distinct(StringComparer.OrdinalIgnoreCase))
            {
                foreach (var file in Files(root))
                {
                    if (!Related(root, file)) continue;
                    try { report.Bytes += new FileInfo(file).Length; report.Files.Add(file); }
                    catch (Exception) { }
                }
            }
            return report;
        }

        /// <summary>删除扫描到的转储，返回 (已删字节, 删不掉的数量)。正在写入的文件会被跳过</summary>
        public static (long freed, int failed) Clean(Report report)
        {
            long freed = 0; int failed = 0;
            foreach (var file in report.Files)
            {
                try { var size = new FileInfo(file).Length; File.Delete(file); freed += size; }
                catch (Exception) { failed++; }
            }
            return (freed, failed);
        }

        public static string Size(long bytes) =>
            bytes >= 1L << 30 ? $"{bytes / (double)(1L << 30):0.0} GB" : $"{Math.Max(1, bytes >> 20)} MB";
    }
}
