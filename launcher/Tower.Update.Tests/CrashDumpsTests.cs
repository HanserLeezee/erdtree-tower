using Tower.Launcher;
using Xunit;

public class CrashDumpsTests
{
    [Fact]
    public void Finds_only_elden_ring_dumps_and_cleans_them()
    {
        var root = Path.Combine(Path.GetTempPath(), "tower-dumps-" + Guid.NewGuid().ToString("N"));
        var local = Path.Combine(root, "local");
        var game = Path.Combine(root, "Game");
        void Write(string path, int size) { Directory.CreateDirectory(Path.GetDirectoryName(path)!); File.WriteAllBytes(path, new byte[size]); }
        try
        {
            Write(Path.Combine(local, "CrashDumps", "eldenring.exe.1234.dmp"), 300);
            Write(Path.Combine(local, "CrashDumps", "chrome.exe.42.dmp"), 50);
            Write(Path.Combine(local, "Microsoft", "Windows", "WER", "ReportArchive", "AppCrash_eldenring.exe_abc", "memory.hdmp"), 200);
            Write(Path.Combine(local, "Microsoft", "Windows", "WER", "ReportArchive", "AppCrash_eldenring.exe_abc", "Report.wer"), 10);
            Write(Path.Combine(game, "eldenring_crash.dmp"), 100);
            Write(Path.Combine(game, "mods", "deep", "eldenring.dmp"), 70);
            Write(Path.Combine(game, "eldenring.exe"), 5);

            var report = CrashDumps.Scan(CrashDumps.Roots(local, game));
            Assert.Equal(3, report.Files.Count);
            Assert.Equal(600, report.Bytes);

            var (freed, failed) = CrashDumps.Clean(report);
            Assert.Equal(600, freed);
            Assert.Equal(0, failed);
            Assert.True(File.Exists(Path.Combine(local, "CrashDumps", "chrome.exe.42.dmp")));
            Assert.True(File.Exists(Path.Combine(game, "mods", "deep", "eldenring.dmp")));
            Assert.True(File.Exists(Path.Combine(game, "eldenring.exe")));
            Assert.Empty(CrashDumps.Scan(CrashDumps.Roots(local, game)).Files);
        }
        finally { Directory.Delete(root, true); }
    }

    [Theory]
    [InlineData(5L << 20, "5 MB")]
    [InlineData(3L << 30, "3.0 GB")]
    public void Formats_sizes(long bytes, string text) => Assert.Equal(text, CrashDumps.Size(bytes));
}
