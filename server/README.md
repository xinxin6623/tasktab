# tasktab-board · 看板镜像服务端（设备间同步 / 手机查看）

服务器**自己从 GitHub 聚合**各项目三件套成 `board.json`，桌面 App 与手机网页**只读**它。
解决「两台设备各自开发，看板状态怎么串联」：文件 push 到 GitHub = 过了看板。

## 架构 / 边界

```
设备 A/B push 三件套 ──→ GitHub 各独立 repo
                              │ 服务器定时 GitHub API 拉三件套 + 最新 commit
                              ▼
                   Go 解析(parse.go，对齐 board.rs 契约) → 聚合 board.json
                              │
                ┌─────────────┼─────────────┐
            手机网页       桌面 App A      桌面 App B
           (只读)       (只读+同步徽章)  (只读+同步徽章)
```

- **解析逻辑两份、必须同源**：`server/parse.go`(本服务) 与 `app/src-tauri/src/board.rs`(桌面 App) 是
  同一套契约（02 §1.1b）的两份实现。**改契约必须同时改两处**，`parse.go` 每个函数标了对应 `board.rs`
  函数名，`parse_test.go` 用同样的样例做一致性断言。为什么要两份：服务器是 Go、不便复用 Rust，
  用契约文档 + 测试 + 函数级注释钉死一致性。
- **极轻**：纯 Go 标准库 + 一个 yaml 依赖，`scratch` 镜像、常驻内存几 MB —— 贴合火山云 ECS 4GB 约束。
- **并发拉取**：各项目 + 每项目的 4 个 API 都并发，整盘聚合耗时 ≈ 最慢单个 repo（实测 7 repo ~3s）。
- **私有 repo 要 token**：`TB_GH_TOKEN`（GitHub PAT），走环境变量，**绝不进 git**。

## 路由

| 方法 | 路径 | 作用 |
|---|---|---|
| GET | `/board.json` | 返回当前聚合数据（桌面 App / 网页轮询拉取） |
| GET | `/` | 静态看板网页（`web/index.html`，移动端只读卡片） |
| GET | `/healthz` | 健康检查 |
| POST | `/ingest` | [兼容] 接收外部推来的 board JSON（聚合模式下一般不用，旧 push 方案遗留） |

## 环境变量

| 变量 | 缺省 | 说明 |
|---|---|---|
| `TB_ADDR` | `:8787` | 监听地址 |
| `TB_DATA` | `./data/board.json` | 数据落盘路径（镜像里挂 `/data` 卷） |
| `TB_WEB` | `./web` | 静态网页目录 |
| `TB_REGISTRY` | （空）| **设了才启用聚合**。`owner/repo@branch:path`（从 GitHub 拉 registry，推荐）或本地路径 |
| `TB_GH_TOKEN` | （空）| GitHub PAT，读私有 repo 必需；未设则只能读公开 repo 且限流极低 |
| `TB_POLL_SEC` | `60` | 聚合轮询间隔秒 |

> `TB_REGISTRY` 未设 → 退回兼容模式（仅 `/ingest`，不聚合）。

## registry 与同步锚点

registry.yaml 每项目需有 `github: owner/repo@branch` 字段（见 02 §1.2）。`cra add` 会探测项目 git remote
自动填充。服务器据它拉 repo；缺该字段的项目在镜像看板降级「未配置 github」。

`TB_REGISTRY` 推荐填 GitHub 坐标（如 `xinxin6623/tasktab@main:cli/registry.yaml`），让 registry 本身也走
单一真相；本地联调可直接填本地 registry 路径。

## App 端配置（只读方）

桌面 App 的同步徽章受环境变量开关，**未设置则远端同步关闭**、App 行为与之前一致（仍本地解析渲染）：

| 变量 | 说明 |
|---|---|
| `TB_BOARD_URL` | 服务器 `board.json` 地址，如 `https://kanban.alphaxbot.xyz/board.json` |

开发期用 `scripts/dev-detached.sh` 起 App，从 `.dev/push.env`（gitignore）读该变量。

## 本地 / 部署

```bash
./build.sh           # 交叉编译 linux/amd64 静态二进制（重活放本地，ECS 不直连 github）
./build.sh --image   # 连镜像一起构建
```

部署沿用 huoshan-server 的 Dokploy + Traefik：本地 build 镜像 → 传 ECS → swarm service（限 64M 内存）
+ Traefik 路由 `deploy/tasktab-kanban.yml` 绑 `kanban.alphaxbot.xyz`，LE 自动签证书。
**新增**：部署时要给 service 注入 `TB_REGISTRY` + `TB_GH_TOKEN` + `TB_POLL_SEC` 环境变量（token 走 secret，不进 yml）。

> 部署是外向操作，按项目守则需先与 James 确认再执行。
