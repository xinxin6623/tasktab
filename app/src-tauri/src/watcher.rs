// watcher.rs —— M4：文件监听自动刷新
//
// 中文说明（重要逻辑）：
// 用 notify crate（macOS 后端即 FSEvents）监听两类文件，文件一变就通知前端重拉看板，
// 用户永远不需要手动刷新（项目核心理念：文件是唯一真相）：
//   1) registry.yaml 本身：增删项目（cra add / App 内增删）会改它。
//   2) 每个已登记项目的 PROGRESS.md（路径 = 展开后的 path / progress_file）：进度更新会改它。
//
// 去抖（debounce）500ms：编辑器（vim 等）保存常触发多个连续事件（写临时文件→rename→属性变更），
// 用 notify-debouncer-full 把 500ms 窗口内的事件合并成一次，避免前端被刷爆。
//
// registry 变更时重建监听列表：registry 改了 → 重新读 registry → 比对 PROGRESS.md 文件集合，
// 新项目的文件加入监听、移除的项目取消监听。这样 cra add 新项目后看板能自动出现新卡片。
//
// 监听父目录而非文件本身：vim 保存 / registry 原子写都用「写新文件 + rename 覆盖」，
// 直接监听文件 inode 在 rename 后会丢失目标。监听其父目录（非递归）能稳定捕获子文件的
// create/modify/remove/rename，再用关心的路径集合过滤，鲁棒性最好。
//
// 线程安全：watcher 跑在后台线程，持有 AppHandle 的 clone（AppHandle 可 Clone 且线程安全），
// 事件去抖后用 app_handle.emit("board-changed", ...) 把信号推给前端。
// App 端零智能、零网络：这里只发「板子变了」的信号，不做任何解析/判断，解析仍由 load_board 负责。

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent, Debouncer, FileIdMap};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::board;

/// 前端订阅的事件名：收到即重新 invoke("load_board") 重拉整盘（load_board 很轻，整体重拉最简单）。
const EVENT_BOARD_CHANGED: &str = "board-changed";

/// 去抖窗口：02 §3 M4 要求 500ms。
const DEBOUNCE_MS: u64 = 500;

/// 在后台线程启动文件监听。由 Tauri setup 钩子调用（拿到 app_handle 后）。
/// 失败（如 notify 初始化不了）时仅打印日志，不影响 App 启动——监听是增强，不是必需。
pub fn spawn(app_handle: AppHandle) {
    std::thread::spawn(move || {
        if let Err(e) = run_watch_loop(app_handle) {
            eprintln!("[watcher] 文件监听启动失败（看板仍可用，仅自动刷新失效）: {e}");
        }
    });
}

/// 监听主循环：建 debouncer → 初始挂载监听集合 → 收去抖事件 → 命中关心路径则 emit + 重建监听集合。
fn run_watch_loop(app_handle: AppHandle) -> Result<(), String> {
    let registry_path = board::default_registry_path();

    // 通道接收去抖后的事件批次
    let (tx, rx): (
        _,
        Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
    ) = std::sync::mpsc::channel();

    let mut debouncer: Debouncer<RecommendedWatcher, FileIdMap> =
        new_debouncer(Duration::from_millis(DEBOUNCE_MS), None, tx)
            .map_err(|e| format!("创建 debouncer 失败: {e}"))?;

    // 当前已挂载监听的「目录集合」。监听父目录（见文件头说明），故这里存目录。
    let mut watched_dirs: HashSet<PathBuf> = HashSet::new();
    // 关心的文件路径集合（registry + 各 PROGRESS.md），用于过滤无关事件。
    let mut tracked_files: HashSet<PathBuf> = HashSet::new();

    // 初次挂载
    rebuild_watches(
        &mut debouncer,
        &registry_path,
        &mut watched_dirs,
        &mut tracked_files,
    );

    // 阻塞接收去抖事件。channel 关闭（App 退出）时退出循环。
    for res in rx {
        let events = match res {
            Ok(evs) => evs,
            Err(errs) => {
                eprintln!("[watcher] 监听事件错误: {errs:?}");
                continue;
            }
        };

        // 该批次是否命中我们关心的任一路径？
        let hit = events.iter().any(|ev| {
            ev.paths
                .iter()
                .any(|p| path_is_tracked(p, &tracked_files))
        });
        if !hit {
            continue;
        }

        // registry 本身是否变了 → 变了就需要重建监听集合（项目可能增删）。
        let registry_changed = events.iter().any(|ev| {
            ev.paths.iter().any(|p| same_file(p, &registry_path))
        });

        if registry_changed {
            rebuild_watches(
                &mut debouncer,
                &registry_path,
                &mut watched_dirs,
                &mut tracked_files,
            );
        }

        // 通知前端重拉看板（增量或整体都可，这里整体重拉，最简单且 load_board 很轻）。
        // 注：新「设备间同步」方案下，服务器自己从 GitHub 聚合看板，App 不再向服务器推送
        //（旧 push.rs 已退役到 archive/）。这里只发本地「板子变了」信号，前端收到后顺带
        // 重拉一次远端 board.json + 本地 git 状态刷新同步徽章（见前端 App.tsx）。
        if let Err(e) = app_handle.emit(EVENT_BOARD_CHANGED, ()) {
            eprintln!("[watcher] emit board-changed 失败: {e}");
        }
    }

    Ok(())
}

/// 重建监听集合：重新读 registry → 算出需要监听的目录与关心文件 → 与当前已挂载集合做差分增删。
/// 监听的目录 = { registry 所在目录 } ∪ { 每个 PROGRESS.md 所在目录 }（去重）。
fn rebuild_watches(
    debouncer: &mut Debouncer<RecommendedWatcher, FileIdMap>,
    registry_path: &Path,
    watched_dirs: &mut HashSet<PathBuf>,
    tracked_files: &mut HashSet<PathBuf>,
) {
    let (desired_dirs, files) = compute_watch_targets(registry_path);
    *tracked_files = files;

    // 新增需要监听、当前未监听的目录
    for dir in desired_dirs.difference(watched_dirs).cloned().collect::<Vec<_>>() {
        // 监听不存在的目录会失败：父目录可能还没建（项目刚登记）。能监听就监听，监听不上就跳过，
        // 下次 registry 变更或目录出现后会再尝试。注册表目录我们会主动 create_dir_all（见 lib.rs setup）。
        if dir.exists() {
            // notify-debouncer-full 0.3：需同时 watch（拿事件）+ cache.add_root（让去抖层
            // 追踪文件 id，正确识别 rename 覆盖——vim 保存与 registry 原子写都靠 rename）。
            if let Err(e) = debouncer.watcher().watch(&dir, RecursiveMode::NonRecursive) {
                eprintln!("[watcher] 监听目录失败 {}: {e}", dir.display());
                continue;
            }
            debouncer.cache().add_root(&dir, RecursiveMode::NonRecursive);
            watched_dirs.insert(dir);
        }
    }

    // 移除不再需要监听的目录
    for dir in watched_dirs.difference(&desired_dirs).cloned().collect::<Vec<_>>() {
        let _ = debouncer.watcher().unwatch(&dir);
        debouncer.cache().remove_root(&dir);
        watched_dirs.remove(&dir);
    }
}

/// 计算监听目标：返回 (需监听目录集合, 关心文件集合)。纯函数式，便于单测。
/// 关心文件 = registry.yaml + 各 PROGRESS.md；目录 = 这些文件各自的父目录去重。
fn compute_watch_targets(registry_path: &Path) -> (HashSet<PathBuf>, HashSet<PathBuf>) {
    let mut files: HashSet<PathBuf> = HashSet::new();
    files.insert(registry_path.to_path_buf());
    for p in board::collect_watched_files_from(registry_path) {
        files.insert(p);
    }

    let mut dirs: HashSet<PathBuf> = HashSet::new();
    for f in &files {
        if let Some(parent) = f.parent() {
            dirs.insert(parent.to_path_buf());
        }
    }
    (dirs, files)
}

/// 事件路径 p 是否落在我们关心的文件集合里。
/// 直接相等比较；同时容忍 macOS 上 /private 前缀差异等通过 same_file 兜底。
fn path_is_tracked(p: &Path, tracked: &HashSet<PathBuf>) -> bool {
    if tracked.contains(p) {
        return true;
    }
    tracked.iter().any(|t| same_file(p, t))
}

/// 判断两个路径是否指向同一文件。优先按规整后路径比较，容忍 macOS 的
/// /var ↔ /private/var 软链、相对/绝对差异。文件不存在时退化为字面比较。
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

// ═════════════════════════ 单元测试 ═════════════════════════
// 说明：FSEvents 实时性需要桌面环境，单测里不验证「真机保存→收到事件」的实时链路
//（那部分需 James 本机 tauri dev 终验）。这里只测可纯函数化的部分：监听目标集合计算、
// 路径命中判断、registry 变更后目标集合的增删，保证去抖/重建逻辑本身正确。
#[cfg(test)]
mod tests {
    use super::*;

    fn unique_tmp() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "tb_watch_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn mkproj(tmp: &Path, dir: &str) -> PathBuf {
        let root = tmp.join(dir);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("PROGRESS.md"),
            "---\nproject: x\nstages:\n  - a\ncurrent_stage: 1\n---\n",
        )
        .unwrap();
        root
    }

    // 监听目标：registry 文件 + 每个 PROGRESS.md 都在关心集合里，目录是它们的父目录去重。
    #[test]
    fn test_compute_watch_targets_collects_files_and_dirs() {
        let tmp = unique_tmp();
        let p1 = mkproj(&tmp, "voice");
        let p2 = mkproj(&tmp, "alpha");
        let reg = tmp.join("registry.yaml");
        std::fs::write(
            &reg,
            format!(
                "version: 1\nprojects:\n  - id: voice\n    path: {}\n  - id: alpha\n    path: {}\n",
                p1.display(),
                p2.display()
            ),
        )
        .unwrap();

        let (dirs, files) = compute_watch_targets(&reg);
        // 关心文件：registry + 每项目 4 个（AGENTS/INDEX/CHANGELOG/PROGRESS）×2 = 9
        assert!(files.contains(&reg));
        assert!(files.contains(&p1.join("AGENTS.md")));
        assert!(files.contains(&p1.join("CHANGELOG.md")));
        assert!(files.contains(&p2.join("INDEX.md")));
        assert!(files.contains(&p2.join("PROGRESS.md")));
        assert_eq!(files.len(), 9);
        // 目录：registry 父目录(=tmp) + 两个项目目录
        assert!(dirs.contains(&tmp));
        assert!(dirs.contains(&p1));
        assert!(dirs.contains(&p2));
    }

    // 同一目录下的多个项目：父目录应去重为一个。
    #[test]
    fn test_compute_watch_targets_dedups_dirs() {
        let tmp = unique_tmp();
        // 两个项目，PROGRESS.md 通过 progress_file 指向同一目录下不同文件
        let root = tmp.join("mono");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("A.md"), "x").unwrap();
        std::fs::write(root.join("B.md"), "x").unwrap();
        let reg = tmp.join("registry.yaml");
        std::fs::write(
            &reg,
            format!(
                "version: 1\nprojects:\n  - id: a\n    path: {root}\n    progress_file: A.md\n  - id: b\n    path: {root}\n    progress_file: B.md\n",
                root = root.display()
            ),
        )
        .unwrap();
        let (dirs, files) = compute_watch_targets(&reg);
        // registry + 同目录下去重后的 AGENTS/INDEX/CHANGELOG（各 1）+ A.md + B.md = 6
        assert_eq!(files.len(), 6);
        // 目录去重：registry 父目录(tmp) + mono，共 2 个（两个项目共用 mono）
        assert_eq!(dirs.len(), 2);
        assert!(dirs.contains(&root));
    }

    // 路径命中判断
    #[test]
    fn test_path_is_tracked() {
        let tmp = unique_tmp();
        let f = tmp.join("PROGRESS.md");
        std::fs::write(&f, "x").unwrap();
        let mut tracked = HashSet::new();
        tracked.insert(f.clone());
        assert!(path_is_tracked(&f, &tracked));
        assert!(!path_is_tracked(&tmp.join("OTHER.md"), &tracked));
    }

    // registry 变更（新增项目）后重算目标，新项目的文件与目录应进入集合。
    #[test]
    fn test_targets_grow_after_registry_adds_project() {
        let tmp = unique_tmp();
        let p1 = mkproj(&tmp, "voice");
        let reg = tmp.join("registry.yaml");
        std::fs::write(
            &reg,
            format!("version: 1\nprojects:\n  - id: voice\n    path: {}\n", p1.display()),
        )
        .unwrap();
        let (_, files_before) = compute_watch_targets(&reg);
        assert_eq!(files_before.len(), 5); // registry + voice 的 4 个文件

        // 模拟 cra add：registry 追加新项目
        let p2 = mkproj(&tmp, "beta");
        std::fs::write(
            &reg,
            format!(
                "version: 1\nprojects:\n  - id: voice\n    path: {}\n  - id: beta\n    path: {}\n",
                p1.display(),
                p2.display()
            ),
        )
        .unwrap();
        let (dirs_after, files_after) = compute_watch_targets(&reg);
        assert_eq!(files_after.len(), 9); // registry + 2 项目 ×4
        assert!(files_after.contains(&p2.join("PROGRESS.md")));
        assert!(files_after.contains(&p2.join("CHANGELOG.md")));
        assert!(dirs_after.contains(&p2));
    }
}
