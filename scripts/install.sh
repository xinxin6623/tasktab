#!/usr/bin/env bash
# TaskBoard 安装脚本（M5）
# 做三件事：① 把 cra CLI 软链到 ~/bin/cra；② 安装 progress-tracker skill 到 ~/.claude/skills/；
#          ③ 构建并把 TaskBoard.app 安装到 /Applications。
#
# 用法：
#   ./scripts/install.sh            完整安装（cra + skill + 构建并安装 .app）
#   ./scripts/install.sh --no-app   只装 cra 和 skill，跳过耗时的 .app 构建
#
# 设计：幂等可重跑；只软链/复制，不破坏用户既有非常规配置（检测到冲突先提示）。

set -euo pipefail

# ── 路径解析（容忍从任意目录调用，容忍中文路径）────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CRA_SRC="$REPO_ROOT/cli/cra.py"
SKILL_SRC="$REPO_ROOT/skill/progress-tracker/SKILL.md"
APP_DIR="$REPO_ROOT/app"

BIN_DIR="$HOME/bin"
CRA_LINK="$BIN_DIR/cra"
SKILL_DST_DIR="$HOME/.claude/skills/progress-tracker"
APP_BUNDLE_NAME="TaskBoard.app"
APP_BUILD_OUT="$APP_DIR/src-tauri/target/release/bundle/macos/$APP_BUNDLE_NAME"
APP_INSTALL_DST="/Applications/$APP_BUNDLE_NAME"

INSTALL_APP=1
[ "${1:-}" = "--no-app" ] && INSTALL_APP=0

log()  { printf '\033[0;36m▶\033[0m %s\n' "$*"; }
ok()   { printf '\033[0;32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[0;33m⚠\033[0m %s\n' "$*"; }

# ── ① 软链 cra 到 ~/bin/cra ───────────────────────────────────────────
install_cra() {
  log "安装 cra CLI → $CRA_LINK"
  [ -f "$CRA_SRC" ] || { warn "找不到 $CRA_SRC，跳过 cra"; return; }
  command -v uv >/dev/null 2>&1 || warn "未检测到 uv；cra 依赖 uv 运行（brew install uv）"
  chmod +x "$CRA_SRC"
  mkdir -p "$BIN_DIR"
  # 已是指向同一源的软链 → 幂等跳过；是别的东西 → 提示用户自行处理，不覆盖
  if [ -L "$CRA_LINK" ] && [ "$(readlink "$CRA_LINK")" = "$CRA_SRC" ]; then
    ok "cra 软链已存在且正确"
  elif [ -e "$CRA_LINK" ]; then
    warn "$CRA_LINK 已存在且非本项目软链，未覆盖。请手动处理后重跑。"
  else
    ln -s "$CRA_SRC" "$CRA_LINK"
    ok "已软链 cra"
  fi
  case ":$PATH:" in
    *":$BIN_DIR:"*) : ;;
    *) warn "$BIN_DIR 不在 PATH 中，请加入：export PATH=\"\$HOME/bin:\$PATH\"" ;;
  esac
}

# ── ② 安装 progress-tracker skill ─────────────────────────────────────
install_skill() {
  log "安装 progress-tracker skill → $SKILL_DST_DIR"
  [ -f "$SKILL_SRC" ] || { warn "找不到 $SKILL_SRC，跳过 skill"; return; }
  mkdir -p "$SKILL_DST_DIR"
  cp "$SKILL_SRC" "$SKILL_DST_DIR/SKILL.md"
  ok "已安装 skill"
}

# ── ③ 构建并安装 .app ─────────────────────────────────────────────────
install_app() {
  log "构建 TaskBoard.app（pnpm tauri build，首次较慢）"
  command -v pnpm >/dev/null 2>&1 || { warn "未检测到 pnpm，跳过 .app 构建"; return; }
  (
    cd "$APP_DIR"
    pnpm install
    pnpm tauri build
  )
  if [ -d "$APP_BUILD_OUT" ]; then
    log "安装 .app → $APP_INSTALL_DST"
    rm -rf "$APP_INSTALL_DST"
    cp -R "$APP_BUILD_OUT" "$APP_INSTALL_DST"
    ok "已安装 TaskBoard.app 到 /Applications"
  else
    warn "未找到构建产物 $APP_BUILD_OUT；请检查 tauri build 输出"
  fi
}

main() {
  log "TaskBoard 安装开始（repo: $REPO_ROOT）"
  install_cra
  install_skill
  if [ "$INSTALL_APP" -eq 1 ]; then
    install_app
  else
    warn "--no-app：已跳过 .app 构建与安装"
  fi
  ok "完成。试试：cra list（先 cra add <项目目录>）"
}

main "$@"
