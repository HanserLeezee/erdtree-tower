using Tower.Update;
using Xunit;

namespace Tower.Update.Tests;

public sealed class FallbackSourceTests
{
    sealed class Fake(bool down = false) : IVersionedReleaseSource
    {
        public int Calls;
        public bool FailRanges;
        public Task<ReleaseManifest> LatestAsync(CancellationToken cancel) => VersionAsync("v127", cancel);
        public Task<ReleaseManifest> VersionAsync(string tag, CancellationToken cancel)
        {
            cancel.ThrowIfCancellationRequested(); Calls++;
            return down ? Task.FromException<ReleaseManifest>(new IOException("offline")) : Task.FromResult(new ReleaseManifest { Version = int.Parse(tag[1..]), Tag = tag });
        }
        public Task<byte[]> RangeAsync(string tag, string asset, long offset, long length, CancellationToken cancel)
        {
            cancel.ThrowIfCancellationRequested(); Calls++;
            return down || FailRanges ? Task.FromException<byte[]>(new IOException("offline")) : Task.FromResult(new byte[(int)length]);
        }
    }
    static MultiSource Primary(Fake a, Fake b) => new(new[] {
        new KeyValuePair<string,IReleaseSource>("Cloudflare", a), new KeyValuePair<string,IReleaseSource>("GitHub", b) });

    [Fact] public async Task HealthyPrimaryNeverContactsEnabledFallback()
    {
        var backup = new Fake(); var source = new FallbackSource(Primary(new Fake(), new Fake()), "Aliyun", backup);
        await source.LatestAsync(default); await source.RangeAsync("v127", "pack", 0, 4, default);
        Assert.Equal(0, backup.Calls); Assert.Equal("Cloudflare", source.Chosen);
    }
    [Fact] public async Task DisabledFallbackDoesNotHidePrimaryFailure()
    {
        var source = new FallbackSource(Primary(new Fake(true), new Fake(true)));
        await Assert.ThrowsAsync<IOException>(() => source.LatestAsync(default));
    }
    [Fact] public async Task FallbackUsesExactVersionAndAvoidsRetryingDeadPrimariesForEachChunk()
    {
        var a = new Fake(true); var b = new Fake(true); var backup = new Fake();
        var source = new FallbackSource(Primary(a,b), "Aliyun", backup);
        Assert.Equal(115, (await source.VersionAsync("v115", default)).Version);
        int calls = a.Calls + b.Calls;
        await source.RangeAsync("v115", "pack", 0, 4, default);
        Assert.Equal(calls, a.Calls + b.Calls); Assert.Equal("Aliyun", source.Chosen);
    }
    [Fact] public async Task RangeFailureCanActivateFallbackButUserCancellationCannot()
    {
        var backup = new Fake(); var source = new FallbackSource(Primary(new Fake { FailRanges=true }, new Fake { FailRanges=true }), "Aliyun", backup);
        await source.LatestAsync(default); Assert.Equal(0,backup.Calls);
        await source.RangeAsync("v127","pack",0,4,default); Assert.Equal(1,backup.Calls);
        using var cancel = new CancellationTokenSource(); cancel.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => source.LatestAsync(cancel.Token));
        Assert.Equal(1,backup.Calls);
    }
}
