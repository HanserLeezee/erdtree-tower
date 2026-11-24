using System.Text;
using Tower.Launcher;
using Xunit;

public class LoggedLaunchTests
{
    [Fact]
    public async Task DirectWorkerStreamsUtf8AndKeepsLoggingAfterFirstFrame()
    {
        if (!OperatingSystem.IsWindows()) return; // 依赖 Windows PowerShell
        var root = Path.Combine(Path.GetTempPath(), "tower 日志 ' " + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        var entry = Path.Combine(root, "worker.ps1");
        File.WriteAllText(entry, "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)\n[Console]::WriteLine($PID)\nWrite-Output 'First overlay frame rendered'\nStart-Sleep -Milliseconds 150\nWrite-Output '资源检查通过'\n[Console]::Error.WriteLine('测试错误')\nexit 17", new UTF8Encoding(true));
        var shell = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "WindowsPowerShell", "v1.0", "powershell.exe");
        var worker = LoggedLaunch.Start(shell, "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"" + entry + "\"", root, root);
        await worker.Completion.WaitAsync(TimeSpan.FromSeconds(20));
        Assert.True(worker.HasExited);
        Assert.Equal(17, worker.ExitCode);
        var output = File.ReadAllLines(Path.Combine(root, "stdout.log"));
        Assert.Equal(worker.Id.ToString(), output[0]);
        Assert.Contains("资源检查通过", output);
        Assert.Contains("测试错误", File.ReadAllText(Path.Combine(root, "stderr.log")));
        Assert.Equal("17", File.ReadAllText(Path.Combine(root, "exit-code.txt")));
    }

    [Fact]
    public void DecodeLinePrefersUtf8AndFallsBackToAnsiForEarlyErrors()
    {
        var utf8 = Encoding.UTF8.GetBytes("资源检查通过\r");
        Assert.Equal("资源检查通过", LoggedLaunch.DecodeLine(utf8, utf8.Length));
        var latin = Encoding.Latin1.GetBytes("caf\u00e9\r");
        Assert.Equal("café", LoggedLaunch.DecodeLine(latin, latin.Length, Encoding.Latin1));
    }
}
