using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace Tower.Launcher
{
    static class NetworkProfile
    {
        public const string Steam = "Steam", OnlineFix = "OnlineFix";
        public const string SteamLabel = "Steam", OnlineFixLabel = "Spacewar（外部补丁）";
        static readonly (string Name, string Sha256)[] OnlineFixHashes = {
            ("OnlineFix64.dll", "71a4f566abbf2ceb3b0d6159cd7950d120593c5badda4dde2711671957f16003"),
            ("steam_api64.dll", "ca1fa9a7609ab10b7926400559cf073e5888423cc156af72c6027d72a89eea73"),
            ("winmm.dll", "54a6ece54641ee6e437caa351e493ddf4435f3e23e3d4ab4655c35f57b94fc59"),
        };

        public static string Detect(string? gameDir) => !string.IsNullOrWhiteSpace(gameDir) &&
            (File.Exists(Path.Combine(gameDir!, "OnlineFix64.dll")) || File.Exists(Path.Combine(gameDir!, "OnlineFix.ini"))) ? OnlineFix : Steam;
        public static string Normalize(string? value) => value == OnlineFix ? OnlineFix : Steam;
        public static string Label(string value) => value == OnlineFix ? OnlineFixLabel : SteamLabel;
        public static string AppId(string value) => value == OnlineFix ? "480" : "1245620";
        public static string PathFile(string value) => "GamePath.txt";
        public static string Runtime(string value) => value == OnlineFix ? "onlinefix-runtime" : "runtime";

        /// <summary>进程内完成 A/B 联机环境校验（与 distribution/Tower-Network.ps1 规则一致），避免 UI 线程阻塞在 powershell.exe。</summary>
        public static string Validate(string gameDir)
        {
            if (string.IsNullOrWhiteSpace(gameDir) || !Directory.Exists(gameDir))
                throw new InvalidOperationException("游戏目录无效或不存在。");
            if (File.Exists(Path.Combine(gameDir, ".tower-network", "pending.json")))
                throw new InvalidOperationException("上次 A/B 切换中途被中断（Interrupted manual patch operation）。请先关闭游戏和启动器，打开补丁包中的「联机环境切换(A-B).cmd」选择 A 还原后再启动。");
            var network = Detect(gameDir);
            if (network == Steam) return Steam;

            foreach (var (name, expected) in OnlineFixHashes)
            {
                var file = Path.Combine(gameDir, name);
                if (!File.Exists(file))
                    throw new InvalidOperationException($"缺少 B 补丁文件 {name}（Missing {name}），可能被杀毒软件误删或未完整解压。请检查隔离区并用「联机环境切换(A-B).cmd」重新选择 B 或还原为 A。");
                if (!string.Equals(Sha256Of(file), expected, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException($"B 补丁组件版本不匹配：{name}（Unsupported OnlineFix component）。仅支持本补丁包自带的 2.0.3 组件，请勿混用其他来源的 OnlineFix 文件。");
            }
            var iniFile = Path.Combine(gameDir, "OnlineFix.ini");
            if (!File.Exists(iniFile))
                throw new InvalidOperationException("缺少 B 补丁文件 OnlineFix.ini（Missing OnlineFix.ini），可能被杀毒软件误删或未完整解压。请检查隔离区并用「联机环境切换(A-B).cmd」重新选择 B 或还原为 A。");
            var ini = File.ReadAllText(iniFile);
            foreach (var (key, expected) in new[] { ("RealAppId", "1245620"), ("FakeAppId", "480") })
            {
                var matches = Regex.Matches(ini, $@"^\s*{key}\s*=\s*([^\r\n;#]+)", RegexOptions.Multiline | RegexOptions.IgnoreCase);
                if (matches.Count != 1 || matches[0].Groups[1].Value.Trim() != expected)
                    throw new InvalidOperationException($"OnlineFix.ini 配置异常（Unexpected OnlineFix {key}），应为 RealAppId=1245620 且 FakeAppId=480。");
            }
            if (File.Exists(Path.Combine(gameDir, "steam_api64_o.dll")))
                throw new InvalidOperationException("检测到混装的其他 Steam 代理文件 steam_api64_o.dll（Mixed Steam API proxies detected）。请先清理旧代理并通过 Steam 验证游戏文件完整性，再用「联机环境切换(A-B).cmd」切换。");
            var dllListFile = Path.Combine(gameDir, "dlllist.txt");
            if (!File.Exists(dllListFile))
                throw new InvalidOperationException("缺少 B 补丁文件 dlllist.txt（Missing dlllist.txt）。请关闭启动器，运行补丁包中的「联机环境切换(A-B).cmd」选择 B 重新应用一次。");
            var bytes = File.ReadAllBytes(dllListFile);
            var canonical = Encoding.ASCII.GetBytes("OnlineFix64.dll");
            if (!bytes.SequenceEqual(canonical))
                throw new InvalidOperationException("B 补丁的 dlllist.txt 未适配当前爬塔启动器（B patch list is not ready）。请关闭启动器，运行补丁包中的「联机环境切换(A-B).cmd」选择 B 重新应用一次。");
            return OnlineFix;
        }

        static string Sha256Of(string file)
        {
            using var sha = SHA256.Create();
            using var stream = File.OpenRead(file);
            return BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "").ToLowerInvariant();
        }
    }
}
