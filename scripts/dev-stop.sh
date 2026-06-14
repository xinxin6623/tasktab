#!/usr/bin/env bash
# 停止由 dev-detached.sh 启动的独立 TaskBoard dev 进程。
#
# tauri dev 会拉起一串子进程（vite、cargo、真正的 App 二进制），直接 kill 父 PID
# 可能留孤儿，所以这里杀整个进程组 + 兜底按名清理残留。
#
# 用法：./scripts/dev-stop.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUN_DIR="$REPO_ROOT/.dev"
PID_FILE="$RUN_DIR/tauri-dev.pid"

log()  { printf '\033[0;36m▶\033[0m %s\n' "$*"; }
ok()   { printf '\033[0;32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[0;33m⚠\033[0m %s\n' "$*"; }

killed=0

if [ -f "$PID_FILE" ]; then
  PID="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [ -n "${PID:-}" ] && kill -0 "${PID}" 2>/dev/null; then
    log "停止 dev 进程组（PID ${PID}）"
    # 杀进程组（负号 = 整组），失败再退回杀单进程
    kill -TERM -- "-${PID}" 2>/dev/null || kill -TERM "${PID}" 2>/dev/null || true
    sleep 1
    kill -9 -- "-${PID}" 2>/dev/null || kill -9 "${PID}" 2>/dev/null || true
    killed=1
  fi
  rm -f "$PID_FILE"
fi

# 兜底：清理可能残留的 TaskBoard / tauri dev 相关进程（不误伤 VSCode）
# 注意：dev 模式二进制名是小写 taskboard（见 tauri 日志 target/debug/taskboard）
for pat in "target/debug/taskboard" "tauri dev"; do
  pids=$(pgrep -f "${pat}" 2>/dev/null | grep -v "$$" || true)
  if [ -n "$pids" ]; then
    log "清理残留：${pat}"
    echo "$pids" | xargs kill -9 2>/dev/null || true
    killed=1
  fi
done

# 兜底：dev 用的 vite 端口 1420 若还被占（tauri 拉起的 vite 子进程可能漏网），按端口清
port_pids=$(lsof -ti :1420 2>/dev/null || true)
if [ -n "$port_pids" ]; then
  log "清理仍占用 1420 端口的 vite 进程"
  echo "$port_pids" | xargs kill -9 2>/dev/null || true
  killed=1
fi

if [ "$killed" -eq 1 ]; then
  ok "已停止 TaskBoard dev"
else
  warn "没有发现正在运行的 TaskBoard dev"
fi
