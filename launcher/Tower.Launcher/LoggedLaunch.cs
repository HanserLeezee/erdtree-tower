using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading.Tasks;

namespace Tower.Launcher
{
    internal sealed class LoggedLaunch
    {
        public int Id { get; }
        public DateTime StartTime { get; }
        readonly Task<int> exited;
        public Task Completion { get; }
        public bool HasExited => exited.IsCompleted;
        public int ExitCode => exited.GetAwaiter().GetResult();

        LoggedLaunch(Process process, string directory, StreamWriter stdout, StreamWriter stderr)
        {
            Id = process.Id;
            StartTime = process.StartTime;
            exited = Task.Run(() => { process.WaitForExit(); return process.ExitCode; });
            Completion = Capture(process, directory, stdout, stderr);
        }

        public static LoggedLaunch Start(string executable, string arguments, string workingDirectory, string logDirectory)
        {
            Directory.CreateDirectory(logDirectory);
            StreamWriter Writer(string name) => new StreamWriter(new FileStream(Path.Combine(logDirectory, name),
                FileMode.Create, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete), new UTF8Encoding(false)) { AutoFlush = true };
            var stdout = Writer("stdout.log");
            StreamWriter? stderr = null;
            var process = new Process { StartInfo = new ProcessStartInfo(executable, arguments) {
                UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = workingDirectory,
                RedirectStandardOutput = true, RedirectStandardError = true,
                StandardOutputEncoding = Encoding.UTF8, StandardErrorEncoding = Encoding.UTF8,
            }};
            try {
                stderr = Writer("stderr.log");
                if (!process.Start()) throw new InvalidOperationException("无法创建启动进程");
                return new LoggedLaunch(process, logDirectory, stdout, stderr);
            } catch { process.Dispose(); stdout.Dispose(); stderr?.Dispose(); throw; }
        }

        static async Task Pump(StreamReader input, StreamWriter output)
        {
            string? line;
            while ((line = await input.ReadLineAsync().ConfigureAwait(false)) != null)
                await output.WriteLineAsync(line).ConfigureAwait(false);
        }

        async Task Capture(Process process, string directory, StreamWriter stdout, StreamWriter stderr)
        {
            using (process) using (stdout) using (stderr) {
                try {
                    await Task.WhenAll(Pump(process.StandardOutput, stdout), Pump(process.StandardError, stderr), exited).ConfigureAwait(false);
                    File.WriteAllText(Path.Combine(directory, "exit-code.txt"), ExitCode.ToString(), new UTF8Encoding(false));
                } catch (Exception error) { Log.Write("启动日志采集失败：" + error.Message); }
            }
        }
    }
}
