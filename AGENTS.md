---
trio: standard-v3
trio-initialized: 2026-06-13
trio-migrated-v3: 2026-06-14
status: active
desc: macOS 桌面任务看板，文件即真相
---

# tasktab · Agent 操作守则

> **上来先读这份**，再看 [`INDEX.md`](./INDEX.md) 找模块和导航。
>
> **通用三件套协议**见 [`docs/trio-protocol.md`](./docs/trio-protocol.md)（文档维护节奏 / Handoff 写入 / 子项目嵌套 / 记忆三条线边界 / 语言规则 / 跨项目反例）。**本文件只列本项目专属守则**。
>
> **`trio: standard-v3`** = 本项目按当前标准维护三件套。其他 agent / skill 可通过本行 frontmatter 判断是否按标准流程处理本项目。

## 这是什么项目

**TaskBoard**：一个 macOS 桌面任务看板应用，把所有项目的进度统一展示在一个看板里。核心理念是**看板零智能、文件是唯一真相**——每个项目用自己的**三件套**（`AGENTS.md` / `INDEX.md` / `CHANGELOG.md`）维护看板信息，TaskBoard 用 FSEvents 监听这些文件 + registry，文件一变看板秒级刷新，用户永远不需要手动更新看板。

> ⚠️ **架构已于 2026-06-14 收敛**：早期版本每个项目单独维护一份 `PROGRESS.md`（由已退役的 `/kanban` skill 写）。现已废除 PROGRESS.md，所有看板字段改从三件套读（见下"看板数据契约"）。写入端 skill 也从 progress-tracker 换成 `/outkanban`（一步发布）+ `/wrap-up`（收尾增量维护）。

由四个组件构成：
- **数据契约**：三件套约定块 + `registry.yaml`（项目名单）。权威定义见 `同步看板files/02-实现步骤.md` §1.1b / §1.2。
- **`cra` CLI**（`cli/cra.py`，Python + uv）：项目登记/移除/列表。**只写 registry，不生成任何项目文件**。
- **Tauri 2 桌面应用**（`app/`，Rust 后端确定性解析 + 前端渲染）。
- **写入端 skill**（住在 myskills，非本仓）：`/outkanban` 一步发布（自动登记 + 铺看板字段）、`/wrap-up` 收尾顺带维护——系统的"智能"只在写入端。

完整设计见 `同步看板files/` 下三份指导文档（产品意图 / 实现步骤 / skill 规则）。

### 看板数据契约（App 从三件套读，权威：02 §1.1b）

> 下表为**派生速查**，字段 schema 唯一权威 = `同步看板files/02-实现步骤.md` §1.1b；与本表冲突一律以 §1.1b 为准。写入端 outkanban 的字段映射也派生自同一处。

| 字段 | 来源 |
|---|---|
| status（active/paused/done）、desc（卡片一句话） | `AGENTS.md` frontmatter |
| 整体进度 + 阶段列表 | `CHANGELOG.md` `## 项目阶段` checkbox（完成数/总数） |
| next / blocked_by | `INDEX.md` `## 当前接力点 (Handoff)`（`⚠`/`阻塞` 前缀归 blocked_by） |
| updated | `CHANGELOG.md` 最新 `## YYYY-MM-DD` 条目 |
| 简介 / 架构图 | `INDEX.md` `## 项目简介` / `## 架构图`（mermaid） |

## 上手三步

1. 读 [`INDEX.md`](./INDEX.md)，看项目结构、子模块导航、当前接力点和「上手 & 运行」。
2. **动手前先读 `同步看板files/02-实现步骤.md`**：执行计划主文档，含数据契约（**§1.1b 唯一权威**；§1.1 已标废弃）、技术栈、里程碑。M1–M5 已全部完成、架构已收敛到三件套，现处于真机终验 + 打包发布阶段。

## 项目进度同步

> ⚠️ **本项目已于 2026-06-14 退役 `PROJECT_PROGRESS.md`**（James 拍板）。本项目进度只走 `CHANGELOG.md`（强标签记录）+ `INDEX.md` 的「当前接力点 (Handoff)」，**不再维护、不再要求同步 PROJECT_PROGRESS.md，最终回复也无需再说明它是否同步**。
>
> 这条**显式覆盖** `docs/trio-protocol.md` 里"每阶段更新 PROJECT_PROGRESS.md"的通用节奏——通用协议对其他项目仍生效，本项目不适用。老文件留在根目录仅作历史快照，勿再写入。

## 项目专属硬规则

> 通用守则（语言 / 节奏 / Handoff / 子项目 / 记忆边界）见 `docs/trio-protocol.md`。本段只列**本项目专属**约束。

- **数据契约是唯一权威，冲突以 02 文档为准**：三件套看板字段 schema 以 `同步看板files/02-实现步骤.md` **§1.1b** 为唯一权威（§1.1 PROGRESS schema 已废弃）；`registry.yaml` schema 见 §1.2；产品行为以 `01-大白话说明书.md` 为准。写入端 skill（outkanban/wrap-up，住 myskills）内嵌的契约必须与 02 §1.1b 逐字段一致，冲突就改 skill。
- **App 端零智能**：App 只做确定性的三件套块提取与渲染，**绝不调用任何 LLM、不做网络请求**。"智能"只存在于写入端 skill。
- **解析必须防御性**：三件套文件/块缺失、frontmatter 损坏、YAML 解析失败 → 该字段取缺省（status=active、进度 0、列表空），无 status 且无阶段表则卡片降级「⚠ 未接入看板」，**绝不崩溃**，其余项目正常渲染。
- **registry 写入必须原子**：写临时文件后 rename，杜绝半截写入损坏 registry。
- **绝不写用户项目目录内的文件**：App 删除项目只是"从看板移除"（仅改 registry），绝不动项目里任何文件。（写入端 skill outkanban/wrap-up 会改三件套，那是写入端职责，与 App 端这条规则分属两侧。）
- **所有路径处理展开 `~` 并容忍中文路径**。
- **不要随手改 `.env` / 凭证 / `settings.json`**：敏感配置由 James 维护。
- **不要主动删除文件**：废弃 → 移到 `archive/`，不要 `rm`。

<!-- 在此追加本项目工作中沉淀的专属硬规则（活文档，随时更新） -->

## 目录命名约定

按 `02-实现步骤.md` §2 规划的仓库结构：

| 子目录 | 用途 |
|---|---|
| `app/` | Tauri 2 项目（`src/` 前端 + `src-tauri/` Rust 后端） |
| `cli/` | `cra.py` 登记 CLI（Python 3.12 + uv，依赖仅 pyyaml + click） |
| `archive/` | 已退役内容（如 `kanban-retired/`、旧图标）；废弃移这里不 `rm` |
| `scripts/` | `install.sh`（软链 cra、安装 skill、构建装 .app） |
| `docs/` | 详细文档；**本项目根持有** `trio-protocol.md`；指导文档包按 02 §2 最终归档于此 |
| `同步看板files/` | 三份指导文档（产品意图 / 实现步骤 / skill 规则），当前的设计权威来源 |
| `PROJECT_PROGRESS.md` | ⚠️ 已退役（2026-06-14），仅历史快照，勿写入；进度走 CHANGELOG + INDEX Handoff |

> 写入端 skill（outkanban / wrap-up）的源住在 `~/Documents/myskills`，不在本仓——本仓只是它们维护的"消费方"。退役的 kanban skill 归档在 `archive/kanban-retired/`。

## 项目专属"不要做的事"

> 通用反例见 `docs/trio-protocol.md` §9。本段只列**本项目专属**反例。

- ❌ 在 App 端引入任何 LLM 调用 / 网络请求
- ❌ 让看板持有独立状态（所有真相落到 registry.yaml + 各项目三件套）
- ❌ 复活 PROGRESS.md / progress-tracker / kanban——已退役，看板字段只从三件套读
- ❌ 非原子地写 registry.yaml
- ❌ 在 App 端写用户项目里的文件（删除项目仅从 registry 移除）
- ❌ 自动提交 secrets / 凭证；替用户做不可逆操作（必须先问）

<!-- 在此追加本项目工作中沉淀的专属反例 -->
