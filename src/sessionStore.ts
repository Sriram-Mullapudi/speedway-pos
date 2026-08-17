import { create } from "zustand";
import { getActiveShift, getCurrentSession, loginWithPin, logoutCashier } from "./api";
import type { SessionInfo, Shift } from "./types";

interface SessionState {
  session: SessionInfo | null;
  activeShift: Shift | null;
  ready: boolean;
  refresh: () => Promise<void>;
  login: (pin: string) => Promise<void>;
  logout: () => Promise<void>;
  setShift: (s: Shift | null) => void;
  isManager: () => boolean;
}

export const useSession = create<SessionState>((set, get) => ({
  session: null,
  activeShift: null,
  ready: false,
  refresh: async () => {
    const session = await getCurrentSession();
    const activeShift = session ? await getActiveShift() : null;
    set({ session, activeShift, ready: true });
  },
  login: async (pin) => {
    await loginWithPin(pin);
    await get().refresh();
  },
  logout: async () => {
    await logoutCashier();
    set({ session: null, activeShift: null });
  },
  setShift: (s) => set({ activeShift: s }),
  isManager: () => {
    const r = get().session?.role;
    return r === "manager" || r === "admin";
  },
}));
