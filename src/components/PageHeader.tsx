import { PanelRightClose, PanelRightOpen } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "../lib/cn";
import { useUI } from "../stores/ui";

type Props = {
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
  showPanelToggle?: boolean;
};

export function PageHeader({ title, subtitle, actions, showPanelToggle = true }: Props) {
  const rightPanel = useUI((s) => s.rightPanel);
  const toggle = useUI((s) => s.toggleRightPanel);

  return (
    <header className="flex items-end justify-between gap-4 px-6 pb-2.5 pt-2.5">
      <div className="min-w-0">
        <h1 className="truncate text-[17px] font-semibold tracking-tight text-ink">{title}</h1>
        {subtitle && <div className="mt-0.5 text-[12px] text-ink-faint">{subtitle}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {actions}
        {showPanelToggle && (
          <button
            type="button"
            onClick={toggle}
            title={rightPanel ? "收起待办栏" : "展开待办栏"}
            className={cn("rounded p-1.5 text-ink-faint hover:bg-panel hover:text-ink")}
          >
            {rightPanel ? <PanelRightClose size={15} /> : <PanelRightOpen size={15} />}
          </button>
        )}
      </div>
    </header>
  );
}
