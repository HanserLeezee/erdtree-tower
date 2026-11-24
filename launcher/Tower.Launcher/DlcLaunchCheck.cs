using System;
using System.IO;

namespace Tower.Launcher
{
    static class DlcLaunchCheck
    {
        public static string? Warning(string gameDir, bool seamless, string? mode, Func<string, bool>? hasArchive = null)
        {
            var noDlc = string.Equals(mode, "none", StringComparison.OrdinalIgnoreCase)
                || string.Equals(mode, "weapons", StringComparison.OrdinalIgnoreCase);
            // 选了没有 DLC：单人与联机都按没有 DLC 运行（联机时队伍里有人没有 DLC，整队都不出 DLC 内容），不需要提示
            if (noDlc) return null;
            hasArchive = hasArchive ?? HasArchive;
            var present = hasArchive(Path.Combine(gameDir, "DLC.bhd")) && hasArchive(Path.Combine(gameDir, "DLC.bdt"));
            if (present) return null;
            return LauncherLanguage.Pick("游戏目录中的黄金树之影 DLC 资源文件（DLC.bhd / DLC.bdt）缺失、为空或无法读取。", "遊戲目錄中的黃金樹之影 DLC 資原始檔（DLC.bhd / DLC.bdt）缺失、為空或無法讀取。", "Shadow of the Erdtree files DLC.bhd / DLC.bdt are missing, empty or unreadable.")
                + LauncherLanguage.Pick("\n\n当前设置为「有 DLC」，会启用 DLC 敌人与物品，缺少所需资源可能导致加载失败或闪退。", "\n\n當前設定為「有 DLC」，會啟用 DLC 敵人與物品，缺少所需資源可能導致載入失敗或閃退。", "\n\nDLC is enabled. Missing resources may cause loading failures or crashes.")
                + LauncherLanguage.Pick("\n没有购买 DLC 的话，请在设置 → 游戏里把 DLC 改为「没有 DLC」；联机时只要队伍里有人没有 DLC，整队都会按没有 DLC 运行。", "\n沒有購買 DLC 的話，請在設定 → 遊戲裡把 DLC 改為「沒有 DLC」；聯機時只要隊伍裡有人沒有 DLC，整隊都會按沒有 DLC 執行。", "\nIf you do not own the DLC, select No DLC under Settings → Game. If any party member lacks DLC, the entire party uses No DLC mode.")
                + LauncherLanguage.Pick("\n本检查仅查看本地文件，不能验证账号是否拥有 DLC，也不能验证资源是否完整。", "\n本檢查僅檢視本地檔案，不能驗證賬號是否擁有 DLC，也不能驗證資源是否完整。", "\nThis check reads local files only and cannot verify ownership or file integrity.")
                + LauncherLanguage.Pick("\n\n确定：仍然启动。\n取消：返回启动器。", "\n\n確定：仍然啟動。\n取消：返回啟動器。", "\n\nOK: launch anyway. Cancel: return to launcher.");
        }

        static bool HasArchive(string path)
        {
            try { return new FileInfo(path).Length > 0; }
            catch (IOException) { return false; }
            catch (UnauthorizedAccessException) { return false; }
        }
    }
}
