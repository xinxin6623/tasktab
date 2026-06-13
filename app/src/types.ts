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
