// Toast.tsx —— 极简全局 toast（动作失败 / 操作反馈提示）。
// 设计：单条短消息，3 秒后自动消失；样式沿用 §4 设计 token，不引第三方库。
import { createContext, useCallback, useContext, useState, type ReactNode } from "react";

type ToastFn = (message: string) => void;

const ToastContext = createContext<ToastFn>(() => {});

/** 在任意组件里取 toast 函数：const toast = useToast(); toast("出错了"); */
export function useToast(): ToastFn {
  return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [msg, setMsg] = useState<string | null>(null);

  const toast = useCallback((message: string) => {
    setMsg(message);
    // 3 秒后自动消失（重复调用会刷新内容，简单够用）
    window.setTimeout(() => setMsg(null), 3000);
  }, []);

  return (
    <ToastContext.Provider value={toast}>
      {children}
      {msg && (
        <div className="toast" role="status" onClick={() => setMsg(null)}>
          {msg}
        </div>
      )}
    </ToastContext.Provider>
  );
}
