import React from "react";

const INLINE =
  /(\*\*[^*\n]+\*\*)|(`[^`\n]+`)|(\[[^\]\n]+\]\([^)\n]+\))|(\*[^*\n]+\*)/g;

function renderInline(text: string): React.ReactNode[] {
  const nodes: React.ReactNode[] = [];
  let last = 0;
  let match: RegExpExecArray | null;
  let i = 0;
  INLINE.lastIndex = 0;

  while ((match = INLINE.exec(text)) !== null) {
    if (match.index > last) nodes.push(text.slice(last, match.index));
    const token = match[0];

    if (token.startsWith("**")) {
      nodes.push(<strong key={`b${i++}`}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("`")) {
      nodes.push(<code key={`c${i++}`}>{token.slice(1, -1)}</code>);
    } else if (token.startsWith("[")) {
      const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(token);
      nodes.push(
        <a key={`a${i++}`} href={link?.[2]} target="_blank" rel="noreferrer">
          {link?.[1]}
        </a>,
      );
    } else {
      nodes.push(<em key={`i${i++}`}>{token.slice(1, -1)}</em>);
    }
    last = match.index + token.length;
  }

  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

/** 轻 Markdown：仅支持加粗、斜体、行内代码、链接、无序列表，不做 HTML 注入。 */
export function Markdown({ text }: { text: string }) {
  const lines = text.split("\n");
  const blocks: React.ReactNode[] = [];
  let listItems: string[] = [];
  let key = 0;

  const flushList = () => {
    if (listItems.length === 0) return;
    blocks.push(
      <ul key={`ul${key++}`}>
        {listItems.map((item, idx) => (
          <li key={idx}>{renderInline(item)}</li>
        ))}
      </ul>,
    );
    listItems = [];
  };

  for (const line of lines) {
    const listMatch = /^\s*[-*]\s+(.*)$/.exec(line);
    if (listMatch) {
      listItems.push(listMatch[1]);
      continue;
    }
    flushList();
    if (line.trim() === "") continue;
    blocks.push(<p key={`p${key++}`}>{renderInline(line)}</p>);
  }
  flushList();

  return <div className="entry-body">{blocks}</div>;
}

/** 搜索结果命中片段：\u0001 与 \u0002 之间为命中文字 */
export function HighlightedSnippet({ snippet }: { snippet: string }) {
  const parts = snippet.split(/[\u0001\u0002]/);
  return (
    <span>
      {parts.map((part, i) =>
        i % 2 === 1 ? (
          <mark
            key={i}
            className="rounded-[3px] bg-accent-soft px-0.5 text-ink"
          >
            {part}
          </mark>
        ) : (
          <React.Fragment key={i}>{part}</React.Fragment>
        ),
      )}
    </span>
  );
}
