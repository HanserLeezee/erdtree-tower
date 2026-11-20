using System.Text.Json;
using Tower.Launcher;
using Xunit;

namespace Tower.Update.Tests;

public sealed class SaveRecoveryTests : IDisposable
{
    readonly string root = Path.Combine(Path.GetTempPath(), "tower-recovery-test-" + Guid.NewGuid().ToString("N"));
    const string Run = "{\"phase\":\"Map\",\"nodes\":[{}],\"visited\":[],\"hp\":100,\"gold\":123}";
    string Save(string content = Run)
    {
        var folder = Path.Combine(root, "seamless-entry", "runtime");
        Directory.CreateDirectory(folder);
        var path = Path.Combine(folder, "tower-2-17.json");
        File.WriteAllText(path, content); return path;
    }
    [Fact]
    public void CreatesOwnerBoundSnapshotAfterBackupWithoutChangingOriginal()
    {
        var path = Save();
        File.WriteAllText(Path.ChangeExtension(path, "history"), "history");
        var backup = SaveRecovery.Prepare(root, path, () => false);
        Assert.Equal(Run, File.ReadAllText(path));
        Assert.Equal(Run, File.ReadAllText(Path.Combine(backup, Path.GetFileName(path))));
        Assert.Equal("history", File.ReadAllText(Path.Combine(backup, "tower-2-17.history")));
        using var doc = JsonDocument.Parse(File.ReadAllText(Path.ChangeExtension(path, "paused")));
        Assert.Equal(1, doc.RootElement.GetProperty("version").GetInt32());
        Assert.Equal(new long[] { 17, 2 }, doc.RootElement.GetProperty("owner").EnumerateArray().Select(x => x.GetInt64()));
        Assert.Equal(Run, doc.RootElement.GetProperty("run").GetString());
        Assert.Throws<IOException>(() => SaveRecovery.Prepare(root, path, () => false));
    }
    [Theory]
    [InlineData("reset")]
    [InlineData("party-award")]
    [InlineData("paused")]
    public void PendingTransactionsAndExistingSnapshotsAreNeverOverwritten(string suffix)
    {
        var path = Save(); var marker = Path.ChangeExtension(path, suffix);
        File.WriteAllText(marker, "original");
        Assert.Throws<IOException>(() => SaveRecovery.Prepare(root, path, () => false));
        Assert.Equal("original", File.ReadAllText(marker)); Assert.Equal(Run, File.ReadAllText(path));
    }
    [Fact]
    public void RunningGameAndConcurrentLaunchPreventRepair()
    {
        var path = Save();
        Assert.Throws<IOException>(() => SaveRecovery.Prepare(root, path, () => true));
        using var launch = new FileStream(Path.Combine(root, ".tower-launch.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        Assert.Throws<IOException>(() => SaveRecovery.Prepare(root, path, () => false));
        Assert.False(File.Exists(Path.ChangeExtension(path, "paused")));
    }
    [Fact]
    public void GameStartingDuringPreparationLeavesOnlyBackup()
    {
        var path = Save(); var checks = 0;
        Assert.Throws<IOException>(() => SaveRecovery.Prepare(root, path, () => ++checks == 3));
        Assert.False(File.Exists(Path.ChangeExtension(path, "paused")));
        Assert.Equal(Run, File.ReadAllText(path));
    }
    [Theory]
    [InlineData("{broken")]
    [InlineData("{\"phase\":\"Map\"}")]
    [InlineData("{\"phase\":{\"Combat\":{\"id\":0}}}")]
    public void InvalidOrNonMapRecordsNeverProduceSnapshot(string json)
    {
        var path = Save(json);
        Assert.ThrowsAny<Exception>(() => SaveRecovery.Prepare(root, path, () => false));
        Assert.False(File.Exists(Path.ChangeExtension(path, "paused")));
        Assert.Equal(json, File.ReadAllText(path));
    }
    public void Dispose() { if (Directory.Exists(root)) Directory.Delete(root, true); }
}
