# 三件套通用协议（trio-protocol · standard-v2）

> 本文件由 `/project-init` 创建。**所有遵循 `trio: standard-v2` 的项目共享这套协议**，子项目通过相对路径 `../docs/trio-protocol.md` 引用最近祖先的副本。
>
> 项目自己的专属守则（项目定位 / 专属硬规则 / 目录约定 / 不要做的事）写在项目根的 `AGENTS.md`；本文件只放跨项目通用的部分。

## 1. 三份文档 + 进度页的分工与更新节奏

三份文档和进度页**不是平权的**。它们的更新频率和触发条件不一样，按错节奏会污染 signal：

- **AGENTS.md（活文档）**：**随时更新**。工作中发现新的项目惯例 / 踩坑 / 决策立刻加进来。Agent 边干边写，不要等收尾。
- **INDEX.md（结构快照 + 接力位）**：**阶段性更新**。两类内容：① 顶层目录 / 子模块结构变化时同步导航；② 顶部 `## 当前接力点 (Handoff)` 段承载本项目"下一步动作"。**不要每改一个文件就动**。
- **CHANGELOG.md（演绎记录）**：**阶段性更新**。每个任务 / 功能 / 修复**告一段落时**追加一条，不是每个 edit 一条。强标签格式见 CHANGELOG.md 自身顶部。**只记摘要 + Why + 详细记录指针**，不贴 diff，**不记接力 / 未尽事项**（那是 INDEX Handoff 段的事）。
- **PROJECT_PROGRESS.md（非工程进度页）**：**每个阶段或可验收小任务后更新**。写给不懂代码工程的人看，用普通中文说明“现在到哪了 / 做完什么 / 下一步是什么 / 怎么验收 / 风险有什么变化”。不要只列文件名、命令或内部实现。

## 2. 任务阶段性成果 → 自动更新三件套

任何 agent 在完成一个任务阶段（功能落地 / bug 修完 / 重构告段落）后，**默认就要把三件套同步到位**，不必等用户喊"收尾"：

| 信号 | 该更新 |
|---|---|
| 新增项目惯例 / 踩坑 / 约束 | AGENTS.md |
| 目录 / 子模块结构变化 | INDEX.md 导航段 |
| 完整告一段落的改动 | CHANGELOG.md 一条（强标签） |
| 阶段或可验收小任务完成 | PROJECT_PROGRESS.md 的已完成 / 下一步 / 验收 / 风险 |
| 未尽事项 / 待联调 / 待另一台机器接力 / 待用户触发 | INDEX.md 顶部 `## 当前接力点 (Handoff)` 段（详见 §3） |

## 3. INDEX.md 顶部"当前接力点 (Handoff)"段的强约束

- **位置固定**：INDEX.md 一句话项目定位之后、第一个 `## ` 实质段（通常是"项目结构"或"在用模块"）之前，标题必须**字面**为 `## 当前接力点 (Handoff)`
- **永远只有一条最新**：覆盖式写，不堆历史。历史脉络写到 obwiki
- **写入触发**：① `/wrap-up`（或 `/jilu`）识别到明显未完成 / 需要接力 → 自动写；② 用户在对话中显式说"加个接力 / 把 X 放接力位 / 接力指令是 ..." → 原样写
- **自动清理**：下一轮 wrap-up 或下一个 agent 接手时判断——产物已落地 / 用户说"做完了" / 命令已跑过 → **直接清掉该段**。过期的接力点比没有更误导
- **不开独立文件**：禁止新建 `relay-task.md` / `HANDOFF.md` / `TODO.md`
- **不写到 CHANGELOG**：CHANGELOG 记**已发生**，Handoff 是**未发生**，语义冲突

### 内容格式

```markdown
## 当前接力点 (Handoff)

> 只保留最新一条，下一轮 wrap-up 直接覆盖。历史接力沉淀到 obwiki。

- **YYYY-MM-DD**：<一句话下一步>；<关键命令 / 文件指针>；详见 obwiki <wiki page>。
```

## 4. Agent 必须主动提醒同步的节点

Agent 在以下节点应**主动**提示用户做 INDEX/CHANGELOG 同步——用户干活时不会自己想起：

- **任务阶段性收尾**：一个功能 / 修复 / 重构告一段落、即将切话题前
- **上下文长度即将触发压缩**：感觉对话已经很长、再几轮可能被压缩，趁记忆还在赶紧落
- **用户明确说**"沉淀一下""做个 checkpoint""收尾"

提醒话术示例：
> 这一段告一段落了。我会同步 PROJECT_PROGRESS.md 给非工程读者看，并把 CHANGELOG 加一条检索记录。这一阶段的改动清单：...

## 5. 子项目嵌套（max-depth = 3）

### 判定与最大深度

- **判定**：子目录里同时有 `AGENTS.md + INDEX.md + CHANGELOG.md` 三件套 → 它是一个**子项目**
- **最大深度 3 层**：根项目（0）/ 子项目（1）/ 孙子项目（2）。**禁止再嵌套**。再深就拆成独立项目，不要继续嵌套
- **trio-protocol.md 位置**：根项目持有 `docs/trio-protocol.md`；子 / 孙级**不持有副本**，AGENTS 用相对路径引用：
  - 子项目 → `../docs/trio-protocol.md`
  - 孙子项目 → `../../docs/trio-protocol.md`

### 更新边界（核心规则）

| 操作类型 | 当前层三件套 | 父层 INDEX | 父层 CHANGELOG |
|---|---|---|---|
| 当前层内部的开发 / 改动 | ✅ 按节奏更新 | ❌ 不动 | ❌ **绝不记录** |
| 当前层结构变化（新增 / 重命名 / 归档子模块） | ✅ | ✅ 摘要行 | ❌ 不记录 |
| 横跨多个同级子项目的同时操作 | ✅ 各记一条 | 视情况 | ✅ 多 `scope:` 一条 |

**核心**：父 CHANGELOG **只记录横跨多个子项目的同时操作**，单一子项目内部操作绝不进父 CHANGELOG。

### 为什么这么切

父 CHANGELOG 若收所有子项目流水会被噪音淹没失去检索价值。让每个子项目自己的 CHANGELOG 承担细粒度记录，父级 CHANGELOG 自然成为"项目级里程碑视图"。

## 6. 记忆三条线的边界（auto memory / obwiki / trio）

Claude 生态里有三处可以"记东西"，职责不同，搞混就会漂移。本标准的边界：

### 线 1：auto memory（Claude 官方跨会话记忆）

- **位置**：`~/.claude/projects/<encoded-cwd>/memory/MEMORY.md` + 各 entry 文件（**机器本地，不进 git**）
- **何时加载**：每次会话启动时 `MEMORY.md` 索引自动入上下文
- **存什么**：
  - 用户偏好 / 习惯 / 工作流（`feedback` 类型）
  - 跨项目稳定的口语映射 / 路由动作（`feedback` 类型，如 personal language Hotset）
  - 用户身份 / 角色 / 环境（`user` 类型）
  - 外部系统指针（`reference` 类型，如"bug 在 Linear INGEST 项目"）
- **不存什么**：
  - ❌ 当前项目的状态快照（已装哪些模块 / 当前架构 / 最近 commit）→ 那是 INDEX/CHANGELOG 的事
  - ❌ 事件性知识 / 概念总结 / 跨项目可引用的洞见 → 那是 obwiki 的事
  - ❌ 临时任务进度 → 那是会话级 task 列表的事

### 线 2：obwiki（个人知识库 context-engine）

- **位置**：`~/baidu/obwiki/`（默认）或项目内 vault；可同步、可 git
- **何时加载**：用户显式 `/obwiki retrieve <q>` 时按需 L0→L1→L2 渐进披露
- **存什么**：
  - 事件性知识结晶（一次调试 / 一次重构 / 一次决策的"教训页"）
  - 概念页（某术语 / 某模式 / 某工具的连贯叙述）
  - 跨项目可引用的洞见 / 实体关系
- **不存什么**：
  - ❌ 当前项目的状态快照 → INDEX
  - ❌ 项目活动流水 → CHANGELOG
  - ❌ 高频跨项目口语映射 → auto memory Hotset

### 线 3：trio（项目三件套，本协议管的对象）

- **位置**：项目 repo 内（AGENTS.md / INDEX.md / CHANGELOG.md）
- **何时加载**：AGENTS.md 在 cwd 匹配时自动入上下文；INDEX/CHANGELOG 按需 Read
- **存什么**：
  - 本项目的 agent 操作守则（专属硬规则、目录约定、不要做的事）
  - 本项目的结构快照 + 子模块导航 + Handoff 接力位
  - 本项目的活动流水（强标签 CHANGELOG）
- **不存什么**：
  - ❌ 跨项目偏好 / 口语映射 → auto memory
  - ❌ 概念知识 / 事件结晶 → obwiki

### 三条线的协作示例

| 场景 | 哪条线 |
|---|---|
| 用户说"以后中文回复" | auto memory（feedback） |
| "标准收尾" 映射到 `/wrap-up` | auto memory（Hotset） |
| 本项目把所有 skill 软链分发到四家 agent（这是本项目的定位） | trio（AGENTS"这是什么项目"段） |
| obwiki 新增 retrieve 上下文增强 | trio（obwiki/CHANGELOG `#feat` 条目） |
| 渐进式披露的设计理论 | obwiki（concept 页） |
| 本会话已做完 task 1、正在做 task 2 | 都不存（task 是会话级） |

### 防漂移建议

- **每次 `/wrap-up` 时分流**：事件总结 → obwiki；跨会话偏好 → auto memory；项目活动 → CHANGELOG
- **auto memory 里出现 `project_*` 状态快照**（已装哪些 skill / 集成关系）→ 那是 INDEX 该承担的，**应该删掉迁移**
- **CHANGELOG 里出现"用户偏好"或"理论笔记"** → 那是 auto memory / obwiki 该承担的，**不要写进 CHANGELOG**

## 7. 语言规则

- 解释性内容、架构决策、注意事项 → **中文**
- 代码、变量名、函数名、目录名 → **英文**
- 与项目所有者对话 → **中文**（除非对方用英文）

## 8. 详细改版记录的位置

根目录 `CHANGELOG.md` 只是**索引**。详细改版记录写在该模块自己目录下（如 `<module>/CHANGELOG.md` 或 commit message 里），根 CHANGELOG 用"详见 `<path>`"指过去。

## 9. 不要做的事（跨项目通用反例）

- ❌ 把 INDEX / CHANGELOG 当 AGENTS 用（每轮都改），或把 AGENTS 当 CHANGELOG 用（攒着到收尾才写）——三份文档节奏不同
- ❌ 阶段性收尾或上下文快压缩时**不**主动提醒同步 INDEX / CHANGELOG
- ❌ 在 `CHANGELOG.md` 里贴大段 diff / 长解释
- ❌ **单一子项目操作时往父项目 CHANGELOG 写条目**（父 CHANGELOG 只接收跨多个子项目的同时操作）
- ❌ 自动提交 secrets / 凭证
- ❌ 替用户做 `git push --force` / 任何不可逆操作（必须先问）
- ❌ 把项目状态快照塞 auto memory（会漂移，那是 INDEX 的事）
- ❌ 把跨会话偏好或口语映射写进 CHANGELOG / AGENTS（那是 auto memory 的事）

---

*本协议由 `/project-init` 维护，版本 `standard-v2`。规则演化通过 `/project-init migrate` 升级到 v3 等后续版本。*
