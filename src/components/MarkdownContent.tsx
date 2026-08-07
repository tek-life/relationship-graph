/**
 * Markdown 渲染组件
 * 基于 react-markdown + remark-gfm（支持表格/删除线/任务列表）+ 代码语法高亮。
 * 保持原有 props 签名（content / className）完全兼容。
 */
import { useState, type ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import oneLight from 'react-syntax-highlighter/dist/esm/styles/prism/one-light';

interface MarkdownContentProps {
  content: string;
  className?: string;
}

export default function MarkdownContent({ content, className = '' }: MarkdownContentProps) {
  return (
    <div className={`markdown-body ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1: ({ children }) => (
            <h1 className="mt-4 mb-2 text-xl font-semibold leading-snug first:mt-0">{children}</h1>
          ),
          h2: ({ children }) => (
            <h2 className="mt-4 mb-2 text-lg font-semibold leading-snug first:mt-0">{children}</h2>
          ),
          h3: ({ children }) => (
            <h3 className="mt-3 mb-1.5 text-base font-semibold leading-snug first:mt-0">{children}</h3>
          ),
          h4: ({ children }) => (
            <h4 className="mt-3 mb-1.5 text-sm font-semibold leading-snug first:mt-0">{children}</h4>
          ),
          p: ({ children }) => <p className="my-2 leading-7 first:mt-0 last:mb-0">{children}</p>,
          blockquote: ({ children }) => (
            <blockquote className="my-3 border-l-4 border-slate-300 pl-3 text-sm text-slate-600">
              {children}
            </blockquote>
          ),
          ul: ({ children }) => <ul className="my-2 list-disc space-y-1 pl-5">{children}</ul>,
          ol: ({ children }) => <ol className="my-2 list-decimal space-y-1 pl-5">{children}</ol>,
          li: ({ children }) => <li className="leading-6">{children}</li>,
          a: ({ href, children }) => (
            <a
              href={href}
              className="text-blue-600 underline underline-offset-2"
              target="_blank"
              rel="noreferrer"
            >
              {children}
            </a>
          ),
          hr: () => <hr className="my-4 border-slate-200" />,
          img: ({ src, alt }) => (
            <img src={src} alt={alt ?? ''} className="my-3 max-w-full rounded-xl border border-slate-200" />
          ),
          table: ({ children }) => (
            <div className="my-3 overflow-x-auto">
              <table className="w-full border-collapse text-sm">{children}</table>
            </div>
          ),
          thead: ({ children }) => <thead className="bg-slate-50">{children}</thead>,
          th: ({ children }) => (
            <th className="border border-slate-200 px-3 py-1.5 text-left font-semibold">{children}</th>
          ),
          td: ({ children }) => (
            <td className="border border-slate-200 px-3 py-1.5 align-top">{children}</td>
          ),
          code: ({ className: codeClassName, children, ...rest }) => {
            const text = String(children ?? '').replace(/\n$/, '');
            const match = /language-([\w-]+)/.exec(codeClassName ?? '');
            // 块级代码：带语言标记，或内容含换行
            const isBlock = Boolean(match) || text.includes('\n');
            if (isBlock) {
              return <CodeBlock language={match?.[1]} code={text} />;
            }
            return (
              <code
                className="rounded bg-slate-100 px-1.5 py-0.5 text-[0.9em] text-slate-800"
                {...rest}
              >
                {children}
              </code>
            );
          },
          pre: ({ children }) => <>{children as ReactNode}</>,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}

/** 带语言标签与"复制"按钮的高亮代码块（亮色主题，与整体 UI 协调） */
function CodeBlock({ language, code }: { language?: string; code: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板不可用时静默失败
    }
  };

  return (
    <div className="my-3 overflow-hidden rounded-xl border border-slate-200">
      <div className="flex items-center justify-between border-b border-slate-200 bg-slate-100/80 px-3 py-1.5">
        <span className="text-xs font-medium uppercase tracking-wide text-slate-500">
          {language ?? 'text'}
        </span>
        <button
          type="button"
          className="rounded-md px-2 py-0.5 text-xs text-slate-500 transition hover:bg-slate-200 hover:text-slate-700"
          onClick={handleCopy}
        >
          {copied ? '已复制' : '复制'}
        </button>
      </div>
      <SyntaxHighlighter
        language={language ?? 'text'}
        style={oneLight}
        customStyle={{ margin: 0, borderRadius: 0, fontSize: '0.85rem', lineHeight: 1.6 }}
        PreTag="div"
      >
        {code}
      </SyntaxHighlighter>
    </div>
  );
}
