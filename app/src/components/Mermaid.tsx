// Mermaid.tsx —— 架构图渲染（详情页用）。
// mermaid 包较大（~3MB），用动态 import 只在详情页加载，首页不受影响。
// 零网络：mermaid bundle 进 dist，运行时不联网（符合 App 端零智能/零网络硬规则）。
// 渲染失败（语法错）→ 降级显示原文，绝不崩整页。
import { useEffect, useRef, useState } from "react";

let seq = 0; // 每次渲染给一个唯一 id，避免 mermaid 复用 DOM 冲突

export function Mermaid({ code }: { code: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setFailed(false);
    (async () => {
      try {
        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
        const id = `arch-${seq++}`;
        const { svg } = await mermaid.render(id, code);
        if (!cancelled && ref.current) {
          ref.current.innerHTML = svg;
        }
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [code]);

  if (failed) {
    return (
      <div className="mermaid-box">
        <div className="md-empty">架构图渲染失败，原文：</div>
        <pre className="mermaid-raw">{code}</pre>
      </div>
    );
  }

  return <div className="mermaid-box" ref={ref} />;
}
