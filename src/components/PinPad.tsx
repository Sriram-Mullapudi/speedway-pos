import { useEffect } from "react";

export default function PinPad({
  value, onChange, onSubmit, maxLen = 6,
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  maxLen?: number;
}) {
  const press = (d: string) => { if (value.length < maxLen) onChange(value + d); };
  const back = () => onChange(value.slice(0, -1));

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key >= "0" && e.key <= "9") press(e.key);
      else if (e.key === "Backspace") back();
      else if (e.key === "Enter") onSubmit();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });

  return (
    <div className="pinpad">
      <div className="pindots">
        {Array.from({ length: maxLen }).map((_, i) => (
          <span key={i} className={`pindot ${i < value.length ? "on" : ""}`} />
        ))}
      </div>
      <div className="keys">
        {["1","2","3","4","5","6","7","8","9"].map((d) => (
          <button key={d} className="key" onClick={() => press(d)}>{d}</button>
        ))}
        <button className="key ghost" onClick={back}>⌫</button>
        <button className="key" onClick={() => press("0")}>0</button>
        <button className="key go" onClick={onSubmit}>↵</button>
      </div>
    </div>
  );
}
