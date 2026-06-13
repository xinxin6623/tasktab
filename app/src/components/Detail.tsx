// Detail.tsx —— 项目详情页（M3）。
// 包含：阶段垂直时间线（完成✓/当前高亮+阶段内进度/未来灰）、完整 next 列表、
//       blocked_by 警示条、PROGRESS.md 正文 markdown 只读预览、返回导航 + 动作按钮。
// App 端零智能：正文用确定性 markdown 库 marked 渲染，绝不调 LLM、不做网络请求。
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { marked } from "marked";
import type { ProjectDetail } from "../types";
import { openInEditor, openProgress } from "../actions";
import { useToast } from "./Toast";

const STATUS_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  done: "已完成",
  unknown: "未知",
};

// marked 同步渲染配置：确定性、无网络；GitHub 风格换行。
marked.setOptions({ gfm: true, breaks: true });

export function Detail({ id, onBack }: { id: string; onBack: () => void }) {
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const toast = useToast();

  useEffect(() => {
    invoke<ProjectDetail>("load_project", { id })
      .then(setDetail)
      .catch((e) => setErr(String(e)));
  }, [id]);

  if (err) {
    return (
      <DetailShell onBack={onBack} title="加载失败">
        <div className="notice">无法加载该项目：{err}</div>
      </DetailShell>
    );
  }
  if (!detail) {
    return (
      <DetailShell onBack={onBack} title="加载中…">
        <div className="notice">加载中…</div>
      </DetailShell>
    );
  }

  const p = detail.card;

  // 动作按钮：失败统一 toast 提示
  const doOpenProgress = async () => {
    try {
      await openProgress(p.progress_path);
    } catch (e) {
      toast(`打开 PROGRESS.md 失败：${e}`);
    }
  };
  const doOpenEditor = async () => {
    try {
      await openInEditor(p.path);
    } catch (e) {
      toast(`用 VS Code 打开失败：${e}`);
    }
  };

  // 降级：格式异常 / 文件缺失 —— 仍给「打开文件」入口
  if (p.error) {
    const label = p.error.kind === "missing" ? "文件缺失" : "格式异常";
    return (
      <DetailShell onBack={onBack} title={p.name} badge={{ cls: "warn", text: `⚠ ${label}` }}>
        <div className="warn-msg detail-block">{p.error.message}</div>
        <div className="detail-actions">
          <button className="btn" onClick={doOpenProgress}>打开 PROGRESS.md</button>
          <button className="btn" onClick={doOpenEditor}>VS Code 打开项目</button>
        </div>
        <div className="detail-path">{p.path}</div>
      </DetailShell>
    );
  }

  const pct = Math.round(p.overall_progress);
  // marked.parse 在 async:false（默认）下返回 string；这里用确定性同步渲染。
  const bodyHtml = detail.body.trim()
    ? (marked.parse(detail.body) as string)
    : "";

  return (
    <DetailShell
      onBack={onBack}
      title={p.name}
      badge={{ cls: p.status, text: STATUS_LABEL[p.status] ?? p.status }}
    >
      {/* 整体进度 */}
      <div className="progress detail-block">
        <div className="progress-track">
          <div className="progress-fill" style={{ width: `${pct}%` }} />
        </div>
        <div className="progress-meta">
          <span>整体进度</span>
          <span>{pct}%</span>
        </div>
      </div>

      {/* blocked_by 警示条（非空时） */}
      {p.blocked_by.length > 0 && (
        <div className="blocked detail-block">
          <div className="blocked-title">⚠ 被阻塞</div>
          <ul>
            {p.blocked_by.map((b, i) => (
              <li key={i}>{b}</li>
            ))}
          </ul>
        </div>
      )}

      {/* 阶段垂直时间线 */}
      <section className="detail-block">
        <h3 className="detail-h">阶段时间线</h3>
        <ol className="timeline">
          {p.stages.map((stage, i) => {
            const idx = i + 1;
            const isDone = idx < p.current_stage || p.status === "done";
            const isCurrent = idx === p.current_stage && p.status !== "done";
            const cls = isDone ? "done" : isCurrent ? "current" : "future";
            return (
              <li key={i} className={`tl-item ${cls}`}>
                <span className="tl-marker">{isDone ? "✓" : idx}</span>
                <div className="tl-body">
                  <div className="tl-name">{stage}</div>
                  {isCurrent && (
                    <div className="tl-progress">
                      <div className="progress-track sm">
                        <div
                          className="progress-fill"
                          style={{ width: `${Math.round(p.stage_progress)}%` }}
                        />
                      </div>
                      <span className="tl-pct">阶段内 {Math.round(p.stage_progress)}%</span>
                    </div>
                  )}
                </div>
              </li>
            );
          })}
        </ol>
      </section>

      {/* 完整 next 列表 */}
      {p.next.length > 0 && (
        <section className="detail-block">
          <h3 className="detail-h">接下来</h3>
          <ul className="next">
            {p.next.map((n, i) => (
              <li key={i}>{n}</li>
            ))}
          </ul>
        </section>
      )}

      {/* PROGRESS.md 正文 markdown 只读预览 */}
      <section className="detail-block">
        <h3 className="detail-h">PROGRESS.md 正文</h3>
        {bodyHtml ? (
          <div className="md-preview" dangerouslySetInnerHTML={{ __html: bodyHtml }} />
        ) : (
          <div className="md-empty">（正文为空）</div>
        )}
      </section>

      {/* 元信息 + 动作按钮 */}
      <div className="detail-actions">
        <button className="btn" onClick={doOpenProgress}>打开 PROGRESS.md</button>
        <button className="btn" onClick={doOpenEditor}>VS Code 打开项目</button>
      </div>
      <div className="detail-meta">
        <span>更新 {p.updated || "—"}</span>
        <span className="detail-path">{p.path}</span>
      </div>
    </DetailShell>
  );
}

// 详情页外壳：顶栏（返回 + 标题 + 徽标）+ 内容
function DetailShell({
  onBack,
  title,
  badge,
  children,
}: {
  onBack: () => void;
  title: string;
  badge?: { cls: string; text: string };
  children: React.ReactNode;
}) {
  return (
    <>
      <div className="topbar">
        <button className="btn back" onClick={onBack}>← 返回看板</button>
        <h1 className="detail-title">
          {title}
          {badge && <span className={`badge ${badge.cls}`}>{badge.text}</span>}
        </h1>
      </div>
      <div className="detail-wrap">{children}</div>
    </>
  );
}
