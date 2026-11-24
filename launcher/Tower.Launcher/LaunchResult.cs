using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;

namespace Tower.Launcher
{
    internal static class LaunchResult
    {
        internal static string Summary(string errorFile, string logs, int exitCode)
        {
            var reason = Tail(errorFile).Trim();
            if (reason.IndexOf("Local asset generation failed",StringComparison.OrdinalIgnoreCase)>=0 || reason.Contains("本地资源生成失败")) {
                var detail=Tail(Path.Combine(logs,"stderr.log")).Split(new[]{'\r','\n'},StringSplitOptions.RemoveEmptyEntries)
                    .LastOrDefault(line=>line.Contains("资源准备失败："));
                if(detail!=null) reason=detail.Substring(detail.IndexOf("资源准备失败：",StringComparison.Ordinal));
            }
            if (reason.Length == 0) {
                var lines = (Tail(Path.Combine(logs, "stdout.log")) + "\n" + Tail(Path.Combine(logs, "stderr.log")))
                    .Split(new[] {'\r','\n'}, StringSplitOptions.RemoveEmptyEntries);
                reason = lines.LastOrDefault(l => Regex.IsMatch(l, "error|exception|failed|fatal|错误|失败|函数不正确|incorrect function", RegexOptions.IgnoreCase)) ?? "";
            }
            reason = reason.Replace('\r', ' ').Replace('\n', ' ').Trim();
            return reason.Length == 0 ? $"脚本退出码 {exitCode}，请打开日志查看详情" : WithHint(reason.Substring(0, Math.Min(300, reason.Length)));
        }
        internal static string WithHint(string reason)
        {
            if (reason.IndexOf("函数不正确", StringComparison.OrdinalIgnoreCase) < 0 &&
                reason.IndexOf("incorrect function", StringComparison.OrdinalIgnoreCase) < 0) return reason;
            return reason + " 已确认 MOD 放在 exFAT 分区可触发此错误。请右键 MOD 所在盘，打开“属性”查看文件系统。若为 exFAT，请退出游戏和启动器，将整个 MOD 文件夹复制到 NTFS 分区，从新位置启动。若在 NTFS 上仍报错，请打开日志，反馈本次启动的 stdout.log、stderr.log 和 MOD 文件夹完整路径。";
        }
        static string Tail(string path)
        {
            try {
                using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                file.Seek(Math.Max(0, file.Length - 16384), SeekOrigin.Begin);
                using var reader = new StreamReader(file, Encoding.UTF8, true);
                return reader.ReadToEnd();
            } catch (IOException) { return ""; }
            catch (UnauthorizedAccessException) { return ""; }
        }
    }
}
