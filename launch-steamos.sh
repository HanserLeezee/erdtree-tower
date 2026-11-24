#!/usr/bin/env bash
set -euo pipefail

# SteamOS / Steam Deck / Linux Proton launcher for Erdtree Tower.
# Uses native Linux ME3 (Mod Engine 3) to load Erdtree Tower into Elden Ring under Proton.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODE="solo"
GAME_DIR="${ERDTREE_GAME_DIR:-}"
ME3_BIN="${ME3_BIN:-}"
CHECK_ONLY=0
SEAMLESS_PASSWORD=""

usage() {
  cat <<'EOF'
用法 / Usage:
  ./launch-steamos.sh [选项]

选项 / Options:
  --mode <solo|seamless>   启动模式（默认 solo；seamless 为无缝联机模式）
  --game-dir <path>        指定 Elden Ring 的 Game 目录（含 eldenring.exe）
  --me3 <path>             指定 Linux 版 me3 可执行文件路径
  --password <text>        无缝联机密码（仅 --mode seamless 时可选写入 ersc_settings.ini）
  --check-only             仅校验环境、资源与配置，不启动游戏
  -h, --help               显示帮助
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      MODE="${2:-}"
      shift 2
      ;;
    --game-dir)
      GAME_DIR="${2:-}"
      shift 2
      ;;
    --me3)
      ME3_BIN="${2:-}"
      shift 2
      ;;
    --password)
      SEAMLESS_PASSWORD="${2:-}"
      shift 2
      ;;
    --check-only)
      CHECK_ONLY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "未知参数：$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$MODE" != "solo" && "$MODE" != "seamless" ]]; then
  echo "无效模式：$MODE（仅支持 solo 或 seamless）" >&2
  exit 2
fi

to_wine_path() {
  local p="$1"
  printf 'Z:%s' "${p//\//\\}"
}

find_package_root() {
  local cand
  for cand in "$SCRIPT_DIR" "$SCRIPT_DIR/release-preview"; do
    if [[ -f "$cand/preview.me3" && -f "$cand/scene-assets/regulation.bin" ]]; then
      printf '%s' "$cand"
      return 0
    fi
  done
  return 1
}

find_game_dir() {
  if [[ -n "$GAME_DIR" ]]; then
    if [[ -f "$GAME_DIR/eldenring.exe" ]]; then
      printf '%s' "$GAME_DIR"
      return 0
    fi
    return 1
  fi

  local candidates=(
    "$HOME/.local/share/Steam/steamapps/common/ELDEN RING/Game"
    "$HOME/.steam/steam/steamapps/common/ELDEN RING/Game"
    "$HOME/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common/ELDEN RING/Game"
  )
  # Steam Deck microSD / mounted drives
  local mount_dir
  for mount_dir in /run/media/*/* /run/media/*; do
    if [[ -d "$mount_dir/steamapps/common/ELDEN RING/Game" ]]; then
      candidates+=("$mount_dir/steamapps/common/ELDEN RING/Game")
    fi
  done

  local dir
  for dir in "${candidates[@]}"; do
    if [[ -f "$dir/eldenring.exe" ]]; then
      printf '%s' "$dir"
      return 0
    fi
  done
  return 1
}

find_me3() {
  if [[ -n "$ME3_BIN" ]]; then
    [[ -x "$ME3_BIN" ]] && { printf '%s' "$ME3_BIN"; return 0; }
    return 1
  fi
  if command -v me3 >/dev/null 2>&1; then
    command -v me3
    return 0
  fi
  local cand
  for cand in "$HOME/.local/bin/me3" "$HOME/.cargo/bin/me3" "/usr/local/bin/me3" "/usr/bin/me3"; do
    if [[ -x "$cand" ]]; then
      printf '%s' "$cand"
      return 0
    fi
  done
  return 1
}

PACKAGE_ROOT="$(find_package_root)" || {
  echo "找不到完整的黄金树爬塔整合包（缺少 preview.me3 或 scene-assets/regulation.bin）。" >&2
  exit 1
}

DLL_NAME="$(sed -nE "s/.*['\"](erdtree_tower_preview_v[0-9]+\.dll)['\"].*/\1/p" "$PACKAGE_ROOT/preview.me3" | head -n 1)"
if [[ -z "$DLL_NAME" || ! -f "$PACKAGE_ROOT/$DLL_NAME" ]]; then
  echo "找不到版本化核心 DLL（检查 preview.me3 与目录中的 erdtree_tower_preview_v*.dll）。" >&2
  exit 1
fi
DLL_PATH="$PACKAGE_ROOT/$DLL_NAME"
ASSETS_PATH="$PACKAGE_ROOT/scene-assets"

GAME_DIR="$(find_game_dir)" || {
  echo "未自动找到艾尔登法环 Game 目录，请通过 --game-dir 指定包含 eldenring.exe 的路径。" >&2
  exit 1
}

ME3_BIN="$(find_me3)" || {
  echo "未找到 Linux 版 me3（Mod Engine 3）。请先安装 me3（见 STEAMOS.md），或通过 --me3 指定路径。" >&2
  exit 1
}

if [[ "$PACKAGE_ROOT" == *"'"* ]]; then
  echo "安装路径包含单引号，ME3 配置文件不支持：$PACKAGE_ROOT" >&2
  exit 1
fi

# 首次在 SteamOS/Steam Deck 启动且尚无配置文件时，写入默认 game(DX12) + 手柄提示（不覆盖玩家已有配置）
OPTIONS_FILE="$PACKAGE_ROOT/tower-options.ini"
if [[ ! -f "$OPTIONS_FILE" ]]; then
  cat > "$OPTIONS_FILE" <<'EOF'
renderer=game
prompts=pad
EOF
  echo "已初始化默认 SteamOS 配置：$OPTIONS_FILE（renderer=game, prompts=pad）"
fi

if [[ "$MODE" == "solo" ]]; then
  PROFILE_PATH="$PACKAGE_ROOT/preview.steamos.me3"
  LOG_DIR_LINUX="$PACKAGE_ROOT"
  cat > "$PROFILE_PATH" <<EOF
profileVersion = "v1"
savefile = "ER_TOWER_PREVIEW.sl2"
start_online = false

[[supports]]
game = "eldenring"

[[natives]]
path = '$DLL_PATH'

[[packages]]
path = '$ASSETS_PATH'
EOF
  SEAMLESS_FLAG="0"
else
  RUNTIME_DIR="$PACKAGE_ROOT/seamless-entry/runtime"
  PRIVATE_COOP="$RUNTIME_DIR/SeamlessCoop"
  mkdir -p "$PRIVATE_COOP"

  # 用标准符号链接代替 Windows 的 NTFS Junction
  ln -sfn "$ASSETS_PATH" "$RUNTIME_DIR/scene-assets"

  # 查找无缝联机 ersc.dll（优先整合包内 SeamlessCoop，其次游戏目录 SeamlessCoop）
  ERSC_SRC=""
  for cand in "$PACKAGE_ROOT/SeamlessCoop/ersc.dll" "$GAME_DIR/SeamlessCoop/ersc.dll"; do
    if [[ -f "$cand" ]]; then
      ERSC_SRC="$cand"
      break
    fi
  done
  if [[ -z "$ERSC_SRC" ]]; then
    echo "无缝联机模式缺少 ersc.dll：请确认 SeamlessCoop/ersc.dll 位于整合包目录或游戏 Game 目录。" >&2
    exit 1
  fi
  ERSC_DIR="$(dirname "$ERSC_SRC")"
  cp -u "$ERSC_SRC" "$PRIVATE_COOP/ersc.dll"
  for sub in crashpad locale; do
    if [[ -d "$ERSC_DIR/$sub" ]]; then
      cp -ru "$ERSC_DIR/$sub" "$PRIVATE_COOP/"
    fi
  done

  GATE_DLL=""
  if [[ -f "$PACKAGE_ROOT/SeamlessCoop/Tower.SeamlessGate.dll" ]]; then
    cp -u "$PACKAGE_ROOT/SeamlessCoop/Tower.SeamlessGate.dll" "$PRIVATE_COOP/Tower.SeamlessGate.dll"
    GATE_DLL="$PRIVATE_COOP/Tower.SeamlessGate.dll"
  fi

  SETTINGS_DST="$PRIVATE_COOP/ersc_settings.ini"
  if [[ ! -f "$SETTINGS_DST" ]]; then
    for cand in "$PACKAGE_ROOT/seamless-preset.ini" "$ERSC_DIR/ersc_settings.ini"; do
      if [[ -f "$cand" ]]; then
        cp "$cand" "$SETTINGS_DST"
        break
      fi
    done
  fi
  if [[ -f "$SETTINGS_DST" ]]; then
    # 强制开启爬塔所需的骨灰召唤开关
    if grep -qE '^allow_summons[[:space:]]*=' "$SETTINGS_DST"; then
      sed -i -E 's/^allow_summons[[:space:]]*=.*/allow_summons = 1/' "$SETTINGS_DST"
    else
      printf '\nallow_summons = 1\n' >> "$SETTINGS_DST"
    fi
    if [[ -n "$SEAMLESS_PASSWORD" ]]; then
      if grep -qE '^cooppassword[[:space:]]*=' "$SETTINGS_DST"; then
        sed -i -E "s|^cooppassword[[:space:]]*=.*|cooppassword = ${SEAMLESS_PASSWORD}|" "$SETTINGS_DST"
      else
        printf 'cooppassword = %s\n' "$SEAMLESS_PASSWORD" >> "$SETTINGS_DST"
      fi
    fi
  fi

  PROFILE_PATH="$RUNTIME_DIR/seamless.steamos.me3"
  LOG_DIR_LINUX="$RUNTIME_DIR"
  {
    cat <<EOF
profileVersion = "v1"
start_online = false

[[supports]]
game = "eldenring"

[[natives]]
path = '$PRIVATE_COOP/ersc.dll'
EOF
    if [[ -n "$GATE_DLL" ]]; then
      cat <<EOF

[[natives]]
path = '$GATE_DLL'
load_after = [{ id = "ersc.dll", optional = false }]
EOF
    fi
    cat <<EOF

[[natives]]
path = '$DLL_PATH'
load_after = [{ id = "ersc.dll", optional = false }]

[[packages]]
path = '$ASSETS_PATH'
EOF
  } > "$PROFILE_PATH"
  SEAMLESS_FLAG="1"
fi

LOG_DIR_WINE="$(to_wine_path "$LOG_DIR_LINUX")"

echo "=== Erdtree Tower (SteamOS / Linux Proton) ==="
echo "模式       : $MODE"
echo "整合包目录 : $PACKAGE_ROOT"
echo "游戏目录   : $GAME_DIR"
echo "核心 DLL   : $DLL_NAME"
echo "ME3 路径   : $ME3_BIN"
echo "配置文件   : $PROFILE_PATH"
echo "日志目录   : $LOG_DIR_LINUX ($LOG_DIR_WINE)"

if [[ "$CHECK_ONLY" -eq 1 ]]; then
  echo "预检通过（--check-only 未启动游戏）。"
  exit 0
fi

export ERDTREE_TOWER_PREVIEW=1
export ERDTREE_TOWER_SEAMLESS_ENTRY="$SEAMLESS_FLAG"
export ERDTREE_TOWER_SEAMLESS_PROBE=0
export ERDTREE_TOWER_LOG_DIR="$LOG_DIR_WINE"

exec "$ME3_BIN" launch --game eldenring -p "$PROFILE_PATH"
