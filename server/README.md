# tasktab-board · 看板镜像服务端（手机查看）

把 Mac App 解析好的看板 `board.json` 镜像到服务器，手机浏览器只读查看。

## 这是什么 / 边界

- **服务端零智能**：不解析三件套。解析 100% 由 Mac App 的 Rust 后端做，App 把 `board.json` 单向 POST 过来，本服务只「收下存好 + 静态托管网页」。
- **极轻**：纯 Go 标准库单二进制（无外部依赖），基于 `scratch` 镜像，常驻内存几 MB —— 贴合火山云 ECS 4GB 硬约束。
- **全公开只读**（按 James 选择）：`/ingest` 不鉴权，网页谁有链接都能看。`TB_PUSH_TOKEN` 留作以后加固用。

## 路由

| 方法 | 路径 | 作用 |
|---|---|---|
| POST | `/ingest` | 接收 App 推来的 board JSON，原子写盘（限 8MB，校验合法 JSON） |
| GET | `/board.json` | 返回当前数据（网页轮询拉取）；还没收到推送时返回空看板 |
| GET | `/` | 静态看板网页（`web/index.html`，移动端只读卡片） |
| GET | `/healthz` | 健康检查 |

## 环境变量

| 变量 | 缺省 | 说明 |
|---|---|---|
| `TB_ADDR` | `:8787` | 监听地址 |
| `TB_DATA` | `./data/board.json` | 数据落盘路径（镜像里挂 `/data` 卷） |
| `TB_WEB` | `./web` | 静态网页目录 |

## App 端配置（推送方）

App 侧推送受环境变量开关，**未设置则功能完全关闭**，App 行为与之前一致：

| 变量 | 说明 |
|---|---|
| `TB_PUSH_URL` | 服务端 `/ingest` 完整地址，如 `https://board.alphaxbot.xyz/ingest` |
| `TB_PUSH_TOKEN` | 可选，带 `Authorization: Bearer <token>`（服务端当前不校验） |

开发期用 `scripts/dev-detached.sh` 起 App 时，在该脚本或 shell 里 export 上述变量即可。

## 本地 / 部署

```bash
# 本地交叉编译 linux/amd64 静态二进制（重活放本地，ECS 不直连 github）
./build.sh

# 连镜像一起构建
./build.sh --image
```

部署到火山云 ECS 的形态（沿用 huoshan-server 现有 Gitea + Traefik）：
1. 本地 `./build.sh` 出二进制 → `docker build` 出镜像（或本地 build 镜像后 `docker save` 传过去）
2. 写 `docker stack yml`（加入 `dokploy-network`）+ Traefik 动态路由 `/etc/dokploy/traefik/dynamic/tasktab-board.yml`，绑二级域名 `board.alphaxbot.xyz`，LE 自动签证书
3. `ufw allow` + 火山云控制台安全组放行（若需直连端口；走 Traefik 则只需 80/443 已开）

> 部署是外向操作，按项目守则需先与 James 确认再执行。
