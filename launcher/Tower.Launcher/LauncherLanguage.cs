using System;

namespace Tower.Launcher
{
    internal static class LauncherLanguage
    {
        static string locale = System.Globalization.CultureInfo.CurrentUICulture.Name;
        public static void Select(string choice) => locale = choice;
        public static string Pick(string simplified, string traditional, string english)
        {
            if (locale == "繁體中文" || locale.StartsWith("zh-TW", StringComparison.OrdinalIgnoreCase)
                || locale.StartsWith("zh-HK", StringComparison.OrdinalIgnoreCase) || locale.StartsWith("zh-MO", StringComparison.OrdinalIgnoreCase)
                || locale.StartsWith("zh-Hant", StringComparison.OrdinalIgnoreCase)) return traditional;
            return locale == "简体中文" || locale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? simplified : english;
        }
    }
}
