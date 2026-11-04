import React from "react";
import { cn } from "../../lib/cn";

type Variant = "primary" | "ghost" | "outline" | "danger";
type Size = "sm" | "md";

type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: Size;
};

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent text-white hover:brightness-110",
  ghost: "text-ink-muted hover:bg-panel-2 hover:text-ink",
  outline: "border border-line bg-panel text-ink hover:bg-panel-2",
  danger: "text-danger hover:bg-danger/10",
};

const SIZES: Record<Size, string> = {
  sm: "h-7 px-2 text-[12.5px]",
  md: "h-8 px-3 text-[13px]",
};

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = "ghost", size = "md", className, type = "button", ...rest }, ref) => (
    <button
      ref={ref}
      type={type}
      className={cn(
        "inline-flex select-none items-center justify-center gap-1.5 rounded-md font-medium transition-colors",
        "disabled:pointer-events-none disabled:opacity-50",
        SIZES[size],
        VARIANTS[variant],
        className,
      )}
      {...rest}
    />
  ),
);
Button.displayName = "Button";
