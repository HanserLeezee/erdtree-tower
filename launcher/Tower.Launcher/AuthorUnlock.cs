namespace Tower.Launcher
{
    sealed class AuthorUnlock
    {
        long first, last;
        int count;

        public bool Tap(long milliseconds)
        {
            if (count == 0 || milliseconds - last > 600 || milliseconds - first > 2000 || milliseconds < last)
            {
                first = milliseconds;
                count = 0;
            }
            last = milliseconds;
            return ++count >= 5;
        }
    }
}
