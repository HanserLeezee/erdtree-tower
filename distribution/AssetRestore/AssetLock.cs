using System;
using System.IO;
using System.Diagnostics;
using System.Threading;

namespace Tower.Assets
{
    internal static class AssetLock
    {
        public static FileStream Acquire(string path, TimeSpan timeout, Action waiting)
        {
            var clock=Stopwatch.StartNew();
            var reportAt=TimeSpan.Zero;
            while(true) {
                try { return new FileStream(path, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None); }
                catch(IOException error) when((error.HResult & 0xffff)==32 || (error.HResult & 0xffff)==33) {
                    if(clock.Elapsed>=timeout) throw new TimeoutException("其他资源生成任务仍未结束，请查看正在运行的任务日志后重试；当前任务尚未生成资源。",error);
                    if(clock.Elapsed>=reportAt) { waiting(); reportAt=clock.Elapsed+TimeSpan.FromSeconds(5); }
                    Thread.Sleep(100);
                }
            }
        }
    }
}
