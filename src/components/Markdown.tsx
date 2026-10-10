import { memo, useState, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import rehypeHighlight from "rehype-highlight";
import { Check, Copy } from "lucide-react";
import { api } from "../lib/api";

const remarkPlugins = [remarkGfm, remarkMath];
const rehypePlugins: never[] = [
  [rehypeKatex, { throwOnError: false, strict: false }] as never,
  [rehypeHighlight, { detect: false }] as never,
];

function textOf(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (node && typeof node === "object" && "props" in node) {
    return textOf((node as { props: { children?: ReactNode } }).props.children);
  }
  return "";
}

function CodeBlock({ children }: { children?: ReactNode }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(textOf(children));
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard can be unavailable; the code stays selectable.
    }
  };
  return (
    <pre>
      {children}
      <button type="button" className="icon-btn copy-code" aria-label={copied ? "Copied" : "Copy code"} onClick={copy}>
        {copied ? <Check aria-hidden /> : <Copy aria-hidden />}
      </button>
    </pre>
  );
}

const components: Components = {
  a: ({ href, children }) => (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        if (href && /^https:\/\//i.test(href)) void api.openExternal(href);
      }}
      title={href}
    >
      {children}
    </a>
  ),
  pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
  img: ({ alt }) => <span>[{alt || "image"}]</span>,
};

/** Model text: GitHub-flavored Markdown with KaTeX math and highlighted code. Raw HTML is not rendered. */
export const Markdown = memo(function Markdown({ text, streaming = false, className = "prose" }: { text: string; streaming?: boolean; className?: string }) {
  return (
    <div className={streaming ? `${className} cursor-blink` : className}>
      <ReactMarkdown remarkPlugins={remarkPlugins} rehypePlugins={rehypePlugins} components={components}>
        {text}
      </ReactMarkdown>
    </div>
  );
});
