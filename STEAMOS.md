# SteamOS / Steam Deck 使用说明（实验性支持）

黄金树爬塔的原生 DLL（`erdtree_tower_preview_v*.dll`）会在检测到运行于 **Wine / Proton** 环境时自动强制切换为进程内 **DX12** 渲染模式（跳过 Windows 专属的 DirectComposition 透明窗口），从而在 Steam Deck 的 **Gamescope** 合成器下正常渲染悬浮界面。

> **注意**：SteamOS 适配目前为第一阶段（命令行 / 非 Steam 游戏快捷方式启动），不包含 Windows 版图形启动器（`ErdtreeTowerLauncher.exe`）。单人游玩、路线图、构筑面板、图鉴、无缝联机大厅均在游戏内界面完成。

---

## 1. 前置条件

1. 已在 Steam 安装正版《艾尔登法环》（版本 **1.17.2 / `eldenring.exe` 2.7.2.0**），并在 Steam 里至少使用 Proton 启动过一次。
2. 安装 **Linux 原生版 Mod Engine 3（`me3`）**：
   - 在 Steam Deck 切换到 **桌面模式（Desktop Mode）**；
   - 按 [Mod Engine 3 官方文档](https://me3.help) 安装 Linux 版 `me3`（通常安装到 `~/.local/bin/me3` 或系统 `PATH` 中）；
   - 在终端执行 `me3 --version` 确认可用。
3. 将黄金树爬塔整合包解压到任意不含单引号的目录（如 `/home/deck/Games/ErdtreeTower`）。

---

## 2. 终端预检与启动

在整合包目录打开终端（Konsole）：

```bash
chmod +x ./launch-steamos.sh

# 1）预检（不启动游戏，检查游戏目录、ME3、DLL 与资源）
./launch-steamos.sh --check-only

# 2）单人模式启动
./launch-steamos.sh

# 3）无缝联机模式启动（需整合包或游戏目录存在 SeamlessCoop/ersc.dll）
./launch-steamos.sh --mode seamless

# 若想直接设置无缝联机密码：
./launch-steamos.sh --mode seamless --password 你的密码
```

如果游戏装在自定义路径，可显式指定：

```bash
./launch-steamos.sh --game-dir "/run/media/mmcblk0p1/steamapps/common/ELDEN RING/Game"
```

---

## 3. 添加到 Steam 游戏模式（掌机一键启动）

1. 在桌面模式打开 Steam → 左下角 **「添加游戏」→「添加非 Steam 游戏」** → 浏览并选择 `launch-steamos.sh`。
2. 右键新建的快捷方式 → **属性**：
   - 如需无缝联机模式，在 **启动选项** 填入：`--mode seamless`（单人留空即可）。
   - **不要**给 `launch-steamos.sh` 勾选“强制使用特定 Steam Play 兼容性工具”（它本身是 Linux 脚本，会由 `me3` 通过 Steam/Proton 拉起法环）。
3. 切回 **游戏模式（Gaming Mode）** 即可直接用手柄一键启动。

---

## 4. 配置与日志排查

- **配置文件**：首次运行 `./launch-steamos.sh` 时会自动在整合包目录生成 `tower-options.ini`（默认 `renderer=game`、`prompts=pad`），后续可手动修改（例如 `dlc=none`、`language=en-US` 等）。
- **日志文件**：
  - 单人模式：`<整合包目录>/tower-preview.log`
  - 无缝联机模式：`<整合包目录>/seamless-entry/runtime/tower-preview.log`
- **确认已进入 SteamOS / Proton 兼容路径**：日志开头会出现：
  - `RUNTIME platform=wine/proton version=...`
  - `DX12 hooks installed`
- 若在 SteamOS 实机遇到异常，请保留 `tower-preview.log` 到 Issue 反馈。
