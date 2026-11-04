import { Eye, EyeOff, Lock, ShieldQuestion } from "lucide-react";
import { useEffect, useState } from "react";
import { errorText } from "../lib/api";
import { useSecret } from "../stores/secret";
import { Button } from "./ui/Button";
import { Modal } from "./ui/Modal";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

/**
 * 保密项目解锁 / 首次设置密码。
 *
 * - 没设过密码：录入密码 + 密码提示（提示必填，忘记密码时靠它提醒）
 * - 设过密码：只输入密码；连续输错 3 次在弹层里显示密码提示
 */
export function SecretDialog({ open, onOpenChange }: Props) {
  const status = useSecret((s) => s.status);
  const hint = useSecret((s) => s.hint);
  const lastError = useSecret((s) => s.lastError);
  const unlock = useSecret((s) => s.unlock);
  const savePassword = useSecret((s) => s.setPassword);

  const [password, setPassword] = useState("");
  const [hintInput, setHintInput] = useState("");
  const [confirm, setConfirm] = useState("");
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setPassword("");
      setHintInput("");
      setConfirm("");
      setError(null);
      setShow(false);
    }
  }, [open]);

  const setupMode = !status.configured;

  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      if (setupMode) {
        if (password.trim().length < 4) throw new Error("密码至少 4 位");
        if (password !== confirm) throw new Error("两次输入的密码不一致");
        if (!hintInput.trim()) throw new Error("密码提示必须填写");
        await savePassword(password, hintInput);
        onOpenChange(false);
      } else {
        const result = await unlock(password);
        if (result.ok) onOpenChange(false);
      }
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={setupMode ? "设置保密密码" : "解锁保密项目"}
      className="w-[420px]"
    >
      <div className="border-b border-line px-4 py-3">
        <p className="flex items-center gap-1.5 text-[13.5px] font-medium text-ink">
          <Lock size={14} className="text-accent" />
          {setupMode ? "设置保密密码" : "解锁保密项目"}
        </p>
      </div>
      <div className="space-y-3 px-4 py-3">
        <p className="flex items-start gap-1.5 text-[12px] leading-relaxed text-ink-muted">
          <Lock size={13} className="mt-0.5 shrink-0" />
          {setupMode
            ? "保密项目里的任务与记录会被隐藏，只有输入密码后才会显示。忘记密码只能重装/清库，请务必记住提示。"
            : "输入密码后显示保密项目。连续输错 3 次会给出密码提示。"}
        </p>

        <label className="block">
          <span className="text-[11.5px] text-ink-faint">{setupMode ? "设置密码" : "密码"}</span>
          <div className="mt-1 flex items-center gap-1">
            <input
              type={show ? "text" : "password"}
              value={password}
              autoFocus
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.nativeEvent.isComposing) void submit();
              }}
              className="h-8 w-full rounded-md border border-line bg-panel px-2 text-[13px] text-ink focus:border-accent focus:outline-none"
              placeholder={setupMode ? "至少 4 位" : "请输入密码"}
            />
            <button
              type="button"
              onClick={() => setShow((v) => !v)}
              className="rounded p-1.5 text-ink-faint hover:text-ink"
              title={show ? "隐藏" : "显示"}
            >
              {show ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
        </label>

        {setupMode && (
          <>
            <label className="block">
              <span className="text-[11.5px] text-ink-faint">再输入一次</span>
              <input
                type={show ? "text" : "password"}
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                className="mt-1 h-8 w-full rounded-md border border-line bg-panel px-2 text-[13px] text-ink focus:border-accent focus:outline-none"
              />
            </label>
            <label className="block">
              <span className="text-[11.5px] text-ink-faint">密码提示（3 次输错后显示，必填）</span>
              <input
                type="text"
                value={hintInput}
                onChange={(e) => setHintInput(e.target.value)}
                placeholder="例如：老家门牌号 + 生日"
                className="mt-1 h-8 w-full rounded-md border border-line bg-panel px-2 text-[13px] text-ink focus:border-accent focus:outline-none"
              />
            </label>
          </>
        )}

        {lastError && !setupMode && (
          <p className="text-[11.5px] text-danger">{lastError}</p>
        )}
        {hint && !setupMode && (
          <p className="flex items-start gap-1.5 rounded-md border border-warn/40 bg-warn/10 px-2 py-1.5 text-[12px] text-ink">
            <ShieldQuestion size={13} className="mt-0.5 shrink-0 text-warn" />
            <span>密码提示：{hint}</span>
          </p>
        )}
        {error && <p className="text-[11.5px] text-danger">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button variant="primary" size="sm" disabled={busy} onClick={() => void submit()}>
            {busy ? "处理中…" : setupMode ? "设置并解锁" : "解锁"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
