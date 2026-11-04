import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { api } from "./api";
import type { Attachment } from "./types";

/**
 * 图片附件的显示地址缓存。
 *
 * 正文里存的是 `att:<id>`，渲染时需要把 id 换成可显示的 URL（asset 协议）。
 * 缓存放在模块级，Markdown 渲染是同步的；数据到位后通过订阅触发重渲染。
 */
const cache = new Map<number, string>();
const listeners = new Set<() => void>();
let version = 0;

function notify() {
  version += 1;
  for (const listener of listeners) listener();
}

export function attachmentSrc(id: number): string | undefined {
  return cache.get(id);
}

/** Markdown 里用了这行，缓存更新时组件会重渲染 */
export function useAttachmentVersion(): number {
  const [local, setLocal] = useState(version);
  useEffect(() => {
    const listener = () => setLocal(version);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);
  return local;
}

export async function loadEntryAttachments(entryId: number): Promise<void> {
  try {
    const list = await api.listAttachments(entryId);
    let changed = false;
    for (const item of list) {
      const src = convertFileSrc(item.path);
      if (cache.get(item.id) !== src) {
        cache.set(item.id, src);
        changed = true;
      }
    }
    if (changed) notify();
  } catch {
    // 图片加载失败不影响文字显示
  }
}

/** File → base64（分段处理，避免大图爆栈） */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result ?? "");
      resolve(result.includes(",") ? result.slice(result.indexOf(",") + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("读取图片失败"));
    reader.readAsDataURL(file);
  });
}

/** 上传一张图片；entryId 为空表示还没保存的记录（走认领流程） */
export async function uploadImageFile(file: File, entryId: number | null): Promise<Attachment> {
  const mime = file.type || "image/png";
  const dataBase64 = await fileToBase64(file);
  const att = await api.saveAttachment(entryId, mime, dataBase64);
  cache.set(att.id, convertFileSrc(att.path));
  notify();
  return att;
}

/** 从粘贴事件里挑出第一张图片 */
export function imageFromClipboard(data: DataTransfer | null): File | null {
  if (!data) return null;
  for (const item of Array.from(data.items ?? [])) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) return file;
    }
  }
  for (const file of Array.from(data.files ?? [])) {
    if (file.type.startsWith("image/")) return file;
  }
  return null;
}

/** 把图片标记插到光标处（没有光标就追加到末尾） */
export function insertImageMarker(
  textarea: HTMLTextAreaElement | null,
  setText: (value: string) => void,
  current: string,
  attachmentId: number,
): string {
  const marker = `![](att:${attachmentId})`;
  if (!textarea) {
    const next = current ? `${current}\n${marker}` : marker;
    setText(next);
    return next;
  }
  const start = textarea.selectionStart ?? current.length;
  const end = textarea.selectionEnd ?? current.length;
  const before = current.slice(0, start);
  const after = current.slice(end);
  const prefix = before && !before.endsWith("\n") ? "\n" : "";
  const suffix = after.startsWith("\n") || !after ? "\n" : "\n";
  const next = `${before}${prefix}${marker}${suffix}${after}`;
  setText(next);
  const caret = before.length + prefix.length + marker.length + suffix.length;
  requestAnimationFrame(() => {
    textarea.focus();
    textarea.setSelectionRange(caret, caret);
  });
  return next;
}

/** 附件文件名友好展示（用于可能的错误提示） */
export function attachmentLabel(att: Attachment): string {
  const kb = Math.max(1, Math.round(att.byteSize / 1024));
  return `${att.mime} · ${kb} KB`;
}
