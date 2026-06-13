# progress-tracker Skill 创建规则(For Claude Code)

> 本文档定义 TaskBoard 自动更新机制的核心:一个安装在 `~/.claude/skills/progress-tracker/` 的 Claude Code skill。
> 整个系统的"智能"只存在于这一处——写入端。看板 App 是纯读取的哑终端。
> 执行 M5 里程碑时按本文档生成 skill。下方已给出完整草稿,按规则微调后直接使用。

## 1. Skill 要解决的问题

James 在各项目里用 Claude Code 推进任务。任务收尾时,Claude Code 应当**主动、无需被提醒地**更新该项目的 `PROGRESS.md` frontmatter,使 TaskBoard 看板实时反映真实进度。没有这个 skill,"自动更新"就不成立。

## 2. 创建规则

1. **目录结构**:单文件 skill,`progress-tracker/SKILL.md`,不需要 scripts/references/assets
2. **frontmatter 必填** `name` 和 `description`;description 是唯一触发机制,必须写得"pushy"——明确列出触发场景,宁可偏积极也不要漏触发(Claude 有 undertrigger 倾向)
3. **正文控制在 100 行内**,只写行为规则,不重复 description 中的触发条件
4. **schema 以文档 02 第 1.1 节为唯一权威**,skill 中内嵌的 schema 必须与之逐字段一致;若两边冲突,改 skill
5. **安全边界写死**:只允许修改 frontmatter 结构化字段与追加正文记录,禁止删改既有正文、禁止动 PROGRESS.md 之外的文件

## 3. SKILL.md 完整草稿

````markdown
---
name: progress-tracker
description: Maintain the project's PROGRESS.md so the TaskBoard kanban app stays up to date. Use this skill EVERY time you complete a task, finish a work session, reach a milestone, hit a blocker, or the user says things like 收尾、更新进度、记录一下、这个阶段完成了、wrap up、done for today. Also use it when starting work in a repo that contains a PROGRESS.md, to read current state first. If a PROGRESS.md exists in the project root, assume progress tracking is expected even if the user doesn't mention it.
---

# Progress Tracker

本项目的进度通过根目录 `PROGRESS.md` 对外暴露,TaskBoard 看板实时读取该文件。
你的职责:让 frontmatter 始终反映真实进度。

## 何时更新

- 任务/会话收尾时(最重要,默认动作,不需要用户提醒)
- 完成 `next` 列表中的某一项时
- 当前阶段完成、进入下一阶段时
- 遇到阻塞时(写入 `blocked_by`)

## frontmatter schema(权威定义,不得增删字段语义)

```yaml
---
project: <kebab-case id, 不要修改>
status: active | paused | done
stages: [<有序阶段列表>]
current_stage: <1-based 整数, 指向 stages>
stage_progress: <0-100, 当前阶段内进度估计>
next: [<接下来要做的事, 有序, 保持 2-5 条>]
blocked_by: [<阻塞描述, 无阻塞则空列表>]
updated: <今天的 ISO 日期>
---
```

## 更新规则

1. **先读后写**:更新前完整读取 PROGRESS.md,基于现状增量修改,严禁凭记忆重写整个文件
2. **诚实估计** `stage_progress`:依据本次实际完成的工作量,宁可保守;不确定时与用户确认
3. **维护 `next`**:移除已完成项,补充新明确的后续工作,始终保持 2–5 条、按优先级排序
4. **阶段推进需确认**:将 `current_stage` +1 或把 status 改为 done 之前,先向用户口头确认("X 阶段算完成了吗?")
5. **同步追加正文**:每次更新 frontmatter 时,在 `## 阶段记录` 下追加一行带日期的简短记录(一两句,中文),说明做了什么
6. **只许追加正文、修改 frontmatter**;严禁删除或改写既有正文内容;严禁修改 `project` 字段
7. 文件不存在或 frontmatter 损坏时,告知用户并询问是否运行 `cra add .` 重新初始化,不要擅自重建
````

## 4. 安装与验证

安装(由 `scripts/install.sh` 执行):

```bash
mkdir -p ~/.claude/skills/progress-tracker
cp skill/progress-tracker/SKILL.md ~/.claude/skills/progress-tracker/
```

验证用测试场景(M5 验收时逐条跑):

| # | 场景 | 期望行为 |
|---|---|---|
| 1 | 在含 PROGRESS.md 的项目中完成一个小任务后说"收尾吧" | 主动更新 stage_progress、next、updated,并追加一条阶段记录 |
| 2 | 说"barge-in 状态机这个阶段做完了" | 先确认,确认后 current_stage +1、stage_progress 归 0 |
| 3 | 任务因外部依赖卡住 | blocked_by 写入具体描述 |
| 4 | 在无 PROGRESS.md 的目录里干活 | 不强行创建,提示可用 cra add 接入看板 |
| 5 | 检查更新后的文件 | 既有正文一字未动,仅 frontmatter 变化 + 正文末尾追加 |

若场景 1 不能稳定触发(连续 3 次测试有失败),按 skill-creator 的描述优化方法加强 description 的触发措辞后重测。
