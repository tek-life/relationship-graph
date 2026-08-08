// 图片 OCR 按钮：懒加载 tesseract.js（仅在选图/粘贴时 import），识别结果通过 onText 追加到输入框。
// worker 与 wasm core 已本地化到 public/tesseract/，traineddata 走 jsdelivr CDN（首次识别需联网，之后走缓存）。
import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { Camera } from 'lucide-react';
import { IconBtn } from './ui';

const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
const OCR_LANGS = 'chi_sim+eng';
const TESSERACT_PATHS = {
  workerPath: '/tesseract/worker.min.js',
  corePath: '/tesseract/core',
};

export interface ImageOcrHandle {
  /** 供输入框粘贴图片时调用 */
  processFile: (file: File) => void;
}

interface Props {
  onText: (text: string) => void;
  disabled?: boolean;
}

const ImageOcrButton = forwardRef<ImageOcrHandle | null, Props>(function ImageOcrButton({ onText, disabled }, ref) {
  const [previewUrl, setPreviewUrl] = useState('');
  const [progress, setProgress] = useState<number | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);
  const previewUrlRef = useRef('');

  useEffect(() => () => {
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
  }, []);

  const setPreview = (url: string) => {
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
    previewUrlRef.current = url;
    setPreviewUrl(url);
  };

  const closePanel = () => {
    setPreview('');
    setError('');
    setProgress(null);
  };

  const processFile = async (file: File) => {
    if (running) return;
    if (!file.type.startsWith('image/')) {
      setError('请选择图片文件');
      return;
    }
    if (file.size > MAX_IMAGE_BYTES) {
      setError('图片超过 10MB 限制，请压缩后重试');
      return;
    }

    // 时序关键：先把全部字节读入内存，再立即清空 input（保持“可重复选择同一文件”语义）。
    // 若先清 value 再异步读文件，File 引用会失效并报读取权限错误；
    // 后续预览与识别一律基于已取得的字节工作。
    let buffer: ArrayBuffer;
    try {
      buffer = await file.arrayBuffer();
    } catch {
      setError('图片读取失败，请重试');
      return;
    } finally {
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
    const imageBlob = new Blob([buffer], { type: file.type });

    setPreview(URL.createObjectURL(imageBlob));
    setError('');
    setProgress(0);
    setRunning(true);
    const started = performance.now();

    let createWorker: typeof import('tesseract.js').createWorker;
    try {
      ({ createWorker } = await import('tesseract.js'));
    } catch {
      setError('OCR 引擎加载失败，请检查网络或改用文字输入');
      setRunning(false);
      setProgress(null);
      return;
    }

    let worker: Awaited<ReturnType<typeof createWorker>> | null = null;
    try {
      worker = await createWorker(OCR_LANGS, 1, {
        ...TESSERACT_PATHS,
        logger: (m) => {
          if (m.status === 'recognizing text') {
            setProgress(Math.round(m.progress * 100));
          }
        },
      });
    } catch {
      setError('OCR 引擎加载失败，请检查网络或改用文字输入');
      setRunning(false);
      setProgress(null);
      return;
    }

    try {
      const { data } = await worker.recognize(imageBlob);
      const text = data.text.replace(/[ \t]+/g, ' ').trim();
      if (!text) {
        setError('未识别到文字，请更换更清晰的图片重新上传');
        return;
      }
      console.info('[ocr] recognize_done', {
        imageBytes: file.size,
        textLength: text.length,
        elapsedMs: Math.round(performance.now() - started),
      });
      onText(text);
      closePanel();
    } catch (err) {
      console.warn('[ocr] recognize_failed', {
        imageBytes: file.size,
        elapsedMs: Math.round(performance.now() - started),
        error: err instanceof Error ? err.message : String(err),
      });
      setError('图片识别失败，请重试或改用文字输入');
    } finally {
      setRunning(false);
      setProgress(null);
      await worker.terminate().catch(() => undefined);
    }
  };

  useImperativeHandle(ref, () => ({ processFile }));

  const panelVisible = Boolean(previewUrl || error);

  return (
    <div className="relative inline-flex">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0];
          // 不在这里同步清空 value：processFile 会在读完字节后立即清空，
          // 避免先清 value 导致 File 引用失效（读取权限错误）
          if (file) void processFile(file);
        }}
      />
      <IconBtn
        size="lg"
        title="上传图片识别文字（也可直接粘贴截图）"
        style={{ borderRadius: '9999px' }}
        disabled={disabled || running}
        onClick={() => fileInputRef.current?.click()}
      >
        <Camera size={18} aria-hidden="true" />
      </IconBtn>

      {panelVisible && (
        <div className="absolute bottom-full left-0 z-10 mb-2 w-64 rounded-lg border bg-card p-3 shadow-lg">
          <div className="flex items-start justify-between gap-2">
            <span className="text-xs font-medium text-text-secondary">{running ? '正在识别图片文字...' : '图片 OCR'}</span>
            {!running && (
              <button type="button" className="text-xs text-muted hover:text-text-secondary" onClick={closePanel}>
                关闭
              </button>
            )}
          </div>
          {previewUrl && (
            <img src={previewUrl} alt="待识别图片预览" className="mt-2 max-h-24 rounded border object-contain" />
          )}
          {progress !== null && (
            <div className="mt-2">
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary">
                <div className="h-full rounded-full bg-accent transition-all" style={{ width: `${progress}%` }} />
              </div>
              <p className="mt-1 text-xs text-text-secondary">识别进度 {progress}%</p>
            </div>
          )}
          {error && <p className="mt-2 text-xs text-danger">{error}</p>}
        </div>
      )}
    </div>
  );
});

export default ImageOcrButton;
