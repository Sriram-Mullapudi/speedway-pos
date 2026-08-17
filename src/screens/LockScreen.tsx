import { useState } from "react";
import PinPad from "../components/PinPad";
import { useSession } from "../sessionStore";

export default function LockScreen() {
  const login = useSession((s) => s.login);
  const [pin, setPin] = useState("");
  const [err, setErr] = useState<string | null>(null);

  async function submit() {
    if (!pin) return;
    try {
      await login(pin);
    } catch (e) {
      setErr(String(e));
      setPin("");
    }
  }

  return (
    <div className="lock">
      <div className="lock-card">
        <div className="lock-brand">Speedway Market</div>
        <div className="lock-sub">Enter your PIN to sign in</div>
        <PinPad value={pin} onChange={(v) => { setPin(v); setErr(null); }} onSubmit={submit} maxLen={4} />
        {err && <div className="lock-err">{err}</div>}
        <div className="lock-hint">Demo PINs — Admin 1234 · Manager 2222 · Cashier 1111</div>
      </div>
    </div>
  );
}
