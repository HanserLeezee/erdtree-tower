# 黄金树爬塔 / Erdtree Tower

《艾尔登法环》的 Roguelike 爬塔模组。从大赐福出发，在路线图上选路，依次打普通战、精英、事件、商店和 Boss 房；每层战后三选一拿装备、余烬和护符，词条随机生成，一局输了从头再来。支持单人，也可以用自带的 Seamless Co-op 组队一起爬。

- **随机构筑**：武器、法术和祷告带随机词条，有品质分级；余烬分通用和职业专属两类；每幕开始选一份增益与代价绑定的契约。
- **三幕关卡**：每幕的敌人池、精英和 Boss 各不相同；事件房有各种随机事件（宝箱、重练炉、行商等）。
- **难度**：「苦难律法」叠加额外规则，敌人和 Boss 也可能带词条。
- **联机**：启动器里有联机大厅，可以快速加入、开房邀请好友或输入房间码；队伍里有人没有 DLC 时，全队自动按无 DLC 内容运行。
- **语言**：简体中文、繁體中文、English，在启动器里切换。

## 下载与安装

到 [Releases](https://github.com/tamikip/erdtree-tower/releases) 下载最新的整合包，完整解压到一个独立文件夹（不要解压进游戏目录），然后运行 `ErdtreeTowerLauncher.exe`，选好含 `eldenring.exe` 的 Game 文件夹，点「开始游戏」。

- 需要正版游戏，并且游戏版本要和整合包适配；没有 DLC 也能玩。
- 整合包自带爬塔模组、Seamless Co-op、ME3 和 YAFSML，不用另装无缝联机，也不会改动游戏目录里的原文件。
- 首次启动可能要从本机游戏里提取部分原版资源，会多等几分钟，不要连续点「开始游戏」。
- 详细说明（存档位置、密码、日志、从旧版升级）见 [安装教程](distribution/安装教程.md)；游戏里遇到问题，可以先看启动器「问题修复」页。

## 更新记录

每次改动的说明见 [CHANGELOG.md](CHANGELOG.md)，正式版本的公告是仓库根目录的 `PATCH_NOTES_*.md`。

## 开发

| 目录 | 内容 |
| --- | --- |
| `src/` | 核心规则（Rust）：路线、奖励、词条、存档、联机协议 |
| `native/` | 注入游戏的 DLL（Rust + hudhook / ImGui 界面） |
| `launcher/` | 启动器（C# .NET Framework 4.8 + WebView2，页面在 `launcher/ui`） |
| `scene-builder/` | 从本机游戏生成 `scene-assets/` 的工具（C#，SoulsFormats） |
| `scene-assets/` | 场景、参数、贴图、字体等运行时资源 |
| `distribution/` | 打包、签名与首次启动资源还原 |
| `localization/` | 英文与繁体中文的翻译表和生成脚本 |
| `tests/` | Python 测试；Rust 测试在各 crate 内，启动器测试在 `launcher/Tower.Update.Tests` |
| `reference/` | 设计记录与问题调研 |

常用检查：`cargo test`（核心）、`python -m unittest discover -s tests`、`dotnet test launcher/Tower.Update.Tests`。完整构建和资源重建需要 Windows 和本机游戏文件，说明见 [GITHUB.md](GITHUB.md) 和 `reference/` 下的相关记录。

## 协议

作者编写的代码与数据以 GPL-3.0-or-later 发布，附加保留署名、标明修改版、不授予名称使用权三条条款，详见 [LICENSE](LICENSE) 与 [ADDITIONAL-TERMS.md](ADDITIONAL-TERMS.md)。游戏数据、其他模组的派生内容和第三方代码不在此协议范围内，来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
