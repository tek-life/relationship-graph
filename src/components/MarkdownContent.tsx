import type { ReactNode } from 'react';

interface MarkdownContentProps {
  content: string;
  className?: string;
}

type Block =
  | { type: 'heading'; level: number; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'blockquote'; text: string }
  | { type: 'unordered-list'; items: string[] }
  | { type: 'ordered-list'; items: string[] }
  | { type: 'code'; text: string }
  | { type: 'spacer' };

export default function MarkdownContent({ content, className = '' }: MarkdownContentProps) {
  const blocks = parseMarkdown(content);

  return (
    <div className={className}>
      {blocks.map((block, index) => {
        switch (block.type) {
          case 'heading':
            return block.level === 1 ? (
              <h1 key={index} className="text-xl font-semibold leading-snug">
                {renderInline(block.text)}
              </h1>
            ) : block.level === 2 ? (
              <h2 key={index} className="text-lg font-semibold leading-snug">
                {renderInline(block.text)}
              </h2>
            ) : (
              <h3 key={index} className="text-base font-semibold leading-snug">
                {renderInline(block.text)}
              </h3>
            );
          case 'blockquote':
            return (
              <blockquote key={index} className="border-l-4 border-slate-300 pl-3 text-sm text-slate-600">
                {renderInline(block.text)}
              </blockquote>
            );
          case 'unordered-list':
            return (
              <ul key={index} className="list-disc space-y-1 pl-5">
                {block.items.map((item, itemIndex) => (
                  <li key={itemIndex} className="text-sm leading-6">
                    {renderInline(item)}
                  </li>
                ))}
              </ul>
            );
          case 'ordered-list':
            return (
              <ol key={index} className="list-decimal space-y-1 pl-5">
                {block.items.map((item, itemIndex) => (
                  <li key={itemIndex} className="text-sm leading-6">
                    {renderInline(item)}
                  </li>
                ))}
              </ol>
            );
          case 'code':
            return (
              <pre key={index} className="overflow-x-auto rounded-xl bg-slate-950 px-4 py-3 text-sm text-slate-100">
                <code>{block.text}</code>
              </pre>
            );
          case 'spacer':
            return <div key={index} className="h-2" />;
          default:
            return (
              <p key={index} className="text-sm leading-7">
                {renderInline((block as { text: string }).text)}
              </p>
            );
        }
      })}
    </div>
  );
}

function parseMarkdown(content: string): Block[] {
  const lines = content.replace(/\r\n/g, '\n').split('\n');
  const blocks: Block[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i].trimEnd();
    const trimmed = line.trim();

    if (!trimmed) {
      blocks.push({ type: 'spacer' });
      i += 1;
      continue;
    }

    if (trimmed.startsWith('```')) {
      const codeLines: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i += 1;
      }
      if (i < lines.length) i += 1;
      blocks.push({ type: 'code', text: codeLines.join('\n') });
      continue;
    }

    if (trimmed.startsWith('#')) {
      const level = Math.min(trimmed.match(/^#+/)?.[0].length ?? 1, 3);
      blocks.push({ type: 'heading', level, text: trimmed.replace(/^#+\s*/, '') });
      i += 1;
      continue;
    }

    if (trimmed.startsWith('>')) {
      blocks.push({ type: 'blockquote', text: trimmed.replace(/^>\s*/, '') });
      i += 1;
      continue;
    }

    if (/^[-*]\s+/.test(trimmed)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*]\s+/.test(lines[i].trim())) {
        items.push(lines[i].trim().replace(/^[-*]\s+/, ''));
        i += 1;
      }
      blocks.push({ type: 'unordered-list', items });
      continue;
    }

    if (/^\d+\.\s+/.test(trimmed)) {
      const items: string[] = [];
      while (i < lines.length && /^\d+\.\s+/.test(lines[i].trim())) {
        items.push(lines[i].trim().replace(/^\d+\.\s+/, ''));
        i += 1;
      }
      blocks.push({ type: 'ordered-list', items });
      continue;
    }

    const paragraphLines = [trimmed];
    i += 1;
    while (i < lines.length) {
      const next = lines[i].trim();
      if (!next || next.startsWith('#') || next.startsWith('>') || /^[-*]\s+/.test(next) || /^\d+\.\s+/.test(next) || next.startsWith('```')) {
        break;
      }
      paragraphLines.push(next);
      i += 1;
    }
    blocks.push({ type: 'paragraph', text: paragraphLines.join(' ') });
  }

  return blocks;
}

function renderInline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /(\[[^\]]+\]\([^)]+\)|`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*)/g;
  let lastIndex = 0;

  for (const match of text.matchAll(pattern)) {
    const token = match[0];
    const index = match.index ?? 0;
    if (index > lastIndex) {
      nodes.push(text.slice(lastIndex, index));
    }
    nodes.push(renderToken(token, nodes.length));
    lastIndex = index + token.length;
  }

  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }

  return nodes;
}

function renderToken(token: string, key: number): ReactNode {
  if (token.startsWith('[')) {
    const linkMatch = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (linkMatch) {
      return (
        <a key={key} href={linkMatch[2]} className="text-blue-600 underline underline-offset-2" target="_blank" rel="noreferrer">
          {linkMatch[1]}
        </a>
      );
    }
  }

  if (token.startsWith('**') && token.endsWith('**')) {
    return <strong key={key}>{token.slice(2, -2)}</strong>;
  }

  if (token.startsWith('*') && token.endsWith('*')) {
    return <em key={key}>{token.slice(1, -1)}</em>;
  }

  if (token.startsWith('`') && token.endsWith('`')) {
    return (
      <code key={key} className="rounded bg-slate-100 px-1.5 py-0.5 text-[0.9em] text-slate-800">
        {token.slice(1, -1)}
      </code>
    );
  }

  return token;
}
