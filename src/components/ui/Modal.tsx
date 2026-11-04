import * as RadixDialog from "@radix-ui/react-dialog";
import React from "react";
import { cn } from "../../lib/cn";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
  /** 内容是否自动聚焦（搜索框用） */
};

export function Modal({ open, onOpenChange, title, description, children, className }: Props) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 z-40 bg-black/25 backdrop-blur-[1px]" />
        <RadixDialog.Content
          className={cn(
            "fixed left-1/2 top-[12vh] z-50 w-[640px] max-w-[92vw] -translate-x-1/2",
            "rounded-xl border border-line bg-panel shadow-2xl focus:outline-none",
            className,
          )}
        >
          <RadixDialog.Title className="sr-only">{title}</RadixDialog.Title>
          {description ? (
            <RadixDialog.Description className="sr-only">{description}</RadixDialog.Description>
          ) : (
            <RadixDialog.Description className="sr-only">{title}</RadixDialog.Description>
          )}
          {children}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
