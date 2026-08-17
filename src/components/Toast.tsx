import { create } from "zustand";

interface Toast { id: number; msg: string; kind: "ok" | "err"; }
interface ToastState {
  list: Toast[];
  push: (msg: string, kind: "ok" | "err") => void;
  drop: (id: number) => void;
}

let nextId = 1;
export const useToasts = create<ToastState>((set) => ({
  list: [],
  push: (msg, kind) => {
    const id = nextId++;
    set((s) => ({ list: [...s.list, { id, msg, kind }] }));
    setTimeout(() => set((s) => ({ list: s.list.filter((t) => t.id !== id) })), 3200);
  },
  drop: (id) => set((s) => ({ list: s.list.filter((t) => t.id !== id) })),
}));

/** Fire a toast from anywhere. */
export const notify = (msg: string, kind: "ok" | "err" = "ok") =>
  useToasts.getState().push(msg, kind);

export function Toasts() {
  const { list, drop } = useToasts();
  return (
    <div className="toasts">
      {list.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`} onClick={() => drop(t.id)}>{t.msg}</div>
      ))}
    </div>
  );
}
