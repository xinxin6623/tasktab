---
name: kanban
description: 维护当前项目根目录的 PROGRESS.md，让 TaskBoard 看板实时反映真实进度。整个 TaskBoard 系统的"智能"只在这一处——写入端，App 端是纯读取的哑终端。EVERY time you 完成一个任务、结束一段工作、到达里程碑、遇到阻塞、提交或开 PR 前完成一块工作、修好用户在追踪的 bug，或用户说"收尾""更新进度""记录一下""这个阶段完成了""搞定了""wrap up""done for today"，或输入 /kanban 时触发。在含 PROGRESS.md 的仓库里开工时也用它先读当前状态。只要项目根有 PROGRESS.md，就默认需要进度追踪，即使用户没提也主动更新（Claude 有 undertrigger 倾向，宁可偏积极也不要漏触发）。不负责：给没有 PROGRESS.md 的项目擅自创建（提示用 cra add 接入）、改写正文既有内容、动 PROGRESS.md 之外的任何文件、自己计算整体进度（App 算）。
---

> ⚠️ **已退役（2026-06-14）**：PROGRESS.md 整体废弃，真相收敛到三件套。
> 进度→CHANGELOG `## 项目阶段`、status/desc→AGENTS frontmatter、next→INDEX Handoff。
> 收尾维护由 **/wrap-up**（agentChatBox-ending）接管，看板字段产出用 **/outkanban**。
> 本文件仅留存历史，已从四家 agent unlink、myskills 软链已删，**不要再安装**。

# /kanban — Progress Tracker（已退役）

本项目的进度通过根目录 `PROGRESS.md` 对外暴露，TaskBoard 看板实时读取该文件。
你的职责：让 frontmatter 始终反映真实进度。

## 何时更新

- 任务/会话收尾时（最重要，默认动作，不需要用户提醒）
- 完成 `next` 列表中的某一项时
- 当前阶段完成、进入下一阶段时
- 遇到阻塞时（写入 `blocked_by`）

## frontmatter schema（权威定义，不得增删字段语义）

```yaml
---
project: <kebab-case id, 与 registry 对应, 不要修改>
desc: <项目一句话描述, 卡片展示用; 可选, ≤30 字, 缺省按空串处理>
status: active | paused | done
stages: [<项目全程的阶段规划, 有序列表>]
current_stage: <1-based 整数, 指向 stages 中的当前阶段>
stage_progress: <0-100, 当前阶段内进度估计; 可选, 缺省按 0 处理>
next: [<接下来要做的事, 有序, 保持 2-5 条>]
blocked_by: [<阻塞描述列表; 可选, 无阻塞则空列表>]
updated: <最后更新的 ISO 日期>
---
```

整体进度由 App 按 `(current_stage - 1 + stage_progress/100) / len(stages)` 计算，status 为 `done` 时强制 100%。你只需写好上述字段，不要自己算总进度。

## 更新规则

1. **先读后写**：更新前完整读取 PROGRESS.md，基于现状增量修改，严禁凭记忆重写整个文件
2. **诚实估计** `stage_progress`：依据本次实际完成的工作量，宁可保守；不确定时与用户确认
3. **维护 `next`**：移除已完成项，补充新明确的后续工作，始终保持 2–5 条、按优先级排序
4. **阶段推进需确认**：将 `current_stage` +1 或把 status 改为 done 之前，先向用户口头确认（"X 阶段算完成了吗？"）；确认进阶后 `stage_progress` 归 0
5. **同步追加正文**：每次更新 frontmatter 时，在 `## 阶段记录` 下追加一行带日期的简短记录（一两句，中文），说明做了什么
6. **只许追加正文、修改 frontmatter**；严禁删除或改写既有正文内容；严禁修改 `project` 字段；严禁触碰 PROGRESS.md 之外的任何文件
7. 文件不存在或 frontmatter 损坏时，告知用户并询问是否运行 `cra add .` 重新初始化，不要擅自重建

## 详情页三件套块（让看板详情页好看）

TaskBoard **详情页**还会读项目根 `INDEX.md` / `CHANGELOG.md` 的三个约定块（卡片首页只看 PROGRESS.md）。这三块由**项目自己的三件套维护流程**负责，不是 kanban skill 的强制职责；但当你在维护三件套时（如收尾同步 CHANGELOG），应顺带保持它们最新：

- `INDEX.md` 的 `## 项目简介`：一段 50-70 字项目简介（详情页阶段表上方显示）。
- `INDEX.md` 的 `## 架构图`：一个 ```mermaid 代码块（详情页渲染成图）。
- `CHANGELOG.md` 的 `## 项目阶段`：checkbox 列表 `- [x]/[ ] 阶段名 — 描述`（详情页阶段分块列表，标完成状态）。

缺任一块时详情页对应区域不显示（不报错）。schema 细节以 TaskBoard 项目 `02 §1.1b` 为准。
