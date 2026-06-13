// actions.ts —— App 动作按钮的前端封装（M3）。
// 全部走 Rust command（lib.rs），前端不直接调 shell；App 端零智能、零网络。
import { invoke } from "@tauri-apps/api/core";

/** 打开项目根 INDEX.md（open_progress command 本质是通用 `open <file>`）。 */
export async function openIndex(projectRoot: string): Promise<void> {
  const sep = projectRoot.endsWith("/") ? "" : "/";
  await invoke("open_progress", { path: `${projectRoot}${sep}INDEX.md` });
}

/** 用 VS Code 打开项目目录：Rust 侧先试 `code`，降级 `open -a "Visual Studio Code"`。 */
export async function openInEditor(path: string): Promise<void> {
  await invoke("open_in_editor", { path });
}
