using System.IO;

namespace Tower.Launcher
{
    static class NetworkProfile
    {
        public const string Steam = "Steam", OnlineFix = "OnlineFix";
        public const string SteamLabel = "Steam", OnlineFixLabel = "Spacewar（外部补丁）";
        public static string Detect(string? gameDir) => !string.IsNullOrWhiteSpace(gameDir) &&
            (File.Exists(Path.Combine(gameDir!, "OnlineFix64.dll")) || File.Exists(Path.Combine(gameDir!, "OnlineFix.ini"))) ? OnlineFix : Steam;
        public static string Normalize(string? value) => value == OnlineFix ? OnlineFix : Steam;
        public static string Label(string value) => value == OnlineFix ? OnlineFixLabel : SteamLabel;
        public static string AppId(string value) => value == OnlineFix ? "480" : "1245620";
        public static string PathFile(string value) => "GamePath.txt";
        public static string Runtime(string value) => value == OnlineFix ? "onlinefix-runtime" : "runtime";
    }
}
