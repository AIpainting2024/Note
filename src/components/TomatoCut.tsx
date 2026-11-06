import { useEffect, useState } from "react";

type Props = {
  /** 每次「专注完成」时把 token 换成新值即可重放动画 */
  token: number;
};

/**
 * 番茄钟结束动画（v0.8）：右下角一颗番茄被一刀切开，
 * 左右两半分开并下落淡出，约 2.5 秒后自动消失；无音效。
 *
 * 纯 CSS/SVG 实现，不引入依赖，也不拦截鼠标（pointer-events-none）。
 */
export function TomatoCut({ token }: Props) {
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    if (token === 0) return;
    setPlaying(true);
    const timer = window.setTimeout(() => setPlaying(false), 2600);
    return () => window.clearTimeout(timer);
  }, [token]);

  if (!playing) return null;

  return (
    <div
      key={token}
      aria-live="polite"
      className="pointer-events-none fixed bottom-6 right-6 z-[60] flex h-[132px] w-[168px] flex-col items-center justify-end"
    >
      <style>{`
        @keyframes wnc-tomato-in {
          0%   { opacity: 0; transform: scale(.55) translateY(10px); }
          22%  { opacity: 1; transform: scale(1.06) translateY(0); }
          34%  { transform: scale(1) translateY(0); }
          100% { opacity: 1; transform: scale(1) translateY(0); }
        }
        @keyframes wnc-knife {
          0%, 30%   { opacity: 0; transform: translate(-58px, -46px) rotate(38deg); }
          40%       { opacity: 1; transform: translate(-10px, -8px) rotate(38deg); }
          52%       { opacity: 1; transform: translate(58px, 46px) rotate(38deg); }
          58%, 100% { opacity: 0; transform: translate(70px, 56px) rotate(38deg); }
        }
        @keyframes wnc-half-left {
          0%, 52%   { transform: translate(0, 0) rotate(0deg); opacity: 1; }
          100%      { transform: translate(-40px, 46px) rotate(-26deg); opacity: 0; }
        }
        @keyframes wnc-half-right {
          0%, 52%   { transform: translate(0, 0) rotate(0deg); opacity: 1; }
          100%      { transform: translate(42px, 52px) rotate(26deg); opacity: 0; }
        }
        @keyframes wnc-leaf-out {
          0%, 48%   { opacity: 1; transform: translateY(0); }
          62%, 100% { opacity: 0; transform: translateY(-8px); }
        }
        @keyframes wnc-label-in {
          0%, 55%   { opacity: 0; transform: translateY(6px); }
          75%       { opacity: 1; transform: translateY(0); }
          100%      { opacity: 1; transform: translateY(0); }
        }
        @keyframes wnc-flash {
          0%, 44%   { opacity: 0; }
          50%       { opacity: .9; }
          60%, 100% { opacity: 0; }
        }
      `}</style>

      <div className="relative h-[86px] w-[120px]" style={{ animation: "wnc-tomato-in 2.6s ease-out forwards" }}>
        {/* 切开瞬间的白色闪光 */}
        <span
          className="absolute left-1/2 top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white blur-[2px]"
          style={{ animation: "wnc-flash 2.6s ease-out forwards" }}
        />

        {/* 左半颗 */}
        <div
          className="absolute left-1/2 top-3 -ml-[34px] h-[68px] w-[68px] rounded-full bg-[#e5484d] shadow-[inset_-6px_-8px_12px_rgba(0,0,0,.18)]"
          style={{ clipPath: "inset(0 50% 0 0)", animation: "wnc-half-left 2.6s cubic-bezier(.4,.1,.7,1) forwards" }}
        >
          <span className="absolute left-[14px] top-[16px] h-3 w-4 rounded-full bg-white/35 blur-[1px]" />
        </div>

        {/* 右半颗 */}
        <div
          className="absolute left-1/2 top-3 h-[68px] w-[68px] rounded-full bg-[#d13b41] shadow-[inset_6px_-8px_12px_rgba(0,0,0,.2)]"
          style={{ clipPath: "inset(0 0 0 50%)", animation: "wnc-half-right 2.6s cubic-bezier(.4,.1,.7,1) forwards" }}
        >
          <span className="absolute right-[14px] top-[18px] h-2.5 w-3 rounded-full bg-white/25 blur-[1px]" />
        </div>

        {/* 顶部绿蒂（切开后消失） */}
        <div
          className="absolute left-1/2 top-[6px] -ml-[9px] h-3 w-[18px] rounded-b-full bg-[#2f9e44]"
          style={{ animation: "wnc-leaf-out 2.6s ease-out forwards" }}
        />

        {/* 刀光 */}
        <span
          className="absolute left-1/2 top-1/2 h-[2px] w-[92px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-white shadow-[0_0_8px_2px_rgba(255,255,255,.85)]"
          style={{ animation: "wnc-knife 2.6s ease-in-out forwards" }}
        />
      </div>

      <span
        className="mt-1 rounded-full bg-ink/85 px-2.5 py-1 text-[11px] font-medium text-white shadow-lg"
        style={{ animation: "wnc-label-in 2.6s ease-out forwards" }}
      >
        番茄完成，休息一下 🍅
      </span>
    </div>
  );
}
