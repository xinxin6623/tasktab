#!/usr/bin/env bash
# 本地交叉编译 tasktab-board 为 linux/amd64 静态二进制。
# 火山云 ECS 不直连 github、内存紧 —— 重活（编译）放本地，服务器只跑成品（见 huoshan-server skill §6）。
#
# 用法：
#   ./build.sh            # 仅交叉编译出 ./tasktab-board（linux/amd64）
#   ./build.sh --image    # 编译后再 docker build 出镜像 tasktab-board:latest
set -euo pipefail
cd "$(dirname "$0")"

echo "▶ 交叉编译 linux/amd64 静态二进制…"
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -trimpath -ldflags="-s -w" -o tasktab-board .
echo "✅ 产出 ./tasktab-board ($(du -h tasktab-board | cut -f1))"

if [[ "${1:-}" == "--image" ]]; then
  echo "▶ 构建 docker 镜像 tasktab-board:latest…"
  docker build -t tasktab-board:latest .
  echo "✅ 镜像就绪：tasktab-board:latest"
fi
