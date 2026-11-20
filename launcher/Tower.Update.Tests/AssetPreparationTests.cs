using Tower.Assets;
using Tower.Launcher;
using Xunit;

public class AssetPreparationTests
{
    static string Folder() { var path=Path.Combine(Path.GetTempPath(),"tower-assets-"+Guid.NewGuid().ToString("N")); Directory.CreateDirectory(path); return path; }
    [Fact]
    public async Task ContenderWaitsThenAcquiresAfterOwnerFinishes()
    {
        if(!OperatingSystem.IsWindows()) return;
        var path=Path.Combine(Folder(),".restore.lock");
        using var owner=new FileStream(path,FileMode.OpenOrCreate,FileAccess.ReadWrite,FileShare.None);
        var reported=new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        var contender=Task.Run(()=>AssetLock.Acquire(path,TimeSpan.FromSeconds(5),()=>reported.TrySetResult(true)));
        try { await reported.Task.WaitAsync(TimeSpan.FromSeconds(2)); Assert.False(contender.IsCompleted); }
        finally { owner.Dispose(); }
        using var acquired=await contender.WaitAsync(TimeSpan.FromSeconds(5));
        Assert.True(acquired.CanWrite);
    }
    [Fact]
    public void BusyTimeoutIsDistinctAndStaleLockFileIsReusable()
    {
        if(!OperatingSystem.IsWindows()) return;
        var path=Path.Combine(Folder(),".restore.lock");
        using(var owner=new FileStream(path,FileMode.OpenOrCreate,FileAccess.ReadWrite,FileShare.None))
            Assert.Throws<TimeoutException>(()=>AssetLock.Acquire(path,TimeSpan.Zero,()=>{}));
        using var next=AssetLock.Acquire(path,TimeSpan.Zero,()=>throw new Exception("Not busy"));
        Assert.True(next.CanWrite);
    }
    [Fact]
    public void MissingDirectoryFailsWithoutPretendingToWait()
    {
        var path=Path.Combine(Folder(),"missing",".restore.lock");
        Assert.Throws<DirectoryNotFoundException>(()=>AssetLock.Acquire(path,TimeSpan.FromSeconds(30),()=>throw new Exception("Wrong retry")));
    }
    [Fact]
    public void AssetFailureShowsSpecificReasonInsteadOfWrapper()
    {
        var dir=Folder();var error=Path.Combine(dir,"error.txt");
        File.WriteAllText(error,"Local asset generation failed; see logs or use the full package.");
        File.WriteAllText(Path.Combine(dir,"stderr.log"),"资源准备失败：游戏压缩库版本不匹配，请使用完整整合包。\nLocal asset generation failed; see logs or use the full package.\n");
        Assert.Contains("游戏压缩库版本不匹配",LaunchResult.Summary(error,dir,1));
        File.WriteAllText(error,"Different startup failure");
        Assert.Equal("Different startup failure",LaunchResult.Summary(error,dir,1));
    }
}
