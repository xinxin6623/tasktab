// 与 Rust 侧 board.rs 的序列化结构一一对应（前端零解析，只渲染）。

export interface ParseError {
  kind: "format" | "missing";
  message: string;
}

export interface ProjectCard {
  id: string;
  name: string;
  path: string;
  progress_path: string;
  pinned: boolean;
  desc: string; // 项目一句话描述（frontmatter desc，可选；空串则卡片不渲染该行）
  status: string; // active | paused | done | unknown
  stages: string[];
  current_stage: number;
  stage_progress: number;
  overall_progress: number; // 0..=100
  next: string[];
  blocked_by: string[];
  updated: string;
  error: ParseError | null;
}

export interface Summary {
  active: number;
  paused: number;
  done: number;
  error: number;
}

export interface Board {
  projects: ProjectCard[];
  summary: Summary;
  registry_error: string | null;
}

// 阶段分块项（CHANGELOG.md `## 项目阶段` checkbox 列表，与 Rust StageItem 对应）。
export interface StageItem {
  name: string;
  desc: string;
  done: boolean;
}

// 单项目详情：load_project command 返回（卡片字段 + 三件套块）。
export interface ProjectDetail {
  card: ProjectCard;
  body: string; // PROGRESS.md 正文原文（详情页改版后不再渲染，保留兼容）
  intro: string; // INDEX.md ## 项目简介 块纯文本，缺失为空串
  arch_mermaid: string; // INDEX.md ## 架构图 下首个 mermaid 块源码，缺失为空串
  stages: StageItem[]; // CHANGELOG.md ## 项目阶段 列表，缺失为空数组
}

// App 内添加项目的返回。
export interface AddResult {
  id: string;
  name: string;
  path: string;
  created_template: boolean;
}

// ───────────────────────── 设备间同步 ─────────────────────────

// 单项目本地 git 状态（load_local_sync command 返回，与 Rust sync::LocalGitStatus 对应）。
export interface LocalGitStatus {
  id: string;
  head: string; // 本地 HEAD SHA；非 git / 取不到 → ""
  dirty: boolean; // 工作区有未提交改动
  ahead: boolean; // 本地有未 push 的提交
  behind: boolean; // 上游有未 pull 的提交
  is_git: boolean;
}

// 同步徽章种类。前端按「本地 HEAD vs 服务器 commit + 本地 git 状态」算出。
export type SyncKind =
  | "synced" // 本地 HEAD == 服务器 commit 且工作区干净
  | "dirty" // 工作区有未提交改动
  | "ahead" // 本地领先（有未 push 提交）
  | "behind" // 本地落后（服务器/上游更新）
  | "diverged" // 本地与服务器 commit 不一致且方向不明
  | "offline" // 远端未配置 / 拉不到，无对比基准
  | "unknown"; // 非 git 仓库 / 信息不足

// 卡片上要展示的同步状态（已算好文案与种类）。
export interface SyncBadge {
  kind: SyncKind;
  label: string; // 中文短文案，如「已同步」「待推送」
  title: string; // hover 详细说明
}
