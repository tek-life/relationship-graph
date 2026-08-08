// 技能包管理：列表、zip 导入（fflate 前端解压 + 导入前审阅预检）、
// 包详情（文件树 + Markdown 预览）、整包删除。
// 后端契约：/api/admin/skill-packages（camelCase），导入前先在浏览器端解压并预检，
// admin 确认后才 POST /import；失败展示后端中文错误（ErrorBanner）。

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { unzipSync } from 'fflate';
import { apiDelete, apiGet, apiPost } from '../../services/api';
import {
  analyzeSkillPackage,
  extractPackageFileList,
  formatCharCount,
  PACKAGE_LIMITS,
  parseFrontmatter,
  ZIP_LIMITS,
} from '../../services/skillDoc';
import type { SkillPackagePreview } from '../../services/skillDoc';
import MarkdownContent from '../MarkdownContent';
import { AdminPageHeader, ConfirmDialog, EmptyState, ErrorBanner, LoadingSpinner, StatusBadge } from './shared';
import type { ImportSkillPackageResponse, SkillPackage, SkillPackageFile } from './types';

/** 来源徽标 */
function SourceKindBadge({ kind }: { kind: SkillPackage['sourceKind'] }) {
  const imported = kind === 'imported';
  return (
    <span
      className="badge"
      style={{
        backgroundColor: imported ? 'var(--accent-light)' : 'var(--surface-hover)',
        color: imported ? 'var(--accent-color)' : 'var(--text-muted)',
      }}
    >
      {imported ? '导入' : '内联'}
    </span>
  );
}

/** zip 解压后的导入草稿（待 admin 审阅） */
interface ImportDraft {
  fileName: string;
  files: { relPath: string; content: string }[];
  preview: SkillPackagePreview;
}

export default function SkillPackageManager() {
  const [packages, setPackages] = useState<SkillPackage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  // 各包文件数（列表接口不含文件，打开详情后回填）
  const [fileCountById, setFileCountById] = useState<Record<string, number>>({});

  // zip 导入草稿（解压预检后、确认 POST 前）
  const [draft, setDraft] = useState<ImportDraft | null>(null);
  const [importName, setImportName] = useState('');
  const [importing, setImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 包详情（点击行展开）
  const [detailId, setDetailId] = useState<string | null>(null);

  // 删除确认
  const [deleteTarget, setDeleteTarget] = useState<SkillPackage | null>(null);

  const fetchPackages = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const list = await apiGet<SkillPackage[]>('/api/admin/skill-packages');
      setPackages(list);
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPackages();
  }, [fetchPackages]);

  /** zip 文件选择：体积防护 → fflate 浏览器端解压 → 解压字节防护 → 文本过滤 → 预检 → 展示审阅清单 */
  const handleZipSelect = async (file: File) => {
    setError('');
    setDraft(null);
    setImportName('');
    // zip 炸弹防护①：zip 文件本身超限直接报错，不解压
    if (file.size > ZIP_LIMITS.maxZipBytes) {
      setError(
        `zip 文件过大（${(file.size / 1024 / 1024).toFixed(1)}MB），上限 ${ZIP_LIMITS.maxZipBytes / 1024 / 1024}MB`,
      );
      if (fileInputRef.current) fileInputRef.current.value = '';
      return;
    }
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const entries = unzipSync(bytes);
      // zip 炸弹防护②：按条目累计解压后原始字节，超限中止（与后端 body limit 口径一致）
      let totalBytes = 0;
      for (const [name, data] of Object.entries(entries)) {
        if (name.endsWith('/')) continue;
        totalBytes += data.byteLength;
        if (totalBytes > ZIP_LIMITS.maxUncompressedBytes) {
          setError(
            `解压后总字节超限（上限 ${ZIP_LIMITS.maxUncompressedBytes / 1024 / 1024}MB），疑似 zip 炸弹或超大包，已中止导入`,
          );
          if (fileInputRef.current) fileInputRef.current.value = '';
          return;
        }
      }
      const decoder = new TextDecoder('utf-8');
      const decoded = Object.entries(entries)
        .filter(([name]) => !name.endsWith('/'))
        .map(([name, data]) => ({ relPath: name, content: decoder.decode(data) }));
      const files = extractPackageFileList(decoded);
      if (files.length === 0) {
        setError('zip 内未找到可读的文本文件（.md/.txt 等）；目录与二进制文件已被自动跳过。');
        return;
      }
      setDraft({ fileName: file.name, files, preview: analyzeSkillPackage(files) });
    } catch (err) {
      setError(`zip 解压失败：${String(err instanceof Error ? err.message : err)}`);
    }
  };

  /** 审阅确认后 POST /import */
  const confirmImport = async () => {
    if (!draft) return;
    setImporting(true);
    setError('');
    try {
      const body = {
        ...(importName.trim() ? { name: importName.trim() } : {}),
        files: Object.fromEntries(draft.files.map((f) => [f.relPath, f.content])),
      };
      await apiPost<ImportSkillPackageResponse>('/api/admin/skill-packages/import', body);
      setDraft(null);
      setImportName('');
      if (fileInputRef.current) fileInputRef.current.value = '';
      await fetchPackages();
    } catch (err) {
      // 后端 400 校验失败时携带中文原因，直接展示
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setImporting(false);
    }
  };

  const cancelImport = () => {
    setDraft(null);
    setImportName('');
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await apiDelete(`/api/admin/skill-packages/${deleteTarget.id}`);
      setDeleteTarget(null);
      if (detailId === deleteTarget.id) setDetailId(null);
      await fetchPackages();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
      setDeleteTarget(null);
    }
  };

  /** 详情加载完成后回填文件数到列表 */
  const handleDetailLoaded = (pkg: SkillPackage) => {
    if (pkg.files) {
      setFileCountById((prev) => ({ ...prev, [pkg.id]: pkg.files!.length }));
    }
  };

  if (loading) return <LoadingSpinner text="正在加载技能包列表…" />;

  return (
    <div className="space-y-4">
      {error && <ErrorBanner message={error} />}

      {/* 统一页头范式：标题 + 描述 + 主操作区（zip 导入入口） */}
      <AdminPageHeader
        title="技能包"
        description={`导入与管理多文件技能包，可绑定到数字人（当前 ${packages.length} 个）`}
        actions={
          <label className="btn-primary cursor-pointer">
            导入 zip 技能包
            <input
              ref={fileInputRef}
              type="file"
              accept=".zip"
              className="hidden"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) handleZipSelect(f);
              }}
            />
          </label>
        }
      />

      <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
        上限：文件数 ≤ {PACKAGE_LIMITS.maxFiles}、单文件 ≤ 200KB、总字符 ≤{' '}
        {formatCharCount(PACKAGE_LIMITS.maxTotalChars)}；包必须含根 SKILL.md 且 frontmatter 含非空
        name/description。
      </p>

      {/* zip 导入审阅面板 */}
      {draft && (
        <ImportReviewPanel
          draft={draft}
          importName={importName}
          onImportNameChange={setImportName}
          importing={importing}
          onConfirm={confirmImport}
          onCancel={cancelImport}
        />
      )}

      {/* 技能包列表 */}
      {packages.length === 0 ? (
        <EmptyState text="暂无技能包，点击右上角「导入 zip 技能包」上传。" />
      ) : (
        <div className="overflow-hidden rounded-xl border" style={{ borderColor: 'var(--border-color)' }}>
          <table className="w-full text-sm">
            <thead>
              <tr style={{ backgroundColor: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
                <th className="px-3 py-2 text-left font-medium">名称</th>
                <th className="px-3 py-2 text-left font-medium">来源</th>
                <th className="px-3 py-2 text-left font-medium">文件数</th>
                <th className="px-3 py-2 text-left font-medium">总字符</th>
                <th className="px-3 py-2 text-left font-medium">状态</th>
                <th className="px-3 py-2 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {packages.map((pkg) => (
                <PackageRow
                  key={pkg.id}
                  pkg={pkg}
                  fileCount={pkg.files?.length ?? fileCountById[pkg.id]}
                  expanded={detailId === pkg.id}
                  onToggleDetail={() => setDetailId(detailId === pkg.id ? null : pkg.id)}
                  onDelete={() => setDeleteTarget(pkg)}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* 包详情 */}
      {detailId && (
        <PackageDetailPanel packageId={detailId} onLoaded={handleDetailLoaded} />
      )}

      {/* 删除确认弹窗 */}
      {deleteTarget && (
        <ConfirmDialog
          title="删除技能包"
          message={`确认删除技能包「${deleteTarget.displayName}」？已绑定该包的数字人将自动解绑，此操作不可恢复。`}
          danger
          confirmLabel="删除"
          onConfirm={handleDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}

/** 列表单行（可展开详情） */
function PackageRow({
  pkg,
  fileCount,
  expanded,
  onToggleDetail,
  onDelete,
}: {
  pkg: SkillPackage;
  fileCount: number | undefined;
  expanded: boolean;
  onToggleDetail: () => void;
  onDelete: () => void;
}) {
  return (
    <tr style={{ borderTop: '1px solid var(--border-color)' }}>
      <td className="px-3 py-2">
        <div style={{ color: 'var(--text-primary)' }}>
          <span className="font-medium">{pkg.displayName}</span>
          {pkg.description && (
            <p className="mt-0.5 truncate text-xs" style={{ color: 'var(--text-muted)' }} title={pkg.description}>
              {pkg.description}
            </p>
          )}
        </div>
      </td>
      <td className="px-3 py-2">
        <SourceKindBadge kind={pkg.sourceKind} />
      </td>
      <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
        {fileCount ?? '—'}
      </td>
      <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
        {formatCharCount(pkg.totalChars)}
      </td>
      <td className="px-3 py-2">
        <StatusBadge active={pkg.isActive} />
      </td>
      <td className="px-3 py-2">
        <div className="flex gap-2">
          <button
            type="button"
            className="text-xs transition hover:underline"
            style={{ color: 'var(--accent-color)' }}
            onClick={onToggleDetail}
          >
            {expanded ? '收起详情' : '详情'}
          </button>
          <button
            type="button"
            className="text-xs transition hover:underline"
            style={{ color: 'var(--danger-color)' }}
            onClick={onDelete}
          >
            删除
          </button>
        </div>
      </td>
    </tr>
  );
}

/** zip 导入审阅面板：文件清单 + 预检标记，确认后才真正提交 */
function ImportReviewPanel({
  draft,
  importName,
  onImportNameChange,
  importing,
  onConfirm,
  onCancel,
}: {
  draft: ImportDraft;
  importName: string;
  onImportNameChange: (v: string) => void;
  importing: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { preview } = draft;
  return (
    <div
      className="space-y-3 rounded-xl border p-4"
      style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
    >
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          导入审阅：{draft.fileName}
        </h4>
        <span className="text-xs" style={{ color: 'var(--text-muted)' }}>
          共 {preview.fileCount} 个文本文件 · 总字符 {formatCharCount(preview.totalChars)}
        </span>
      </div>

      {/* 预检状态 */}
      <div className="flex flex-wrap gap-2 text-xs">
        <PreviewBadge
          ok={preview.hasRootSkillMd}
          okText={`已找到根 SKILL.md（${preview.rootSkillMdRelPath}）`}
          failText={
            preview.rootSkillMdAmbiguous
              ? '最浅层存在多个根 SKILL.md 候选，请只保留一个'
              : '未找到根 SKILL.md（文件名大小写不敏感，取最浅层）'
          }
        />
        {preview.frontmatterError && (
          <PreviewBadge ok={false} okText="" failText={preview.frontmatterError} />
        )}
        <PreviewBadge
          ok={!preview.overFileLimit}
          okText={`文件数 ${preview.fileCount} / ${PACKAGE_LIMITS.maxFiles}`}
          failText={`文件数超限：${preview.fileCount} > ${PACKAGE_LIMITS.maxFiles}`}
        />
        <PreviewBadge
          ok={!preview.overCharLimit}
          okText={`总字符 ${formatCharCount(preview.totalChars)} / ${formatCharCount(PACKAGE_LIMITS.maxTotalChars)}`}
          failText={`总字符超限：${formatCharCount(preview.totalChars)} > ${formatCharCount(PACKAGE_LIMITS.maxTotalChars)}`}
        />
        {preview.oversizedFiles.map((p) => (
          <PreviewBadge key={p} ok={false} okText="" failText={`单文件超限：${p}`} />
        ))}
      </div>

      {/* 文件清单 */}
      <div
        className="max-h-56 overflow-y-auto rounded-lg border text-xs"
        style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)' }}
      >
        {draft.files.map((f) => (
          <div
            key={f.relPath}
            className="flex items-center justify-between border-b px-3 py-1.5 last:border-b-0"
            style={{ borderColor: 'var(--border-color)' }}
          >
            <span
              className="truncate font-mono"
              style={{
                color:
                  f.relPath === preview.rootSkillMdRelPath ? 'var(--accent-color)' : 'var(--text-primary)',
              }}
              title={f.relPath}
            >
              {f.relPath}
              {f.relPath === preview.rootSkillMdRelPath && ' ← 根 SKILL.md'}
            </span>
            <span className="ml-3 shrink-0" style={{ color: 'var(--text-muted)' }}>
              {formatCharCount(f.content.length)} 字符
            </span>
          </div>
        ))}
      </div>

      {/* 包名（可选） */}
      <div>
        <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
          技能包显示名称（可选，默认取根 SKILL.md frontmatter name）
        </label>
        <input
          className="input"
          value={importName}
          onChange={(e) => onImportNameChange(e.target.value)}
          placeholder="如：联系人管家技能包"
        />
      </div>

      <div className="flex gap-2">
        <button
          type="button"
          className="btn-primary text-xs"
          disabled={importing || preview.hasBlockingIssue}
          title={preview.hasBlockingIssue ? '存在预检失败项，无法导入' : undefined}
          onClick={onConfirm}
        >
          {importing ? '导入中…' : '确认导入'}
        </button>
        <button type="button" className="btn-secondary text-xs" onClick={onCancel} disabled={importing}>
          取消
        </button>
      </div>
    </div>
  );
}

/** 预检徽标：通过（绿调）/失败（红调） */
function PreviewBadge({ ok, okText, failText }: { ok: boolean; okText: string; failText: string }) {
  return (
    <span
      className="badge"
      style={{
        backgroundColor: ok ? 'var(--accent-light)' : 'var(--danger-color)',
        color: ok ? 'var(--accent-color)' : '#fff',
      }}
    >
      {ok ? okText : failText}
    </span>
  );
}

/** 包详情：文件树列表 + 点选文件 Markdown 预览 */
function PackageDetailPanel({
  packageId,
  onLoaded,
}: {
  packageId: string;
  onLoaded: (pkg: SkillPackage) => void;
}) {
  const [pkg, setPkg] = useState<SkillPackage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [selectedRelPath, setSelectedRelPath] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      setError('');
      try {
        const detail = await apiGet<SkillPackage>(`/api/admin/skill-packages/${packageId}`);
        if (cancelled) return;
        setPkg(detail);
        const files = detail.files ?? [];
        // 默认选中根 SKILL.md，否则选中第一个文件
        const skillMd = files.find((f) => f.relPath === 'SKILL.md');
        setSelectedRelPath((skillMd ?? files[0])?.relPath ?? null);
        onLoaded(detail);
      } catch (err) {
        if (!cancelled) setError(String(err instanceof Error ? err.message : err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
    // onLoaded 仅作回传回调，不参与依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [packageId]);

  const files = useMemo(() => pkg?.files ?? [], [pkg]);
  const selected = useMemo(
    () => files.find((f) => f.relPath === selectedRelPath) ?? null,
    [files, selectedRelPath],
  );

  if (loading) return <LoadingSpinner text="正在加载技能包详情…" />;
  if (error) return <ErrorBanner message={error} />;
  if (!pkg) return null;

  return (
    <div
      className="rounded-xl border p-4"
      style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
    >
      <div className="mb-3 flex items-center gap-2">
        <h4 className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          {pkg.displayName}
        </h4>
        <SourceKindBadge kind={pkg.sourceKind} />
        <span className="text-xs" style={{ color: 'var(--text-muted)' }}>
          slug: {pkg.slug} · {files.length} 个文件
        </span>
      </div>

      {files.length === 0 ? (
        <EmptyState text="该技能包暂无文件。" />
      ) : (
        <div className="flex gap-3">
          {/* 文件树 */}
          <div
            className="max-h-96 w-56 shrink-0 overflow-y-auto rounded-lg border text-xs"
            style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)' }}
          >
            {files.map((f) => (
              <button
                key={f.id}
                type="button"
                className="block w-full truncate px-3 py-1.5 text-left transition"
                style={{
                  backgroundColor:
                    f.relPath === selectedRelPath ? 'var(--accent-light)' : 'transparent',
                  color:
                    f.relPath === selectedRelPath ? 'var(--accent-color)' : 'var(--text-primary)',
                }}
                title={`${f.relPath}（${formatCharCount(f.sizeChars)} 字符）`}
                onClick={() => setSelectedRelPath(f.relPath)}
              >
                <span className="font-mono">{f.relPath}</span>
              </button>
            ))}
          </div>

          {/* 文件预览 */}
          <div
            className="min-w-0 flex-1 overflow-auto rounded-lg border p-3"
            style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)' }}
          >
            {selected ? (
              <FilePreview file={selected} />
            ) : (
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                点选左侧文件查看内容
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/** 单文件预览：.md 走 Markdown 渲染（frontmatter 解析复用 skillDoc），其余展示原文 */
function FilePreview({ file }: { file: SkillPackageFile }) {
  const isMarkdown = /\.md|\.markdown$/i.test(file.relPath);
  const parsed = useMemo(
    () => (isMarkdown ? parseFrontmatter(file.content) : null),
    [isMarkdown, file.content],
  );

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <span className="truncate font-mono text-xs" style={{ color: 'var(--text-secondary)' }}>
          {file.relPath}
        </span>
        <span className="ml-3 shrink-0 text-xs" style={{ color: 'var(--text-muted)' }}>
          {formatCharCount(file.sizeChars)} 字符
        </span>
      </div>
      {isMarkdown && parsed && Object.keys(parsed.meta).length > 0 && (
        <div className="mb-2 flex flex-wrap gap-1.5">
          {Object.entries(parsed.meta).map(([k, v]) => (
            <span
              key={k}
              className="rounded px-1.5 py-0.5 font-mono text-xs"
              style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }}
            >
              {k}: {v}
            </span>
          ))}
        </div>
      )}
      {isMarkdown && parsed ? (
        parsed.body.trim() ? (
          <MarkdownContent content={parsed.body} />
        ) : (
          <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
            暂无正文内容
          </p>
        )
      ) : (
        <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-relaxed" style={{ color: 'var(--text-primary)' }}>
          {file.content}
        </pre>
      )}
    </div>
  );
}
