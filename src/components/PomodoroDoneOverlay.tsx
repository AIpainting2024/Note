import { useEffect, useState } from "react";

type Props = {
  /** 每次结束时把 token 换成新值即可重放动画 */
  token: number;
  /** 主文案 */
  text?: string;
};

/**
 * 番茄钟结束动画（v0.12）：整屏居中的大番茄被一刀切开，
 * 两半分开下落、文字浮起，约 2.8 秒后自动淡出。
 *
 * 纯 CSS/SVG，不引入依赖；pointer-events-none 不挡操作。
 */
export function PomodoroDoneOverlay({ token, text = "番茄完成，休息一下" }: Props) {
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    if (token === 0) return;
    setPlaying(true);
    const timer = window.setTimeout(() => setPlaying(false), 2900);
    return () => window.clearTimeout(timer);
  }, [token]);

  if (!playing) return null;

  return (
    <div
      key={token}
      role="status"
      aria-live="assertive"
      aria-label={text}
      className="pointer-events-none fixed inset-0 z-[70] flex flex-col items-center justify-center gap-4"
      style={{ animation: "wnc-overlay-fade 2.9s ease-out forwards" }}
    >
      <style>{`
        @keyframes wnc-overlay-fade {
          0%   { background: rgba(0,0,0,0); }
          12%  { background: rgba(0,0,0,.22); }
          80%  { background: rgba(0,0,0,.22); }
          100% { background: rgba(0,0,0,0); }
        }
        @keyframes wnc-big-in {
          0%   { opacity: 0; transform: scale(.4); }
          18%  { opacity: 1; transform: scale(1.12); }
          30%  { transform: scale(1); }
          100% { transform: scale(1); }
        }
        @keyframes wnc-big-knife {
          0%, 28%   { opacity: 0; transform: translate(-120px, -96px) rotate(38deg); }
          38%       { opacity: 1; transform: translate(-20px, -16px) rotate(38deg); }
          50%       { opacity: 1; transform: translate(120px, 96px) rotate(38deg); }
          58%, 100% { opacity: 0; transform: translate(140px, 112px) rotate(38deg); }
        }
        @keyframes wnc-big-left {
          0%, 50%   { transform: translate(0,0) rotate(0deg); opacity: 1; }
          100%      { transform: translate(-78px, 90px) rotate(-26deg); opacity: 0; }
        }
        @keyframes wnc-big-right {
          0%, 50%   { transform: translate(0,0) rotate(0deg); opacity: 1; }
          100%      { transform: translate(82px, 96px) rotate(26deg); opacity: 0; }
        }
        @keyframes wnc-big-flash {
          0%, 42%   { opacity: 0; }
          50%       { opacity: .95; }
          62%, 100% { opacity: 0; }
        }
        @keyframes wnc-big-leaf {
          0%, 46%   { opacity: 1; transform: translateY(0); }
          62%, 100% { opacity: 0; transform: translateY(-12px); }
        }
        @keyframes wnc-big-text {
          0%, 30%   { opacity: 0; transform: translateY(14px); }
          55%       { opacity: 1; transform: translateY(0); }
          100%      { opacity: 1; transform: translateY(0); }
        }
      `}</style>

      <div
        className="relative h-[150px] w-[150px]"
        style={{ animation: "wnc-big-in 2.9s cubic-bezier(.34,1.4,.64,1) forwards" }}
      >
        <span
          className="absolute left-1/2 top-1/2 h-6 w-6 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white blur-[4px]"
          style={{ animation: "wnc-big-flash 2.9s ease-out forwards" }}
        />
        <div
          className="absolute left-1/2 top-4 -ml-[58px] h-[116px] w-[116px] rounded-full bg-[#e5484d] shadow-[inset_-10px_-14px_20px_rgba(0,0,0,.18)]"
          style={{
            clipPath: "inset(0 50% 0 0)",
            animation: "wnc-big-left 2.9s cubic-bezier(.4,.1,.7,1) forwards",
          }}
        >
          <span className="absolute left-[24px] top-[28px] h-5 w-7 rounded-full bg-white/35 blur-[2px]" />
        </div>
        <div
          className="absolute left-1/2 top-4 h-[116px] w-[116px] rounded-full bg-[#d13b41] shadow-[inset_10px_-14px_20px_rgba(0,0,0,.2)]"
          style={{
            clipPath: "inset(0 0 0 50%)",
            animation: "wnc-big-right 2.9s cubic-bezier(.4,.1,.7,1) forwards",
          }}
        >
          <span className="absolute right-[24px] top-[30px] h-4 w-6 rounded-full bg-white/25 blur-[2px]" />
        </div>
        <div
          className="absolute left-1/2 top-[10px] -ml-[15px] h-5 w-[30px] rounded-b-full bg-[#2f9e44]"
          style={{ animation: "wnc-big-leaf 2.9s ease-out forwards" }}
        />
        <span
          className="absolute left-1/2 top-1/2 h-[3px] w-[160px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-white shadow-[0_0_14px_4px_rgba(255,255,255,.9)]"
          style={{ animation: "wnc-big-knife 2.9s ease-in-out forwards" }}
        />
      </div>

      <span
        className="rounded-full bg-ink/90 px-4 py-2 text-[13px] font-medium text-white shadow-xl"
        style={{ animation: "wnc-big-text 2.9s ease-out forwards" }}
      >
        {text} 🍅
      </span>
    </div>
  );
}
