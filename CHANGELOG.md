# tasktab · CHANGELOG

> 每次动了什么记一条。详细记录写在各自模块目录下，根目录 CHANGELOG 是**强标签化的检索索引**。
>
> **如本项目下有子项目**（子目录里也有 AGENTS/INDEX/CHANGELOG 三件套）：本 CHANGELOG **只记录跨多个子项目的同时操作**；单一子项目操作记在该子项目自己的 CHANGELOG 里。详见 [`docs/trio-protocol.md`](./docs/trio-protocol.md) §5 子项目嵌套（max-depth = 3）。

## 格式规范（严格）

```
## YYYY-MM-DD #<type> scope:<name> [#<extra-tag>...] - <一句话主题>

- Why: <一句话动机，不复述 what>
- 详见: <path 或 commit hash>
```

**硬约束**：
- 日期必须 ISO 格式 `YYYY-MM-DD`
- 类型标签必须以 `#` 开头，从下面字典选一个为主标签
- 作用域必须 `scope:<name>` 形式，name 用 kebab-case；多模块改动用多个 `scope:`
- Why 一行不超过 80 字符
- **不贴 diff、不复述 what**——那些进 commit 或模块自己的文档

## 类型标签字典

| 标签 | 含义 |
|---|---|
| `#feat` | 新功能 |
| `#fix` | bug 修复 |
| `#refactor` | 重构（无行为变化） |
| `#perf` | 性能优化 |
| `#docs` | 文档变更 |
| `#test` | 测试相关 |
| `#chore` | 构建/依赖/工具链/初始化 |
| `#archive` | 归档/弃用 |
| `#breaking` | 破坏性变更（叠加） |
| `#deprecated` | 标记弃用（叠加） |
| `#wip` | 进行中（叠加） |

## 检索示例

```bash
grep -E "^## .* #feat .* scope:cli" CHANGELOG.md     # cli 模块新功能
grep "#breaking" CHANGELOG.md                         # 所有破坏性变更
grep "^## 2026-06" CHANGELOG.md                       # 2026 年 6 月所有动作
```

---

## 项目阶段

- [x] M1 数据层 + cra CLI — 项目登记/移除/列表
- [x] M2 看板主界面 — 卡片网格 + 进度条 + 防御性降级
- [x] M3 详情页 + 增删按钮 — 阶段列表/架构图/打开文件
- [x] M4 自动刷新 — FSEvents 监听三件套秒级刷新
- [x] M5 自动更新 + 安装脚本 — outkanban/wrap-up 写入端 + install.sh
- [x] 架构收敛 — 废 PROGRESS.md，真相统一到三件套
- [ ] 真机终验 + 打包发布 — .app 构建、签名、装「应用程序」
- [x] 手机查看（看板镜像）— App 推 board.json 到服务器，手机网页只读看板
- [x] 设备间同步 — 服务器从 GitHub 聚合，各端只读 + 同步徽章（取代 App 推送）；线上 2026-06-19 已切聚合模式并验证

## 2026-06-26 #feat scope:cli - cra 自动同步入库镜像 registry，消除两份手动同步缝隙

- Why: 两份 registry（本地真相源 / 入库 server/registry.yaml）此前靠手动同步，
  易漏（llmbs 漏同步、tasktab/bizwiki 分支陈旧都源于此）。cra add/remove 现自动把
  名单精简投影（id/name/github/pinned，剥本机路径）原子写进 server/registry.yaml。
  配套 outkanban 末尾新增「发布上线」push 步骤（push 前确认）。
- 详见: cli/cra.py::sync_server_registry

## 2026-06-26 #fix scope:server #chore - 修 bizwiki registry 分支笔误 + 项目整理

- Why: bizwiki 镜像降级「未接入看板」根因是 registry github 字段指向不存在的分支
  phase5-business-events（仓库只有 main，main 上三件套齐全），服务器拉三件套全 404。
  改回 @main；本地真相源 registry 同步修正。顺带清理散落文件、归档 Logo 设计源稿。
- 详见: ca524d6 / 22302b3

## 2026-06-19 #fix scope:server - 手机网页阶段列表名称空白：用 stage_items 而非 stages

- Why: server/web/index.html 详情页「项目阶段」只渲染出圆点、名称全空。根因取错字段——
  board.json 的 `stages` 是名称字符串数组、`stage_items` 才是带 name/desc/done 的结构化数据，
  网页却按结构化字段访问 `stages`，s.name/s.done 全 undefined。改用 stage_items 并对纯字符串
  数组降级兼容。注：桌面 App（Detail.tsx）走另一套结构化接口、无此 bug——server/web 是
  board.json 的第三份独立消费方，字段陷阱易漏。

## 2026-06-19 #feat scope:server #docs - 线上服务端切到 GitHub 聚合模式（设备间同步收尾）

- Why: 线上 tasktab-board 一直跑旧 push 二进制+零环境变量（仍 [ingest] 兼容模式），设备间同步代码虽合 main 但「线上待切换」。本次完成切换并验证：board.json 出现 generated_at + 7 项目各自 commit SHA、registry_error=null。
- 切换中炸出并修掉三个坑：① scratch 镜像缺 CA 根证书→Go 调 GitHub HTTPS 报 x509，Dockerfile 加 alpine 拷 ca-certificates.crt；② TB_REGISTRY 路径错填 cli/registry.yaml（gitignore 不上 GitHub），实为 server/registry.yaml；③ 旧 push 源（某设备旧版 App）仍在 POST /ingest，切聚合后归零但路由仍在，隐患待禁。
- 实测纠正：国内 ECS api.github.com REST 轻量调用可直连（200/~0.9s），git 大流量才需本地化；同步更正 huoshan-server skill §6。
- 详见: server/README.md、archive/ 无关；环境变量 TB_REGISTRY=xinxin6623/tasktab@main:server/registry.yaml + TB_GH_TOKEN + TB_POLL_SEC=60 已注入 swarm service。

## 2026-06-18 #feat scope:server scope:app scope:cli - 设备间同步：服务器 GitHub 聚合 + App 只读同步徽章

- Why: James 两台设备各自开发，要把看板状态串联。改为 GitHub 仓库做单一真相、服务器聚合、各端只读——「文件 push 到 GitHub = 过了看板」。取代上一版「App 单向推送 board.json」（push.rs 退役到 archive/push-retired/）。
- 服务端（server/）：新增 github.go（GitHub API 拉各 repo 三件套+最新 commit，各 repo/各文件并发，7 repo 聚合 ~3s）+ parse.go（Go 重实现三件套解析，逐函数对齐 board.rs 契约，parse_test.go 用同样本断言一致性）。main.go 加聚合循环（TB_REGISTRY 启用，定时 GitHub→解析→原子写 board.json，含 generated_at）。
- App 端：push.rs 退役，新增 sync.rs（load_local_sync 跑只读 git 拿本地 HEAD/脏/ahead/behind；load_remote_board 只读拉服务器 board.json，受 TB_BOARD_URL）。前端 sync.ts 把本地 HEAD vs 服务器 commit 算成同步徽章（已同步/待推送/待拉取/未提交/分叉），卡片底部显示徽章、顶栏显示服务器最后聚合时间。「App 端零网络」铁律收敛为「仅两个受控只读例外」（见 AGENTS.md）。
- registry：新增 github 字段（owner/repo@branch，02 §1.2），cra add + App 内 add（registry.rs）都探测 git remote 自动填，保持 byte-identical（test_detect_github 守护）。
- 文档：AGENTS.md 加「设备间同步架构」节 + 改零网络铁律/反例；02 §1.2 加 github 字段；server/README.md 重写为聚合模型。
- 验证：四端全绿（Rust 41 测试 / Go 测试+vet / TS 编译 / cra 语法）；本地真实聚合 7 项目全带 commit+进度（私有 repo 也拉到）。**未做**：真机点开桌面 App 看徽章、服务器重新部署注入 TB_REGISTRY/TB_GH_TOKEN。

## 2026-06-17 #feat scope:server scope:app - 手机查看：看板镜像服务端 + App 单向推送

- Why: James 要在手机上看看板；App 推已解析的 board.json 到自有服务器，手机只读网页渲染
- 详见: 新增 server/（纯标准库 Go 单文件 main.go：POST /ingest 原子写 + 静态托管 web/index.html 移动端只读卡片；scratch 镜像 4.89MB）；App 端 push.rs（ureq 单向 POST，受 TB_PUSH_URL 开关，零网络铁律的唯一受控例外）挂在 watcher emit 后 + 启动初始推一次。已部署火山云 ECS：swarm service tasktab-board（限 64M 内存）+ Traefik 路由 server/deploy/tasktab-kanban.yml，公网 https://kanban.alphaxbot.xyz 上线（LE 证书自动签）。真机验证：App 启动实推真实看板（6 项目 4986 字节）。dev-detached.sh 从 .dev/push.env（gitignore）读 TB_PUSH_URL

## 2026-06-14 #chore scope:trio - 迁移到 standard-v3（退役 PROJECT_PROGRESS 升到协议层）

- Why: PROJECT_PROGRESS 退役从单项目改动升格为协议层 standard-v3，本项目作首个 v2→v3 迁移单元落地
- 详见: AGENTS frontmatter trio v2→v3 + trio-migrated-v3；docs/trio-protocol.md 副本升 v3（加 protocol-version:3、删 PP 段，保留 TaskBoard 本地措辞）；协议演进吸收器见 myskills/project-init/references/migrations.md

## 2026-06-14 #feat scope:scripts - 开发期独立启动 dev-detached.sh / dev-stop.sh

- Why: tauri dev 是 VSCode 终端子进程，关 VSCode 连坐杀掉 App；v1.0 边用边改要它常驻
- 详见: scripts/dev-detached.sh（nohup 脱离终端，顶层进程被 launchd 收养，实测父进程=1）+ dev-stop.sh（杀进程组 + 按名/按 1420 端口兜底清 vite 残留）；用法写进 INDEX「上手 & 运行」；.dev/ 已 gitignore

## 2026-06-14 #docs scope:agents scope:index - 退役 PROJECT_PROGRESS.md，斩断同步钩子

- Why: James 拍板退役非工程进度页；进度收敛到 CHANGELOG + INDEX Handoff
- 详见: AGENTS.md「项目进度同步」改退役声明（显式覆盖 trio-protocol 通用节奏，通用协议不动）；INDEX 清全部 PROJECT_PROGRESS 引用 + 顺手清过时 progress-tracker/PROGRESS.md 残留引用；老文件留作历史快照不删

## 2026-06-14 #feat scope:app scope:ui - 接力点概述改纯文本加粗 + 卡片去圆点

- Why: James 要求概述句去掉列表点、内容加粗，desc 也加粗；卡片更清爽
- 详见: board.rs extract_handoff（列表 `- ` 前缀可选、剥首尾 `**`、跳引用行 `>`）；styles.css `.card-desc` 加粗 + `.next` 去圆点（纯前端，不动数据）；三项目 INDEX.md 全迁两段；全量 40 测试过

## 2026-06-14 #feat scope:app scope:contract - Handoff 拆「概述/明细」两段，App 只抓概述

- Why: 接力点单段时长说明和"下一步"混在一起，App 标签页会把整段啰嗦内容抓进 next
- 详见: board.rs extract_handoff_overview（只解析 ### 概述，明细忽略，无子标题向后兼容）；契约同步 02 §1.1b + trio-protocol §3 + 写入端 skill（handoff-protocol/outkanban/INDEX&trio 模板）

## 2026-06-14 #docs scope:agents - 按新架构重写项目宪法 AGENTS.md

- Why: 宪法正文仍是旧版（讲 PROGRESS.md/progress-tracker），会误导后续 agent
- 详见: AGENTS.md（这是什么项目/硬规则/目录约定/反例全部对齐三件套架构 + 看板数据契约表）

## 2026-06-14 #breaking scope:contract scope:app - 废除 PROGRESS.md，真相收敛三件套

- Why: PROGRESS.md 字段全可摊进三件套，双轨冗余；收敛后 /wrap-up 单一写入口
- 详见: 02 §1.1(废弃)/§1.1b(唯一契约)；status←AGENTS、进度←CHANGELOG 阶段表、next←INDEX Handoff、日期←CHANGELOG

## 2026-06-14 #refactor scope:app - App 全面改读三件套 + watcher 监听三件套

- Why: 配合 PROGRESS 退役，board.rs 解析源切到 AGENTS/INDEX/CHANGELOG，watcher 同步
- 详见: board.rs(parse_agents_meta/compute_progress_from_stages/extract_handoff/extract_changelog_date)、watcher.rs、cra.py/registry.rs 去 PROGRESS 模板；37 测试全过

## 2026-06-14 #archive scope:kanban - /kanban skill 退役，四家 unlink + 归档

- Why: 进度维护由 /wrap-up 接管、看板字段产出由 /outkanban 负责，kanban 职责消解
- 详见: archive/kanban-retired/；myskills 软链删除；wrap-up/outkanban/project-init 适配

## 2026-06-14 #fix scope:app - today_iso 改用本地时区，修跨午夜与 cra 差一天

- Why: App UTC 算日期、cra.py 本地时区，跨午夜窗口 registry 字节不一致
- 详见: registry.rs today_iso 改 spawn `date +%F`，失败回退 UTC

## 2026-06-13 #feat scope:app scope:contract - 详情页改读三件套块 + 卡片加 desc

- Why: 详情页从「PROGRESS 单文件」升级为读三件套结构化信息（简介/阶段表/架构图）
- 详见: board.rs(extract_section/mermaid/stage_list+StageItem)、Detail.tsx、Mermaid.tsx、02 §1.1/1.1b

## 2026-06-13 #feat scope:contract - PROGRESS.md frontmatter 新增可选 desc 字段

- Why: 卡片首页需要一句话项目描述（替换原阶段行），20-30 字
- 详见: 02 §1.1、board.rs/types.ts/Card.tsx/cra.py/registry.rs 六处逐字段同步

## 2026-06-13 #chore scope:repo - 首次 push 到 GitHub 远端

- Why: 代码与文档推到 github.com/xinxin6623/tasktab（PUBLIC），建立远端备份与协作入口
- 详见: origin/main 9 commits；push 前已扫无 .env/密钥；main 跟踪 origin/main

## 2026-06-13 #chore scope:app - 图标改为 Pillow 矢量重画青绿玻璃版

- Why: 以 Logo 为参考用代码重画，比裁 AI 原图更锐利无伪影；旧裁切版备份
- 详见: icons/generate_icon.py（青绿渐变玻璃方+清单+对勾）；裁切版 → archive/icons/icon_source_cropped_v2.png

## 2026-06-13 #chore scope:app - 图标换成青绿玻璃清单版（裁切版，已被矢量版取代）

- Why: James 提供 macOS 风格 Logo，替换上一版彩虹打勾；旧脚本归档不删
- 详见: icons/crop_icon.py（裁玻璃方+圆角抠图去文字水印）；原图存项目根；旧版 → archive/generate_icon_rainbow_v1.py

## 2026-06-13 #chore scope:app - 替换应用图标为正式版（彩虹打勾，已被青绿版取代）

- Why: 原 icon.png 是 104 字节珊瑚占位；按参考图做正式品牌图标
- 详见: archive/generate_icon_rainbow_v1.py（Pillow 画 1024 源图）→ tauri icon 生成全套 + tauri.conf 列 icns/ico

## 2026-06-13 #fix scope:app #test - 修复 board 测试 flaky（HOME 全局污染）

- Why: test_expand_tilde 用 set_var 改进程级 HOME，并发跑污染他测，CI 会随机红
- 详见: board.rs 抽出 expand_tilde_with 纯函数注入 home，单测不再碰全局 env（34 passed，并发连跑 5 次稳定）

## 2026-06-13 #chore scope:repo - git 初始化并按里程碑分批提交

- Why: 项目代码完成，按 02 §5 用 git 管理并按里程碑切提交，建立可追溯历史
- 详见: git log（M0 三件套 / M1 cli / M2-M4 app / M5 skill+install 共 4 commit）

## 2026-06-13 #chore scope:install - M5 install.sh 安装脚本完成

- Why: 一键软链 cra、装 progress-tracker skill、构建并安装 TaskBoard.app
- 详见: scripts/install.sh（幂等可重跑，--no-app 跳过构建；cra 端到端烟雾测试通过）

## 2026-06-13 #feat scope:app - M4 FSEvents 文件监听自动刷新完成

- Why: 产品核心卖点——进度文件一变看板秒刷，用户永不手动更新看板
- 详见: app/src-tauri/src/watcher.rs（notify 监听父目录+500ms 去抖，cargo test 34 passed）

## 2026-06-13 #feat scope:app - M3 详情页 + 动作按钮 + App 内增删完成

- Why: 让看板可交互——查单项目时间线、开文件/编辑器、界面内登记与移除项目
- 详见: app/（Rust 增删与 cra.py 字节一致，cargo test 28 passed）

## 2026-06-13 #feat scope:app - M2 Tauri 骨架 + 只读仪表盘完成

- Why: 用户第一屏，把 registry 与各 PROGRESS.md 首次以看板卡片网格呈现
- 详见: app/（Tauri2+React/TS，load_board 防御性解析，cargo test 16 passed）

## 2026-06-13 #feat scope:cli - M1 cra 命令行工具完成

- Why: 看板的数据地基，负责项目登记/移除/列表与 PROGRESS.md 模板生成
- 详见: cli/cra.py（add/remove/list + 原子 registry 写入，4 条验收全过）

## 2026-06-13 #feat scope:skill - M5 progress-tracker skill 草稿落地

- Why: 系统唯一的"智能"写入端，让 Claude Code 干活时自动更新 PROGRESS.md
- 详见: skill/progress-tracker/SKILL.md（schema 已对齐 02 §1.1，修正 stage_progress 可选语义）

## 2026-06-13 #chore scope:init - 项目初始化

- Why: 新 TaskBoard 项目需要 agent 入口、人类导航、演绎记录三件套，便于协作和未来检索
- 详见: AGENTS.md / INDEX.md / PROJECT_PROGRESS.md / 本文件

<!-- 新条目加在这里上方，保持最新在最上 -->
