import React, { useMemo } from "react";
import katex from "katex";

interface LatexPreviewProps {
  source: string;
}

type Segment =
  | { kind: "text"; content: string }
  | { kind: "inline_math"; content: string }
  | { kind: "display_math"; content: string }
  | { kind: "heading"; level: 1 | 2 | 3; content: string }
  | { kind: "list_item"; content: string };

const DISPLAY_ENVS = [
  "align\\*",
  "align",
  "equation\\*",
  "equation",
  "gather\\*",
  "gather",
  "multline\\*",
  "multline",
];

function renderMathHtml(tex: string, displayMode: boolean): string {
  try {
    return katex.renderToString(tex.trim(), {
      displayMode,
      throwOnError: false,
      strict: false,
      trust: false,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return `<span class="katex-fallback-error" title="${escapeHtml(msg)}">${escapeHtml(tex)}</span>`;
  }
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function renderInlineFragment(text: string): string {
  // Replace inline $...$ and \(...\) math, plus \textbf{}, \textit{}, \emph{}
  const tokenRe = /\\\(([\s\S]*?)\\\)|\$([^$\n]+?)\$|\\textbf\{([^}]*)\}|\\(?:textit|emph)\{([^}]*)\}|\\texttt\{([^}]*)\}/g;
  let out = "";
  let lastIdx = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenRe.exec(text)) !== null) {
    if (match.index > lastIdx) {
      out += escapeHtml(text.slice(lastIdx, match.index));
    }
    if (match[1] !== undefined) {
      out += renderMathHtml(match[1], false);
    } else if (match[2] !== undefined) {
      out += renderMathHtml(match[2], false);
    } else if (match[3] !== undefined) {
      out += `<strong>${escapeHtml(match[3])}</strong>`;
    } else if (match[4] !== undefined) {
      out += `<em>${escapeHtml(match[4])}</em>`;
    } else if (match[5] !== undefined) {
      out += `<code>${escapeHtml(match[5])}</code>`;
    }
    lastIdx = tokenRe.lastIndex;
  }

  if (lastIdx < text.length) {
    out += escapeHtml(text.slice(lastIdx));
  }

  return out.replace(/\\\\/g, "<br/>");
}

function parseLatexSegments(raw: string): Segment[] {
  // Strip outer \documentclass / \begin{document} if present for preview readability
  let body = raw;
  const docMatch = /\\begin\{document\}([\s\S]*?)\\end\{document\}/.exec(body);
  if (docMatch) {
    body = docMatch[1];
  }

  const envPattern = DISPLAY_ENVS.join("|");
  const blockRe = new RegExp(
    `(\\$\\$[\\s\\S]*?\\$\\$|\\\\\\[[\\s\\S]*?\\\\\\]|\\\\begin\\{(?:${envPattern})\\}[\\s\\S]*?\\\\end\\{(?:${envPattern})\\})`,
    "g"
  );

  const segments: Segment[] = [];
  const chunks = body.split(blockRe);

  for (const chunk of chunks) {
    if (!chunk) continue;
    const trimmed = chunk.trim();
    if (!trimmed) continue;

    if (trimmed.startsWith("$$") && trimmed.endsWith("$$")) {
      segments.push({ kind: "display_math", content: trimmed.slice(2, -2) });
    } else if (trimmed.startsWith("\\[") && trimmed.endsWith("\\]")) {
      segments.push({ kind: "display_math", content: trimmed.slice(2, -2) });
    } else if (trimmed.startsWith("\\begin{")) {
      segments.push({ kind: "display_math", content: trimmed });
    } else {
      // Parse line-oriented headings & list items inside text blocks
      const lines = chunk.split(/\r?\n/);
      let paragraphBuf: string[] = [];

      const flushPara = () => {
        const joined = paragraphBuf.join("\n").trim();
        if (joined) {
          segments.push({ kind: "text", content: joined });
        }
        paragraphBuf = [];
      };

      for (const line of lines) {
        const t = line.trim();
        if (!t) {
          flushPara();
          continue;
        }
        if (t === "\\begin{itemize}" || t === "\\end{itemize}" || t === "\\begin{enumerate}" || t === "\\end{enumerate}" || t === "\\maketitle") {
          flushPara();
          continue;
        }
        const sec = /^\\section\*?\{([^}]*)\}$/.exec(t);
        if (sec) {
          flushPara();
          segments.push({ kind: "heading", level: 1, content: sec[1] });
          continue;
        }
        const subsec = /^\\subsection\*?\{([^}]*)\}$/.exec(t);
        if (subsec) {
          flushPara();
          segments.push({ kind: "heading", level: 2, content: subsec[1] });
          continue;
        }
        const subsubsec = /^\\subsubsection\*?\{([^}]*)\}$/.exec(t);
        if (subsubsec) {
          flushPara();
          segments.push({ kind: "heading", level: 3, content: subsubsec[1] });
          continue;
        }
        const item = /^\\item\s+(.*)$/.exec(t);
        if (item) {
          flushPara();
          segments.push({ kind: "list_item", content: item[1] });
          continue;
        }
        paragraphBuf.push(t);
      }
      flushPara();
    }
  }

  return segments;
}

export const LatexPreview: React.FC<LatexPreviewProps> = ({ source }) => {
  const segments = useMemo(() => parseLatexSegments(source), [source]);

  if (!source.trim()) {
    return (
      <div className="preview-empty">
        Converted LaTeX will render live here as formatted math and text.
      </div>
    );
  }

  return (
    <div className="latex-preview-paper">
      {segments.map((seg, idx) => {
        if (seg.kind === "display_math") {
          return (
            <div
              key={idx}
              className="latex-display-block"
              dangerouslySetInnerHTML={{
                __html: renderMathHtml(seg.content, true),
              }}
            />
          );
        }
        if (seg.kind === "heading") {
          const Tag = seg.level === 1 ? "h2" : seg.level === 2 ? "h3" : "h4";
          return (
            <Tag
              key={idx}
              className={`latex-heading-${seg.level}`}
              dangerouslySetInnerHTML={{
                __html: renderInlineFragment(seg.content),
              }}
            />
          );
        }
        if (seg.kind === "list_item") {
          return (
            <div key={idx} className="latex-list-item">
              <span className="latex-bullet">•</span>
              <span
                dangerouslySetInnerHTML={{
                  __html: renderInlineFragment(seg.content),
                }}
              />
            </div>
          );
        }
        return (
          <p
            key={idx}
            className="latex-paragraph"
            dangerouslySetInnerHTML={{
              __html: renderInlineFragment(seg.content),
            }}
          />
        );
      })}
    </div>
  );
};
