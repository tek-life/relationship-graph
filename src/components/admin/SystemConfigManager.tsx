// 系统设置（P0-2）：云端 LLM API Key 管理。
// 服务端只回掩码（sk-…后 4 位）+ 是否已设置 + 生效来源，绝不回传明文；
// Key 优先级：env RG_CLOUD_API_KEY > ~/.config/rg-cloud-api-key > 数据库 settings。

import { useCallback, useEffect, useState } from 'react';
import { apiDelete, apiGet, apiPut } from '../../services/api';
import { Badge, Card, ConfirmDialog, ToastProvider, useToast } from '../ui';
import type { BadgeVariant } from '../ui';
import { ErrorBanner, LoadingSpinner } from './shared';
import type { CloudApiKeySource, CloudApiKeyStatus, SystemConfig } from './types';

/** 生效来源的展示文案与徽标语义 */
const SOURCE_META: Record<CloudApiKeySource, { label: string; variant: BadgeVariant }> = {
  env: { label: '环境变量', variant: 'info' },
  file: { label: '配置文件', variant: 'default' },
  db: { label: '数据库', variant: 'success' },
};

/** Key 输入/保存/清除 表单区 */
function CloudApiKeySection({
  status,
  onSaved,
}: {
  status: CloudApiKeyStatus;
  onSaved: (next: CloudApiKeyStatus) => void;
}) {
  const toast = useToast();
  const [draft, setDraft] = useState('');
  const [revealed, setRevealed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [clearing, setClearing] = useState(false);

  const handleSave = async () => {
    const apiKey = draft.trim();
    if (!apiKey) {
      toast.error('请先输入 API Key');
      return;
    }
    setSaving(true);
    try {
      const res = await apiPut<{ updated: boolean; cloudApiKey: CloudApiKeyStatus }>(
        '/api/admin/config/cloud-api-key',
        { apiKey }
      );
      onSaved(res.cloudApiKey);
      setDraft('');
      setRevealed(false);
      if (res.cloudApiKey.source === 'db') {
        toast.success('API Key 已保存并生效');
      } else {
        // env / 文件优先级更高：保存成功但当前不生效，需要明确告知
        toast.info(
          `已保存到数据库，但当前生效来源是「${
            res.cloudApiKey.source ? SOURCE_META[res.cloudApiKey.source].label : '未知'
          }」，数据库中的 Key 暂不会生效`,
          { duration: 8000 }
        );
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleClear = async () => {
    setClearing(true);
    try {
      const res = await apiDelete<{ deleted: boolean; cloudApiKey: CloudApiKeyStatus }>(
        '/api/admin/config/cloud-api-key'
      );
      onSaved(res.cloudApiKey);
      setConfirmClear(false);
      if (res.cloudApiKey.source) {
        toast.info(
          `数据库中的 Key 已清除；当前仍有来自「${SOURCE_META[res.cloudApiKey.source].label}」的生效 Key`,
          { duration: 6000 }
        );
      } else {
        toast.success('数据库中的 Key 已清除');
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setClearing(false);
    }
  };

  return (
    <Card className="space-y-4">
      {/* 标题与当前状态 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-base font-semibold" style={{ color: 'var(--text-primary)' }}>
            云端大模型 API Key
          </h3>
          <p className="mt-0.5 text-xs" style={{ color: 'var(--text-muted)' }}>
            用于 cloud 通道（阿里云百炼兼容端点）；保存后仅展示掩码，不可再读取明文
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant={status.configured ? 'success' : 'danger'}>
            {status.configured ? '已配置' : '未配置'}
          </Badge>
          {status.source && (
            <Badge variant={SOURCE_META[status.source].variant}>
              生效来源：{SOURCE_META[status.source].label}
            </Badge>
          )}
        </div>
      </div>

      {/* 掩码展示 */}
      <div
        className="flex items-center justify-between rounded-lg border px-3 py-2"
        style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)' }}
      >
        <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>
          当前生效 Key
        </span>
        <span className="font-mono text-sm" style={{ color: 'var(--text-primary)' }}>
          {status.mask ?? '— 未配置 —'}
        </span>
      </div>

      {/* 输入与操作 */}
      <div className="space-y-2">
        <label className="block text-xs font-medium" style={{ color: 'var(--text-secondary)' }} htmlFor="cloud-api-key-input">
          {status.dbConfigured ? '更换数据库中的 Key' : '输入 API Key（保存到数据库）'}
        </label>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <input
              id="cloud-api-key-input"
              type={revealed ? 'text' : 'password'}
              className="input w-full pr-10 font-mono text-sm"
              placeholder="sk-sp-…"
              autoComplete="off"
              spellCheck={false}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !saving) handleSave();
              }}
            />
            {/* 明文可见性切换 */}
            <button
              type="button"
              aria-label={revealed ? '隐藏输入内容' : '显示输入内容'}
              className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 transition hover:bg-black/5"
              style={{ color: 'var(--text-muted)' }}
              onClick={() => setRevealed((v) => !v)}
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                {revealed ? (
                  <>
                    <path d="M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4z" stroke="currentColor" strokeWidth="1.3" />
                    <circle cx="8" cy="8" r="1.8" stroke="currentColor" strokeWidth="1.3" />
                  </>
                ) : (
                  <>
                    <path d="M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4z" stroke="currentColor" strokeWidth="1.3" />
                    <path d="M3 13 13 3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
                  </>
                )}
              </svg>
            </button>
          </div>
          <button type="button" className="btn-primary shrink-0" onClick={handleSave} disabled={saving || !draft.trim()}>
            {saving ? '保存中…' : '保存'}
          </button>
          {status.dbConfigured && (
            <button
              type="button"
              className="btn-secondary shrink-0"
              style={{ color: 'var(--danger-color)' }}
              onClick={() => setConfirmClear(true)}
              disabled={clearing}
            >
              清除
            </button>
          )}
        </div>
      </div>

      {/* 优先级说明 */}
      <div
        className="rounded-lg border border-dashed px-3 py-2 text-xs leading-relaxed"
        style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}
      >
        读取优先级：<span className="font-mono">RG_CLOUD_API_KEY</span>（环境变量）＞
        <span className="font-mono"> ~/.config/rg-cloud-api-key</span>（文件）＞ 数据库（本页保存）。
        更高优先级来源存在时，数据库中的 Key 不会生效。
        {status.dbConfigured && status.source && status.source !== 'db' && (
          <span style={{ color: 'var(--warning-text, #d97706)' }}>
            （当前即被「{SOURCE_META[status.source].label}」覆盖）
          </span>
        )}
      </div>

      {confirmClear && (
        <ConfirmDialog
          title="清除数据库中的 API Key"
          message="清除后，cloud 通道将回退到环境变量 / 配置文件来源；若两者都没有，cloud 通道调用会失败。确认清除吗？"
          confirmLabel="清除"
          danger
          onConfirm={handleClear}
          onCancel={() => setConfirmClear(false)}
        />
      )}
    </Card>
  );
}

function SystemConfigInner() {
  const [config, setConfig] = useState<SystemConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const fetchConfig = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      setConfig(await apiGet<SystemConfig>('/api/admin/config'));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  if (loading) return <LoadingSpinner text="正在加载系统设置…" />;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold" style={{ color: 'var(--text-primary)' }}>
          系统设置
        </h3>
      </div>

      {error && <ErrorBanner message={error} />}

      {config && (
        <CloudApiKeySection
          status={config.cloudApiKey}
          onSaved={(next) => setConfig({ ...config, cloudApiKey: next })}
        />
      )}
    </div>
  );
}

export default function SystemConfigManager() {
  return (
    <ToastProvider>
      <SystemConfigInner />
    </ToastProvider>
  );
}
