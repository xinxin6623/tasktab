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

// 单项目详情：load_project command 返回（卡片字段 + 正文原文）。
export interface ProjectDetail {
  card: ProjectCard;
  body: string; // frontmatter 之后的正文 markdown 原文
}

// App 内添加项目的返回。
export interface AddResult {
  id: string;
  name: string;
  path: string;
  created_template: boolean;
}
