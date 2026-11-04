import React from "react";
import { cn } from "../../lib/cn";

export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...rest }, ref) => (
    <input
      ref={ref}
      className={cn(
        "h-8 w-full rounded-md border border-line bg-panel px-2.5 text-[13px] text-ink",
        "placeholder:text-ink-faint focus:border-accent focus:outline-none",
        className,
      )}
      {...rest}
    />
  ),
);
Input.displayName = "Input";

export const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.TextareaHTMLAttributes<HTMLTextAreaElement>
>(({ className, ...rest }, ref) => (
  <textarea
    ref={ref}
    className={cn(
      "w-full rounded-md border border-line bg-panel px-2.5 py-2 text-[13px] leading-relaxed text-ink",
      "placeholder:text-ink-faint focus:border-accent focus:outline-none",
      className,
    )}
    {...rest}
  />
));
Textarea.displayName = "Textarea";
