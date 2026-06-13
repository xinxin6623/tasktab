---
trio: standard-v2
trio-initialized: 2026-06-13
---

# tasktab · Agent 操作守则

> **上来先读这份**，再看 [`INDEX.md`](./INDEX.md) 找模块和导航。
>
> **通用三件套协议**见 [`docs/trio-protocol.md`](./docs/trio-protocol.md)（文档维护节奏 / Handoff 写入 / 子项目嵌套 / 记忆三条线边界 / 语言规则 / 跨项目反例）。**本文件只列本项目专属守则**。
>
> **`trio: standard-v2`** = 本项目按当前标准维护三件套。其他 agent / skill 可通过本行 frontmatter 判断是否按标准流程处理本项目。

## 这是什么项目

**TaskBoard**：一个 macOS 桌面任务看板应用，把所有项目的进度统一展示在一个看板里。核心理念是**看板零智能、文件是唯一真相**——每个项目自己维护一份 `PROGRESS.md`，TaskBoard 用 FSEvents 监听这些文件，文件一变看板秒级刷新，用户永远不需要手动更新看板。

由四个组件构成：**数据契约**（`PROGRESS.md` + `registry.yaml`）、**`cra` CLI**（Python，项目登记/移除/生成模板）、**Tauri 2 桌面应用**（Rust 后端读文件 + 前端渲染）、**`progress-tracker` skill**（让 Claude Code 干活时自动写 PROGRESS.md，整个系统的"智能"只在这一处）。

完整设计见 `同步看板files/` 下三份指导文档（产品意图 / 实现步骤 / skill 规则）。

## 上手三步

1. 读 [`INDEX.md`](./INDEX.md)，看项目结构和子模块导航。
2. 读 [`PROJECT_PROGRESS.md`](./PROJECT_PROGRESS.md)，看非工程视角的当前阶段、已完成、下一步和风险。
3. **动手前先读 `同步看板files/02-实现步骤.md`**：它是执行计划主文档，含数据契约（§1.1/1.2）、技术栈、M1–M5 里程碑与验收标准。按里程碑顺序实现，每个里程碑跑完验收项再进下一个。

## 项目进度同步

[`PROJECT_PROGRESS.md`](./PROJECT_PROGRESS.md) 是给不懂代码工程的人看的项目进度页，和 `CHANGELOG.md` 分工不同：

- `PROJECT_PROGRESS.md` 写"现在到哪了 / 做完什么 / 下一步是什么 / 用户怎么验收"，用普通中文，不堆代码术语。
- `CHANGELOG.md` 写给未来 agent 检索的强标签记录，保持短、结构化。
- 每完成一个里程碑（M1–M5）或可验收小任务，必须同步 `PROJECT_PROGRESS.md` 的"已完成 / 下一步 / 验收方式 / 风险变化"。
- 阶段同步说明要解释"这一步对产品进度意味着什么"，不要只列文件名、命令或内部实现。
- 最终回复用户时要说明 `PROJECT_PROGRESS.md` 是否已同步；如果没同步，必须说明原因。

## 项目专属硬规则

> 通用守则（语言 / 节奏 / Handoff / 子项目 / 记忆边界）见 `docs/trio-protocol.md`。本段只列**本项目专属**约束。

- **数据契约是唯一权威，冲突以 02 文档为准**：`PROGRESS.md` / `registry.yaml` 的 schema 以 `同步看板files/02-实现步骤.md` §1.1/1.2 为唯一权威；产品行为以 `01-大白话说明书.md` 为准；skill 细节以 `03-SKILL创建规则.md` 为准。skill 内嵌 schema 必须与 02 §1.1 逐字段一致，冲突就改 skill。
- **App 端零智能**：App 只做确定性解析与渲染，**绝不调用任何 LLM**。"智能"只存在于写入端（progress-tracker skill）。
- **解析必须防御性**：frontmatter 缺失/损坏/YAML 解析失败时该项目卡片降级显示「⚠ 格式异常」，**绝不崩溃**，其余项目正常渲染。
- **registry 写入必须原子**：写临时文件后 rename，杜绝半截写入损坏 registry。
- **不写项目目录内的文件**（除模板生成 PROGRESS.md 外）：TaskBoard 删除项目只是"从看板移除"，绝不动用户项目里的任何文件。
- **所有路径处理展开 `~` 并容忍中文路径**；不做任何网络请求。
- **不要随手改 `.env` / 凭证 / `settings.json`**：敏感配置由 James 维护。
- **不要主动删除文件**：废弃 → 移到 `archive/`，不要 `rm`。

<!-- 在此追加本项目工作中沉淀的专属硬规则（活文档，随时更新） -->

## 目录命名约定

按 `02-实现步骤.md` §2 规划的仓库结构：

| 子目录 | 用途 |
|---|---|
| `app/` | Tauri 2 项目（`src/` 前端 + `src-tauri/` Rust 后端） |
| `cli/` | `cra.py` 登记 CLI（Python 3.12 + uv，依赖仅 pyyaml + click） |
| `skill/` | `progress-tracker/SKILL.md`（按文档 03 生成，M5 阶段） |
| `scripts/` | `install.sh`（软链 cra、安装 skill、构建装 .app） |
| `docs/` | 详细文档；**本项目根持有** `trio-protocol.md`；指导文档包按 02 §2 最终归档于此 |
| `同步看板files/` | 三份指导文档（产品意图 / 实现步骤 / skill 规则），当前的设计权威来源 |
| `PROJECT_PROGRESS.md` | 给非工程读者看的项目阶段进度和下一步 |

## 项目专属"不要做的事"

> 通用反例见 `docs/trio-protocol.md` §9。本段只列**本项目专属**反例。

- ❌ 在 App 端引入任何 LLM 调用 / 网络请求
- ❌ 让看板持有独立状态（所有真相落到 registry.yaml + 各 PROGRESS.md）
- ❌ 非原子地写 registry.yaml
- ❌ 删除或改写用户项目里的文件（删除项目仅从 registry 移除）
- ❌ 偏离里程碑顺序跳跃实现（M1→M5 顺序，验收通过再进下一个）
- ❌ 自动提交 secrets / 凭证；替用户做不可逆操作（必须先问）

<!-- 在此追加本项目工作中沉淀的专属反例 -->
