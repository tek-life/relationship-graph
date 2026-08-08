import { useMemo, useState } from 'react';
import {
  DuplicateInfo,
  FIELD_DEFS,
  ImportCommitResult,
  ImportPreviewResult,
  MAP_IGNORE,
  MAP_TO_NOTES,
  guessMapping,
  importCommit,
  importPreview,
  normalizeRows,
  parseSheetFile,
} from '../services/importer';
import type { CreatePersonInput } from '../types';

type Step = 'upload' | 'mapping' | 'preview' | 'done';

interface Props {
  onImported: () => void;
}

export default function ImportWizard({ onImported }: Props) {
  const [step, setStep] = useState<Step>('upload');
  const [fileName, setFileName] = useState('');
  const [headers, setHeaders] = useState<string[]>([]);
  const [rawRows, setRawRows] = useState<string[][]>([]);
  const [mapping, setMapping] = useState<string[]>([]);
  const [normalized, setNormalized] = useState<CreatePersonInput[]>([]);
  const [previewResult, setPreviewResult] = useState<ImportPreviewResult | null>(null);
  const [skipIndices, setSkipIndices] = useState<Set<number>>(new Set());
  const [commitResult, setCommitResult] = useState<ImportCommitResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const mappedNameCol = useMemo(() => mapping.includes('name'), [mapping]);

  const handleFile = async (file: File | undefined) => {
    if (!file) return;
    setError('');
    setBusy(true);
    try {
      const parsed = await parseSheetFile(file);
      if (parsed.headers.length === 0 || parsed.rows.length === 0) {
        throw new Error('文件为空或无法识别表头');
      }
      setFileName(file.name);
      setHeaders(parsed.headers);
      setRawRows(parsed.rows);
      setMapping(guessMapping(parsed.headers));
      setStep('mapping');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleMappingConfirm = async () => {
    setError('');
    if (!mappedNameCol) {
      setError('请至少将一列映射为「姓名」');
      return;
    }
    setBusy(true);
    try {
      const rows = normalizeRows(rawRows, mapping, headers);
      const result = await importPreview(rows);
      // 默认跳过：完全重复行 + 无效行
      const defaults = new Set<number>();
      result.duplicates.filter((d) => d.matchType === 'exact').forEach((d) => defaults.add(d.index));
      result.invalid.forEach((issue) => defaults.add(issue.index));
      setNormalized(rows);
      setPreviewResult(result);
      setSkipIndices(defaults);
      setStep('preview');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleCommit = async () => {
    setError('');
    setBusy(true);
    try {
      const result = await importCommit(normalized, Array.from(skipIndices));
      setCommitResult(result);
      setStep('done');
      onImported();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    setStep('upload');
    setFileName('');
    setHeaders([]);
    setRawRows([]);
    setMapping([]);
    setNormalized([]);
    setPreviewResult(null);
    setSkipIndices(new Set());
    setCommitResult(null);
    setError('');
  };

  const toggleSkip = (index: number) => {
    setSkipIndices((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  return (
    <div className="rounded-xl border bg-card p-6 shadow-sm">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-xl font-semibold">Excel 导入</h2>
        <div className="text-sm text-text-secondary">
          {['选择文件', '字段映射', '预览确认', '完成'].map((label, i) => {
            const stepIndex = ['upload', 'mapping', 'preview', 'done'].indexOf(step);
            return (
              <span key={label} className={i <= stepIndex ? 'font-medium text-accent' : ''}>
                {i > 0 && ' → '}
                {label}
              </span>
            );
          })}
        </div>
      </div>

      {error && <div className="mb-4 rounded bg-danger-light p-3 text-sm text-danger">{error}</div>}

      {step === 'upload' && (
        <div className="rounded-lg border-2 border-dashed border-line p-10 text-center">
          <p className="text-text-secondary">选择手捣的 Excel（.xlsx / .xls）或 CSV 文件</p>
          <p className="mt-1 text-sm text-muted">文件在浏览器内解析，原始文件不会上传</p>
          <label className="btn-primary mt-4 inline-block cursor-pointer">
            {busy ? '解析中...' : '选择文件'}
            <input
              type="file"
              accept=".xlsx,.xls,.csv"
              className="hidden"
              disabled={busy}
              onChange={(event) => handleFile(event.target.files?.[0])}
            />
          </label>
        </div>
      )}

      {step === 'mapping' && (
        <div>
          <p className="mb-3 text-sm text-text-secondary">
            已解析 <b>{fileName}</b>：共 {rawRows.length} 行。请确认每列对应的字段（已自动猜测）：
          </p>
          <div className="max-h-96 overflow-auto rounded border">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-secondary">
                <tr>
                  <th className="p-2 text-left">Excel 列</th>
                  <th className="p-2 text-left">样例（前3行）</th>
                  <th className="p-2 text-left">导入为</th>
                </tr>
              </thead>
              <tbody>
                {headers.map((header, col) => (
                  <tr key={col} className="border-t">
                    <td className="p-2 font-medium">{header || `第${col + 1}列`}</td>
                    <td className="p-2 text-text-secondary">
                      {rawRows.slice(0, 3).map((row) => row[col]).filter(Boolean).join(' / ') || '-'}
                    </td>
                    <td className="p-2">
                      <select
                        className="rounded border px-2 py-1"
                        value={mapping[col]}
                        onChange={(event) => {
                          const next = [...mapping];
                          next[col] = event.target.value;
                          setMapping(next);
                        }}
                      >
                        <option value={MAP_IGNORE}>忽略此列</option>
                        <option value={MAP_TO_NOTES}>并入备注</option>
                        {FIELD_DEFS.map((field) => (
                          <option key={field.key} value={field.key}>
                            {field.label}
                          </option>
                        ))}
                      </select>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="mt-4 flex gap-3">
            <button type="button" className="btn-primary" disabled={busy} onClick={handleMappingConfirm}>
              {busy ? '校验中...' : '下一步：预览与查重'}
            </button>
            <button type="button" className="rounded px-4 py-2 text-text-secondary hover:bg-surface" onClick={reset}>
              重新选择文件
            </button>
          </div>
        </div>
      )}

      {step === 'preview' && previewResult && (
        <div>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
            <StatCard label="总行数" value={previewResult.total} />
            <StatCard label="有效" value={previewResult.valid} tone="text-success" />
            <StatCard label="无效（将跳过）" value={previewResult.invalid.length} tone="text-danger" />
            <StatCard label="疑似重复" value={previewResult.duplicates.length} tone="text-warning" />
          </div>

          {previewResult.duplicates.length > 0 && (
            <div className="mb-4">
              <h3 className="mb-2 font-medium">疑似重复（勾选 = 跳过不导入）</h3>
              <div className="max-h-48 overflow-auto rounded border">
                <table className="w-full text-sm">
                  <tbody>
                    {previewResult.duplicates.map((dup: DuplicateInfo) => (
                      <tr key={dup.index} className="border-t">
                        <td className="p-2">
                          <input
                            type="checkbox"
                            checked={skipIndices.has(dup.index)}
                            onChange={() => toggleSkip(dup.index)}
                          />
                        </td>
                        <td className="p-2">第 {dup.index + 2} 行：{normalized[dup.index]?.name}</td>
                        <td className="p-2 text-text-secondary">
                          {dup.matchType === 'exact' ? '姓名+电话完全重复' : '姓名相同'}
                          （{dup.source === 'db' ? '与已有联系人' : '与本文件内其他行'}）
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {previewResult.invalid.length > 0 && (
            <div className="mb-4 rounded bg-danger-light p-3 text-sm text-danger">
              {previewResult.invalid.length} 行无效将自动跳过（如姓名为空）。行号：
              {previewResult.invalid.slice(0, 10).map((issue) => issue.index + 2).join('、')}
              {previewResult.invalid.length > 10 && ' 等'}
            </div>
          )}

          <p className="mb-3 text-sm text-text-secondary">
            将导入 <b>{normalized.length - skipIndices.size}</b> 条，跳过 <b>{skipIndices.size}</b> 条
          </p>
          <div className="flex gap-3">
            <button type="button" className="btn-primary" disabled={busy} onClick={handleCommit}>
              {busy ? '导入中...' : '开始导入'}
            </button>
            <button type="button" className="rounded px-4 py-2 text-text-secondary hover:bg-surface" onClick={() => setStep('mapping')}>
              返回调整映射
            </button>
          </div>
        </div>
      )}

      {step === 'done' && commitResult && (
        <div className="text-center">
          <p className="text-4xl">✅</p>
          <h3 className="mt-2 text-lg font-semibold">导入完成</h3>
          <p className="mt-2 text-text-secondary">
            成功导入 <b className="text-success">{commitResult.imported}</b> 条，
            跳过 {commitResult.skipped} 条，失败 {commitResult.failed.length} 条，
            耗时 {(commitResult.elapsedMs / 1000).toFixed(2)} 秒
          </p>
          {commitResult.failed.length > 0 && (
            <div className="mx-auto mt-3 max-w-md rounded bg-danger-light p-3 text-left text-sm text-danger">
              失败明细：
              {commitResult.failed.slice(0, 5).map((issue) => (
                <p key={issue.index}>第 {issue.index + 2} 行：{issue.reason}</p>
              ))}
            </div>
          )}
          <button type="button" className="btn-primary mt-4" onClick={reset}>
            继续导入其他文件
          </button>
        </div>
      )}
    </div>
  );
}

function StatCard({ label, value, tone }: { label: string; value: number; tone?: string }) {
  return (
    <div className="rounded-lg bg-secondary p-3 text-center">
      <p className={`text-2xl font-bold ${tone ?? ''}`}>{value}</p>
      <p className="text-sm text-text-secondary">{label}</p>
    </div>
  );
}
