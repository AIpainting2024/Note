import * as RadixCheckbox from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { cn } from "../../lib/cn";

type Props = {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  className?: string;
  ariaLabel?: string;
  size?: "sm" | "md";
};

export function Checkbox({ checked, onCheckedChange, className, ariaLabel, size = "md" }: Props) {
  return (
    <RadixCheckbox.Root
      checked={checked}
      onCheckedChange={(v) => onCheckedChange(v === true)}
      aria-label={ariaLabel}
      className={cn(
        "flex shrink-0 items-center justify-center rounded-[5px] border transition-colors",
        size === "sm" ? "h-4 w-4" : "h-[17px] w-[17px]",
        checked
          ? "border-accent bg-accent text-white"
          : "border-line bg-panel hover:border-ink-faint",
        className,
      )}
    >
      <RadixCheckbox.Indicator>
        <Check size={12} strokeWidth={3} />
      </RadixCheckbox.Indicator>
    </RadixCheckbox.Root>
  );
}
