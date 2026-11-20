using System;
using System.IO;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;

namespace Tower.Update
{
    public sealed class FallbackSource : IVersionedReleaseSource
    {
        readonly MultiSource primary;
        readonly IReleaseSource? fallback;
        readonly string fallbackName;
        volatile bool preferFallback;
        public string? Chosen { get; private set; }

        public FallbackSource(MultiSource primary, string fallbackName = "阿里云（备用）", IReleaseSource? fallback = null)
        { this.primary = primary; this.fallbackName = fallbackName; this.fallback = fallback; }

        public Task<ReleaseManifest> LatestAsync(CancellationToken cancel) => ReadAsync((s,c) => s.LatestAsync(c), false, cancel);
        public Task<ReleaseManifest> VersionAsync(string tag, CancellationToken cancel) => ReadAsync((s,c) =>
            s is IVersionedReleaseSource versioned ? versioned.VersionAsync(tag,c) : Task.FromException<ReleaseManifest>(new IOException("线路不支持历史版本")), false, cancel);
        public Task<byte[]> RangeAsync(string tag, string asset, long offset, long length, CancellationToken cancel) =>
            ReadAsync((s,c) => s.RangeAsync(tag,asset,offset,length,c), true, cancel);

        static bool NetworkFailure(Exception error) => error is IOException || error is HttpRequestException || error is OperationCanceledException;

        async Task<T> ReadAsync<T>(Func<IReleaseSource,CancellationToken,Task<T>> read, bool download, CancellationToken cancel)
        {
            cancel.ThrowIfCancellationRequested();
            Exception? backupFailure = null;
            if (download && preferFallback && fallback != null)
            {
                try { return await BackupAsync(read,cancel).ConfigureAwait(false); }
                catch (Exception error) when (!cancel.IsCancellationRequested && NetworkFailure(error)) { backupFailure = error; }
            }
            try
            {
                var result = await read(primary,cancel).ConfigureAwait(false);
                preferFallback = false; Chosen = primary.Chosen;
                return result;
            }
            catch (Exception error) when (!cancel.IsCancellationRequested && NetworkFailure(error))
            {
                if (fallback == null) throw;
                if (backupFailure != null) throw new IOException("主线路和阿里云备用节点均下载失败", new AggregateException(error,backupFailure));
                return await BackupAsync(read,cancel).ConfigureAwait(false);
            }
        }

        async Task<T> BackupAsync<T>(Func<IReleaseSource,CancellationToken,Task<T>> read, CancellationToken cancel)
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancel);
            timeout.CancelAfter(TimeSpan.FromSeconds(20));
            try
            {
                var result = await read(fallback!,timeout.Token).ConfigureAwait(false);
                preferFallback = true; Chosen = fallbackName;
                return result;
            }
            catch (OperationCanceledException error) when (!cancel.IsCancellationRequested)
            { throw new IOException("阿里云备用节点下载超时",error); }
        }
    }
}
