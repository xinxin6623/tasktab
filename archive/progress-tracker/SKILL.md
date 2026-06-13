---
name: progress-tracker
description: Maintain the project's PROGRESS.md so the TaskBoard kanban app stays up to date. Use this skill EVERY time you complete a task, finish a work session, reach a milestone, hit a blocker, finish a chunk of work before committing or opening a PR, fix a bug the user was tracking, or the user says things like 收尾、更新进度、记录一下、这个阶段完成了、搞定了、wrap up、done for today. Also use it when starting work in a repo that contains a PROGRESS.md, to read current state first. If a PROGRESS.md exists in the project root, assume progress tracking is expected and update it proactively even if the user never mentions it.
---

# Progress Tracker

> ⚠️ **已归档（2026-06-13）**：本文件是草稿，已改名为 `kanban`。真实源在本仓
> `skill/kanban/SKILL.md`；`~/Documents/myskills/kanban` 是软链指向它，由 outskill 分发到各 agent。
> 命令 `/kanban`。此处仅留存历史，**不要再编辑或安装本文件**；如需改 skill，改 `skill/kanban/`。

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
