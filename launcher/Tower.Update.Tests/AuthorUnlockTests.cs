using Tower.Launcher;
using Xunit;

public class AuthorUnlockTests
{
    [Fact]
    public void RequiresFiveRapidClicks()
    {
        var unlock = new AuthorUnlock();
        for (var i = 0; i < 4; i++) Assert.False(unlock.Tap(i * 200));
        Assert.True(unlock.Tap(800));
    }

    [Fact]
    public void SlowGapResetsSequence()
    {
        var unlock = new AuthorUnlock();
        for (var i = 0; i < 4; i++) Assert.False(unlock.Tap(i * 200));
        Assert.False(unlock.Tap(1300));
        for (var i = 1; i < 4; i++) Assert.False(unlock.Tap(1300 + i * 200));
        Assert.True(unlock.Tap(2100));
    }

    [Fact]
    public void UnlockSurvivesRestartWithoutChangingNetwork()
    {
        var path = Path.GetTempFileName();
        try
        {
            Assert.False(new HostSettings().NetworkUnlocked);
            File.WriteAllText(path, "{}");
            Assert.False(HostSettings.Load(path).NetworkUnlocked);
            new HostSettings { NetworkUnlocked = true, Network = NetworkProfile.OnlineFix }.Save(path);
            var settings = HostSettings.Load(path);
            Assert.True(settings.NetworkUnlocked);
            Assert.Equal(NetworkProfile.OnlineFix, settings.Network);
        }
        finally { File.Delete(path); }
    }
}
