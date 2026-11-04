import type { ReactNode } from "react";

const CLAIM = /⟦([^|⟧]+)\|([^⟧]*)⟧/g;
const INLINE = /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g;

function inlineFormat(text: string, keyBase: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let match: RegExpExecArray | null;
  let i = 0;
  INLINE.lastIndex = 0;
  while ((match = INLINE.exec(text))) {
    if (match.index > last) out.push(text.slice(last, match.index));
    const token = match[0];
    const key = `${keyBase}-i${i++}`;
    if (token.startsWith("**")) {
      out.push(<strong key={key}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("`")) {
      out.push(<code key={key}>{token.slice(1, -1)}</code>);
    } else {
      out.push(<em key={key}>{token.slice(1, -1)}</em>);
    }
    last = match.index + token.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function withClaims(text: string, onClaim: (id: string) => void, keyBase: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let match: RegExpExecArray | null;
  let i = 0;
  CLAIM.lastIndex = 0;
  while ((match = CLAIM.exec(text))) {
    if (match.index > last) {
      out.push(...inlineFormat(text.slice(last, match.index), `${keyBase}-t${i}`));
    }
    const id = match[1];
    const value = match[2];
    const isUnknown = id.startsWith("unknown");
    const key = `${keyBase}-c${i++}`;
    if (isUnknown) {
      out.push(
        <span key={key} style={{ color: "var(--red)" }} title="This claim has no receipt">
          {value}
        </span>
      );
    } else {
      out.push(
        <button
          type="button"
          key={key}
          className="claim"
          onClick={() => onClaim(id)}
          title={`Open receipt: ${id}`}
        >
          {value}
        </button>
      );
    }
    last = match.index + match[0].length;
  }
  if (last < text.length) out.push(...inlineFormat(text.slice(last), `${keyBase}-tail`));
  return out;
}

export function Markdown({
  markdown,
  onClaim,
}: {
  markdown: string;
  onClaim: (id: string) => void;
}) {
  const lines = markdown.split("\n");
  const blocks: ReactNode[] = [];
  let para: string[] = [];
  let list: string[] = [];
  let key = 0;

  const flushPara = () => {
    if (para.length) {
      const text = para.join(" ");
      blocks.push(<p key={`p${key++}`}>{withClaims(text, onClaim, `p${key}`)}</p>);
      para = [];
    }
  };
  const flushList = () => {
    if (list.length) {
      blocks.push(
        <ul key={`u${key++}`}>
          {list.map((item, idx) => (
            <li key={idx}>{withClaims(item, onClaim, `li${key}-${idx}`)}</li>
          ))}
        </ul>
      );
      list = [];
    }
  };
  const flush = () => {
    flushPara();
    flushList();
  };

  for (const raw of lines) {
    const line = raw.replace(/\s+$/, "");
    if (/^##\s+/.test(line)) {
      flush();
      blocks.push(<h2 key={`h${key++}`}>{line.replace(/^##\s+/, "")}</h2>);
    } else if (/^###\s+/.test(line)) {
      flush();
      blocks.push(<h3 key={`h3${key++}`}>{line.replace(/^###\s+/, "")}</h3>);
    } else if (/^[-*]\s+/.test(line)) {
      flushPara();
      list.push(line.replace(/^[-*]\s+/, ""));
    } else if (line.trim() === "") {
      flush();
    } else {
      flushList();
      para.push(line.trim());
    }
  }
  flush();
  return <div className="prose">{blocks}</div>;
}
