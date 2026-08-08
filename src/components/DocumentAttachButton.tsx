// 文档上传按钮：懒加载 pdfjs-dist / mammoth（仅在选中文档时 import），
// 抽取纯文本后通过 onDocument 回调交给 ChatView 管理附件，不注入输入框。
// pdfjs worker 已本地化到 public/pdfjs/（同 public/tesseract/ 的本地化先例），不走 CDN。
import { useRef, useState } from 'react';
import { Paperclip } from 'lucide-react';

const MAX_FILE_BYTES = 10 * 1024 * 1024;
/** 单文档抽取文本上限（字符数），超限截断并追加标记 */
const MAX_DOC_CHARS = 20000;
const TRUNCATE_MARK = '\n\n[文档已截断]';
/** pdfjs worker 本地路径（构建产物自 node_modules/pdfjs-dist/build/ 复制） */
const PDFJS_WORKER_SRC = '/pdfjs/pdf.worker.min.mjs';

export interface DocumentAttachment {
  fileName: string;
  content: string;
}

interface Props {
  /** 抽取完成回调（文本已截断至 2 万字符上限） */
  onDocument: (doc: DocumentAttachment) => void;
  disabled?: boolean;
}

type DocKind = 'text' | 'pdf' | 'docx';

/** 按扩展名判定文档类型；返回 null 表示不支持（.doc 单独给出降级提示） */
function detectKind(file: File): { kind: DocKind } | { legacyDoc: true } | null {
  const name = file.name.toLowerCase();
  if (name.endsWith('.txt') || name.endsWith('.md')) return { kind: 'text' };
  if (name.endsWith('.pdf')) return { kind: 'pdf' };
  if (name.endsWith('.docx')) return { kind: 'docx' };
  if (name.endsWith('.doc')) return { legacyDoc: true };
  return null;
}

/** 截断超长文本（保留 TRUNCATE_MARK 的额度在限额内） */
function truncateText(text: string): string {
  if (text.length <= MAX_DOC_CHARS) return text;
  return `${text.slice(0, MAX_DOC_CHARS - TRUNCATE_MARK.length)}${TRUNCATE_MARK}`;
}

/** .txt / .md：从已读入的字节解码为文本 */
function extractPlainText(buffer: ArrayBuffer): string {
  return new TextDecoder('utf-8').decode(buffer).trim();
}

/** .pdf：懒加载 pdfjs-dist 逐页 getTextContent 拼接（基于已读入的字节） */
async function extractPdfText(
  buffer: ArrayBuffer,
  onProgress: (page: number, total: number) => void,
): Promise<string> {
  const pdfjs = await import('pdfjs-dist');
  pdfjs.GlobalWorkerOptions.workerSrc = PDFJS_WORKER_SRC;

  const doc = await pdfjs.getDocument({ data: buffer }).promise;
  const parts: string[] = [];
  try {
    for (let pageNum = 1; pageNum <= doc.numPages; pageNum++) {
      onProgress(pageNum, doc.numPages);
      const page = await doc.getPage(pageNum);
      const { items } = await page.getTextContent();
      // TextItem 的 str 字段；换行由 \n 连接（丢失部分版式信息是可接受的）
      const pageText = items
        .map((item) => ('str' in item ? item.str : ''))
        .join(' ')
        .replace(/[ \t]+/g, ' ')
        .trim();
      if (pageText) parts.push(pageText);
    }
  } finally {
    await doc.destroy().catch(() => undefined);
  }
  return parts.join('\n\n');
}

/** .docx：懒加载 mammoth 抽取纯文本（基于已读入的字节） */
async function extractDocxText(buffer: ArrayBuffer): Promise<string> {
  const mammoth = await import('mammoth/mammoth.browser');
  const result = await mammoth.extractRawText({ arrayBuffer: buffer });
  return (result.value ?? '').trim();
}

export default function DocumentAttachButton({ onDocument, disabled }: Props) {
  const [running, setRunning] = useState(false);
  const [statusText, setStatusText] = useState('');
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  const closePanel = () => {
    setError('');
    setProgress(null);
    setStatusText('');
  };

  const processFile = async (file: File) => {
    if (running) return;

    const detected = detectKind(file);
    if (!detected) {
      setError('仅支持 .txt / .md / .pdf / .docx 文档');
      return;
    }
    if ('legacyDoc' in detected) {
      setError('暂不支持 .doc 格式，请另存为 .docx 或 PDF 后上传');
      return;
    }
    if (file.size > MAX_FILE_BYTES) {
      setError('文档超过 10MB 限制，请压缩或拆分后重试');
      return;
    }

    setError('');
    setProgress(null);
    setRunning(true);
    const started = performance.now();
    const kind = detected.kind;

    // 时序关键：先把全部字节读入内存，再立即清空 input（保持“可重复选择同一文件”语义）。
    // 若先清 value 再异步读文件，File 引用会失效并报读取权限错误，
    // 后续解析分支一律基于已取得的 buffer 工作。
    let buffer: ArrayBuffer;
    try {
      buffer = await file.arrayBuffer();
    } catch (err) {
      console.warn('[doc] read_failed', {
        fileName: file.name,
        error: err instanceof Error ? err.message : String(err),
      });
      setError('文件读取失败，请重试');
      setRunning(false);
      return;
    } finally {
      if (fileInputRef.current) fileInputRef.current.value = '';
    }

    try {
      let raw = '';
      if (kind === 'text') {
        setStatusText('正在读取文本...');
        raw = extractPlainText(buffer);
      } else if (kind === 'pdf') {
        setStatusText('正在解析 PDF...');
        raw = await extractPdfText(buffer, (page, total) => {
          setStatusText(`正在提取第 ${page}/${total} 页...`);
          setProgress(Math.round((page / total) * 100));
        });
      } else {
        setStatusText('正在解析 Word 文档...');
        raw = await extractDocxText(buffer);
      }

      const text = raw.replace(/\n{3,}/g, '\n\n').trim();
      if (text.length < 10) {
        // PDF 无文字层（扫描件）或文档几乎为空时的降级提示
        setError(
          kind === 'pdf'
            ? '未检测到文字层（可能是扫描件），请截图后用图片 OCR'
            : '未从文档中提取到有效文字',
        );
        return;
      }

      const content = truncateText(text);
      console.info('[doc] extract_done', {
        fileName: file.name,
        fileBytes: file.size,
        textLength: content.length,
        truncated: content.length < text.length,
        elapsedMs: Math.round(performance.now() - started),
      });
      onDocument({ fileName: file.name, content });
      closePanel();
    } catch (err) {
      console.warn('[doc] extract_failed', {
        fileName: file.name,
        error: err instanceof Error ? err.message : String(err),
      });
      setError('文档解析失败，请确认文件未损坏后重试');
    } finally {
      setRunning(false);
      setProgress(null);
    }
  };

  const panelVisible = running || Boolean(error);

  return (
    <div className="relative inline-flex">
      <input
        ref={fileInputRef}
        type="file"
        accept=".txt,.md,.pdf,.docx,.doc"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0];
          // 不在这里同步清空 value：processFile 会在读完字节后立即清空，
          // 避免先清 value 导致 File 引用失效（读取权限错误）
          if (file) void processFile(file);
        }}
      />
      <button
        type="button"
        title="上传文档（.txt / .md / .pdf / .docx）作为对话附件"
        className="inline-flex items-center justify-center rounded-full p-2 transition hover:bg-surface disabled:cursor-not-allowed disabled:opacity-50"
        disabled={disabled || running}
        onClick={() => fileInputRef.current?.click()}
      >
        <Paperclip size={18} aria-hidden="true" />
      </button>

      {panelVisible && (
        <div className="absolute bottom-full left-0 z-10 mb-2 w-64 rounded-lg border bg-card p-3 shadow-lg">
          <div className="flex items-start justify-between gap-2">
            <span className="text-xs font-medium text-text-secondary">
              {running ? statusText || '正在解析文档...' : '文档上传'}
            </span>
            {!running && (
              <button type="button" className="text-xs text-muted hover:text-text-secondary" onClick={closePanel}>
                关闭
              </button>
            )}
          </div>
          {progress !== null && (
            <div className="mt-2">
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary">
                <div className="h-full rounded-full bg-accent transition-all" style={{ width: `${progress}%` }} />
              </div>
              <p className="mt-1 text-xs text-text-secondary">提取进度 {progress}%</p>
            </div>
          )}
          {error && <p className="mt-2 text-xs text-danger">{error}</p>}
        </div>
      )}
    </div>
  );
}
