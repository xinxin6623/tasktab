# tasktab-board 镜像 —— 极简静态二进制 + 网页
#
# 中文说明：火山云 ECS 内存吃紧（4GB），所以采用
# 「本地交叉编译出 linux 二进制（build.sh）→ 镜像里只 COPY 成品」的形态（编译这种重活别在 ECS 上跑），
# 镜像内不跑 go build、不拉任何模块。最终镜像基于 scratch，体积/内存都极小。
#
# 本服务零外部依赖（纯标准库），二进制自包含。
#
# ⚠️ 唯一例外：需要 CA 根证书。本服务走 HTTPS 调 GitHub API 聚合，scratch 镜像不带
# 任何 CA，Go 会报 x509: certificate signed by unknown authority。所以从 alpine 拷一份
# ca-certificates.crt 进来（Go 默认读 /etc/ssl/certs/ca-certificates.crt）。
FROM alpine:3 AS certs
RUN apk add --no-cache ca-certificates

FROM scratch

WORKDIR /app
# CA 根证书（GitHub API HTTPS 验证必需）
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# 由 build.sh 在本地交叉编译产出（CGO_ENABLED=0 GOOS=linux）
COPY tasktab-board /app/tasktab-board
COPY web /app/web

ENV TB_ADDR=":8787"
ENV TB_DATA="/data/board.json"
ENV TB_WEB="/app/web"
VOLUME ["/data"]
EXPOSE 8787

ENTRYPOINT ["/app/tasktab-board"]
