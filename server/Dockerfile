# tasktab-board 镜像 —— 极简静态二进制 + 网页
#
# 中文说明：火山云 ECS「不直连 github.com、内存吃紧」，所以采用
# 「本地交叉编译出 linux 二进制（build.sh）→ 镜像里只 COPY 成品」的形态，
# 镜像内不跑 go build、不拉任何模块。最终镜像基于 scratch，体积/内存都极小。
#
# 本服务零外部依赖（纯标准库），二进制自包含。
FROM scratch

WORKDIR /app
# 由 build.sh 在本地交叉编译产出（CGO_ENABLED=0 GOOS=linux）
COPY tasktab-board /app/tasktab-board
COPY web /app/web

ENV TB_ADDR=":8787"
ENV TB_DATA="/data/board.json"
ENV TB_WEB="/app/web"
VOLUME ["/data"]
EXPOSE 8787

ENTRYPOINT ["/app/tasktab-board"]
