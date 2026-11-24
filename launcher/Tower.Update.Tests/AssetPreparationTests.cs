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

    static byte[] FakeKrakHeader(int uncompressedSize, int compressedSize, byte level)
    {
        var header = new byte[76];
        System.Text.Encoding.ASCII.GetBytes("DCX\0").CopyTo(header, 0);
        header[23] = 76;
        header[28] = (byte)(uncompressedSize >> 24); header[29] = (byte)(uncompressedSize >> 16);
        header[30] = (byte)(uncompressedSize >> 8); header[31] = (byte)uncompressedSize;
        header[32] = (byte)(compressedSize >> 24); header[33] = (byte)(compressedSize >> 16);
        header[34] = (byte)(compressedSize >> 8); header[35] = (byte)compressedSize;
        System.Text.Encoding.ASCII.GetBytes("KRAK").CopyTo(header, 40);
        header[48] = level;
        return header;
    }

    [Fact]
    public void BuildHeaderUpdatesCompressedSizeAndLevelWithoutMutatingTemplate()
    {
        var template = FakeKrakHeader(340_000_000, 97_000_000, 6);
        var updated = OodleDcx.BuildHeader(template, 340_000_000, 108_123_456, OodleDcx.FastCompressionLevel);
        Assert.Equal(6, template[48]);
        Assert.Equal((byte)OodleDcx.FastCompressionLevel, updated[48]);
        int actualCompressed = (updated[32] << 24) | (updated[33] << 16) | (updated[34] << 8) | updated[35];
        Assert.Equal(108_123_456, actualCompressed);
        Assert.Throws<InvalidDataException>(() => OodleDcx.BuildHeader(template, 340_000_000, 100, 0));
        Assert.Throws<InvalidDataException>(() => OodleDcx.BuildHeader(template, 1234, 100, 2));
    }

    [Fact]
    public void RestoreStateAcceptsLegacyTargetHashAndFastEncodedCacheAcrossReload()
    {
        var root = Folder();
        var statePath = Path.Combine(root, ".restore-state.json");
        var target = Path.Combine(root, "01_common.tpf.dcx");

        // 1) Legacy Level-6 file matches TargetHash without prior state file
        var legacyBytes = new byte[] { 1, 2, 3, 4, 5, 6 };
        File.WriteAllBytes(target, legacyBytes);
        var recipe = new AssetRecipe { Path = "menu/hi/01_common.tpf.dcx", DecodedHash = "dec-aaa", TargetHash = AssetHash.Of(legacyBytes) };
        var state = RestoreState.Load(statePath);
        Assert.True(state.IsFileUpToDate(target, recipe, ignoreCache: false));
        state.Save();
        Assert.True(File.Exists(statePath));

        // 2) Fast Level-2 file has a different compressed hash, recorded in state and verified across reload
        var fastBytes = new byte[] { 9, 8, 7, 6, 5, 4, 3 };
        File.WriteAllBytes(target, fastBytes);
        state.RecordFile(recipe.Path, new FileInfo(target), recipe.DecodedHash, AssetHash.Of(fastBytes), OodleDcx.FastCompressionLevel);
        state.Save();

        var reloaded = RestoreState.Load(statePath);
        Assert.True(reloaded.IsFileUpToDate(target, recipe, ignoreCache: false));
        // Even with ignoreCache=true (e.g. timestamp changed), FileHash still matches!
        Assert.True(reloaded.IsFileUpToDate(target, recipe, ignoreCache: true));

        // 3) Manifest update changes DecodedHash -> invalidates cache immediately
        var updatedRecipe = new AssetRecipe { Path = recipe.Path, DecodedHash = "dec-bbb", TargetHash = "other" };
        Assert.False(reloaded.IsFileUpToDate(target, updatedRecipe, ignoreCache: false));

        // 4) Corrupted file on disk -> fails check
        File.WriteAllBytes(target, new byte[] { 0, 0 });
        Assert.False(reloaded.IsFileUpToDate(target, recipe, ignoreCache: false));
    }

    [Fact]
    public void RestoreStateTracksIconsAndInvalidatesOnHashOrSizeChange()
    {
        var root = Folder();
        var statePath = Path.Combine(root, ".restore-state.json");
        var iconPath = Path.Combine(root, "123.rgba");
        var pixels = new byte[102400];
        pixels[0] = 42;
        File.WriteAllBytes(iconPath, pixels);
        var recipe = new IconRecipe { Id = 123, Sha256 = AssetHash.Of(pixels) };

        var state = RestoreState.Load(statePath);
        Assert.True(state.IsIconUpToDate(iconPath, recipe, ignoreCache: false));
        state.Save();

        var reloaded = RestoreState.Load(statePath);
        Assert.True(reloaded.IsIconUpToDate(iconPath, recipe, ignoreCache: false));
        Assert.False(reloaded.IsIconUpToDate(iconPath, new IconRecipe { Id = 123, Sha256 = "different" }, ignoreCache: false));

        File.WriteAllBytes(iconPath, new byte[10]);
        Assert.False(reloaded.IsIconUpToDate(iconPath, recipe, ignoreCache: false));
    }

    [Fact]
    public void AssetLanguageDetectsOnDemandEnglishFromOptionsAndSteamManifest()
    {
        var root = Folder();
        var steamApps = Path.Combine(Folder(), "steamapps");
        var gameDir = Path.Combine(steamApps, "common", "ELDEN RING", "Game");
        Directory.CreateDirectory(gameDir);

        // Default auto with no Steam manifest -> Chinese (English not needed)
        Assert.False(AssetLanguage.NeedsEnglish(root, gameDir));

        // Steam manifest set to english -> English needed under auto
        File.WriteAllText(Path.Combine(steamApps, "appmanifest_1245620.acf"), "\"AppState\" { \"UserConfig\" { \"language\" \"english\" } }");
        Assert.True(AssetLanguage.NeedsEnglish(root, gameDir));

        // Explicit zh-CN in tower-options.ini overrides Steam english
        File.WriteAllText(Path.Combine(root, "tower-options.ini"), "language = zh-CN # comment\n");
        Assert.False(AssetLanguage.NeedsEnglish(root, gameDir));

        // Explicit en-US overrides Steam schinese
        File.WriteAllText(Path.Combine(steamApps, "appmanifest_1245620.acf"), "\"AppState\" { \"UserConfig\" { \"language\" \"schinese\" } }");
        File.WriteAllText(Path.Combine(root, "tower-options.ini"), "language = en-US\n");
        Assert.True(AssetLanguage.NeedsEnglish(root, gameDir));
    }
}
