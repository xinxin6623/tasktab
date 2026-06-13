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
