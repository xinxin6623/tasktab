#!/usr/bin/env bash
# TaskBoard 独立开发启动脚本（v1.0 开发期"边用边改"用）
#
# 解决的问题：直接 `pnpm tauri dev` 是当前终端的子进程，关掉 VSCode / 终端，
# tauri dev 进程树被一起杀，App 就没了。本脚本用 nohup 把 tauri dev 脱离终端
# 会话独立跑，VSCode 关了也不影响；改前端代码 Vite 热重载，改 Rust 自动重编。
#
# 用法：
#   ./scripts/dev-detached.sh          独立启动（已在跑则提示，不重复起）
#   ./scripts/dev-detached.sh --logs   启动后顺便跟一会儿日志（Ctrl-C 只退日志，不杀 App）
#
# 配套停止：./scripts/dev-stop.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="$REPO_ROOT/app"
RUN_DIR="$REPO_ROOT/.dev"
PID_FILE="$RUN_DIR/tauri-dev.pid"
LOG_FILE="$RUN_DIR/tauri-dev.log"

log()  { printf '\033[0;36m▶\033[0m %s\n' "$*"; }
ok()   { printf '\033[0;32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[0;33m⚠\033[0m %s\n' "$*"; }

mkdir -p "$RUN_DIR"

# 已在跑？（PID 文件存在且进程活着）→ 不重复起
if [ -f "$PID_FILE" ]; then
  OLD_PID="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [ -n "${OLD_PID:-}" ] && kill -0 "$OLD_PID" 2>/dev/null; then
    warn "TaskBoard dev 已在运行（PID $OLD_PID）。要重启先跑 ./scripts/dev-stop.sh"
    exit 0
  fi
fi

command -v pnpm >/dev/null 2>&1 || { warn "未检测到 pnpm"; exit 1; }

# 设备间同步配置：若存在 .dev/push.env（gitignore，James 自填），source 进来，
# 让 tauri dev 继承 TB_BOARD_URL（App 据此只读拉服务器 board.json 算同步徽章；
# 未配置则远端同步关闭、App 仍本地解析渲染，行为与之前一致）。见 server/README.md。
PUSH_ENV="$RUN_DIR/push.env"
if [ -f "$PUSH_ENV" ]; then
  # shellcheck disable=SC1090
  set -a; . "$PUSH_ENV"; set +a
  [ -n "${TB_BOARD_URL:-}" ] && ok "已启用设备间同步 ← $TB_BOARD_URL"
fi

log "独立启动 TaskBoard dev（脱离当前终端，关 VSCode 不影响）"
# nohup + 重定向 + & 让进程脱离终端会话；disown 进一步从作业表摘除
cd "$APP_DIR"
nohup pnpm tauri dev > "$LOG_FILE" 2>&1 &
NEW_PID="$!"
echo "${NEW_PID}" > "$PID_FILE"

ok "已启动（PID ${NEW_PID}）"
log "日志：$LOG_FILE"
log "首次启动 Rust 要编译，窗口可能要等十几秒到一两分钟才弹出。"
log "停止：./scripts/dev-stop.sh"

if [ "${1:-}" = "--logs" ]; then
  log "跟踪日志（Ctrl-C 只退出日志查看，不会杀掉 App）："
  tail -f "$LOG_FILE"
fi
