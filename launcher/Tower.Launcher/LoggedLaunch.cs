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

        static readonly UTF8Encoding StrictUtf8 = new UTF8Encoding(false, true);

        /// <summary>优先按严格 UTF-8 解码；若 PowerShell 早期报错输出系统 ANSI（中文系统 GBK），回退系统默认编码，避免乱码。</summary>
        internal static string DecodeLine(byte[] bytes, int count, Encoding? fallback = null)
        {
            while (count > 0 && bytes[count - 1] == (byte)'\r') count--;
            if (count == 0) return "";
            try { return StrictUtf8.GetString(bytes, 0, count); }
            catch (DecoderFallbackException) { return (fallback ?? Encoding.Default).GetString(bytes, 0, count); }
        }

        static async Task Pump(Stream input, StreamWriter output)
        {
            var buffer = new byte[4096];
            using var line = new MemoryStream(256);
            int read;
            while ((read = await input.ReadAsync(buffer, 0, buffer.Length).ConfigureAwait(false)) > 0)
            {
                var start = 0;
                for (var i = 0; i < read; i++)
                {
                    if (buffer[i] != (byte)'\n') continue;
                    if (i > start) line.Write(buffer, start, i - start);
                    await output.WriteLineAsync(DecodeLine(line.GetBuffer(), (int)line.Length)).ConfigureAwait(false);
                    line.SetLength(0);
                    start = i + 1;
                }
                if (start < read) line.Write(buffer, start, read - start);
            }
            if (line.Length > 0)
                await output.WriteLineAsync(DecodeLine(line.GetBuffer(), (int)line.Length)).ConfigureAwait(false);
        }

        async Task Capture(Process process, string directory, StreamWriter stdout, StreamWriter stderr)
        {
            using (process) using (stdout) using (stderr) {
                try {
                    await Task.WhenAll(Pump(process.StandardOutput.BaseStream, stdout), Pump(process.StandardError.BaseStream, stderr), exited).ConfigureAwait(false);
                    File.WriteAllText(Path.Combine(directory, "exit-code.txt"), ExitCode.ToString(), new UTF8Encoding(false));
                } catch (Exception error) { Log.Write("启动日志采集失败：" + error.Message); }
            }
        }
    }
}
