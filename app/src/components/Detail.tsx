// Detail.tsx —— 项目详情页。
// 改版后布局（上→下）：进度条 → blocked_by 警示 → 项目简介(INDEX) → 阶段分块列表(CHANGELOG，10 行+滚动)
//       → 架构图(INDEX mermaid) → 动作按钮(打开 INDEX.md / VS Code) → 更新+路径。
// 数据全部来自三件套：AGENTS.md(status/desc) + INDEX.md(简介/架构图/Handoff) + CHANGELOG.md(阶段表/日期)。
// App 端零智能：确定性提取 + mermaid 本地渲染，绝不调 LLM、不做网络请求。
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ProjectDetail } from "../types";
import { openInEditor, openIndex } from "../actions";
import { Mermaid } from "./Mermaid";
import { useToast } from "./Toast";

const STATUS_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  done: "已完成",
  unknown: "未知",
};

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
  const doOpenIndex = async () => {
    try {
      await openIndex(p.path);
    } catch (e) {
      toast(`打开 INDEX.md 失败：${e}`);
    }
  };
  const doOpenEditor = async () => {
    try {
      await openInEditor(p.path);
    } catch (e) {
      toast(`用 VS Code 打开失败：${e}`);
    }
  };

  // 降级：未接入看板 / 格式异常 —— 仍给「打开文件」入口引导补三件套
  if (p.error) {
    const label = p.error.kind === "missing" ? "未接入看板" : "格式异常";
    return (
      <DetailShell onBack={onBack} title={p.name} badge={{ cls: "warn", text: `⚠ ${label}` }}>
        <div className="warn-msg detail-block">{p.error.message}</div>
        <div className="detail-actions">
          <button className="btn" onClick={doOpenIndex}>打开 INDEX.md</button>
          <button className="btn" onClick={doOpenEditor}>VS Code 打开项目</button>
        </div>
        <div className="detail-path">{p.path}</div>
      </DetailShell>
    );
  }

  const pct = Math.round(p.overall_progress);

  return (
    <DetailShell
      onBack={onBack}
      title={p.name}
      badge={{ cls: p.status, text: STATUS_LABEL[p.status] ?? p.status }}
    >
      {/* 进度条（仅条形，无文字行） */}
      <div className="progress detail-block">
        <div className="progress-track">
          <div className="progress-fill" style={{ width: `${pct}%` }} />
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

      {/* 项目简介（INDEX ## 项目简介；缺失不显示） */}
      {detail.intro && <p className="detail-intro detail-block">{detail.intro}</p>}

      {/* 阶段分块列表（CHANGELOG ## 项目阶段；缺失不显示；10 行+滚动） */}
      {detail.stages.length > 0 && (
        <section className="detail-block">
          <h3 className="detail-h">项目阶段</h3>
          <ul className="stage-list">
            {detail.stages.map((s, i) => (
              <li key={i} className={`stage-row ${s.done ? "done" : ""}`}>
                <span className="stage-mark">{s.done ? "✓" : "○"}</span>
                <span className="stage-name">{s.name}</span>
                {s.desc && <span className="stage-desc">{s.desc}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* 架构图（INDEX ## 架构图 mermaid；缺失不显示） */}
      {detail.arch_mermaid && (
        <section className="detail-block">
          <h3 className="detail-h">架构图</h3>
          <Mermaid code={detail.arch_mermaid} />
        </section>
      )}

      {/* 动作按钮 */}
      <div className="detail-actions">
        <button className="btn" onClick={doOpenIndex}>打开 INDEX.md</button>
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
