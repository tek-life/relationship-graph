// 模型配置（P1-7）：按场景（local/chat/chat_search/extract/summarize）配置模型，
// 并展示最近 LLM 调用用量元数据（token 数/耗时，不含任何对话内容）。
// 模型解析优先级：env RG_*_MODEL > 数据库配置 > 硬编码默认；env 覆盖时页面如实提示。

import { useCallback, useEffect, useState } from 'react';
import { RefreshCw, Save, Trash2 } from 'lucide-react';
import { apiDelete, apiGet, apiPut } from '../../services/api';
import { Badge, Card, ConfirmDialog, EmptyState, ToastProvider, useToast } from '../ui';
import { AdminPageHeader, ErrorBanner, LoadingSpinner } from './shared';
import type { LlmUsageRow, ModelConfigItem, ModelConfigListResponse } from './types';

/** 场景键的中文展示名 */
const SCENARIO_LABELS: Record<string, string> = {
  local: '本地通道',
  chat: '云端聊天',
  chat_search: '联网搜索',
  extract: '结构化抽取',
  summarize: '压缩摘要',
};

function scenarioLabel(scenario: string): string {
  return SCENARIO_LABELS[scenario] ?? scenario;
}

/** 单场景配置行：展示生效/默认模型，支持修改与清除 */
function ModelConfigRow({
  item,
  onChanged,
}: {
  item: ModelConfigItem;
  onChanged: () => void;
}) {
  const toast = useToast();
  const [draft, setDraft] = useState(item.model ?? '');
  const [saving, setSaving] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);

  // 列表刷新后同步草稿（仅在非编辑中）
  useEffect(() => {
    setDraft(item.model ?? '');
  }, [item.scenario, item.model]);

  const dirty = draft.trim() !== (item.model ?? '');

  const handleSave = async () => {
    const model = draft.trim();
    if (!model) {
      toast.error('模型名不能为空');
      return;
    }
    setSaving(true);
    try {
      await apiPut(`/api/admin/model-configs/${item.scenario}`, { model });
      if (item.envOverride) {
        toast.info('已保存，但当前被环境变量覆盖，暂不会生效', { duration: 6000 });
      } else {
        toast.success(`「${scenarioLabel(item.scenario)}」模型已更新`);
      }
      onChanged();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleClear = async () => {
    try {
      await apiDelete(`/api/admin/model-configs/${item.scenario}`);
      setConfirmClear(false);
      toast.success('已清除数据库配置，回退到环境变量 / 默认值');
      onChanged();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <tr style={{ borderColor: 'var(--border-color)' }} className="border-t">
      <td className="px-3 py-3 align-top">
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
            {scenarioLabel(item.scenario)}
          </span>
          <span className="font-mono text-xs" style={{ color: 'var(--text-muted)' }}>
            {item.scenario}
          </span>
        </div>
      </td>
      <td className="px-3 py-3 align-top">
        <p className="text-xs leading-relaxed" style={{ color: 'var(--text-secondary)' }}>
          {item.description}
        </p>
        <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
          <Badge variant="default">默认 {item.defaultModel}</Badge>
          {item.envOverride && (
            <Badge variant="warning">env 覆盖：{item.envOverride}</Badge>
          )}
        </div>
      </td>
      <td className="px-3 py-3 align-top">
        <span className="font-mono text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          {item.effectiveModel ?? '—'}
        </span>
      </td>
      <td className="px-3 py-3 align-top">
        <div className="flex items-center gap-1.5">
          <input
            className="input w-40 font-mono text-xs"
            placeholder={item.defaultModel}
            spellCheck={false}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && dirty && !saving) handleSave();
            }}
            aria-label={`${scenarioLabel(item.scenario)}场景模型名`}
          />
          <button
            type="button"
            className="btn-secondary shrink-0 !px-2"
            onClick={handleSave}
            disabled={!dirty || saving}
            aria-label={`保存${scenarioLabel(item.scenario)}模型`}
          >
            <Save size={14} aria-hidden="true" />
          </button>
          {item.model && (
            <button
              type="button"
              className="btn-secondary shrink-0 !px-2"
              style={{ color: 'var(--danger-color)' }}
              onClick={() => setConfirmClear(true)}
              aria-label={`清除${scenarioLabel(item.scenario)}配置`}
            >
              <Trash2 size={14} aria-hidden="true" />
            </button>
          )}
        </div>
      </td>
      {confirmClear && (
        <ConfirmDialog
          title="清除数据库中的模型配置"
          message={`清除「${scenarioLabel(item.scenario)}」的配置后，将回退到环境变量或硬编码默认值（${item.defaultModel}）。确认清除吗？`}
          confirmLabel="清除"
          danger
          onConfirm={handleClear}
          onCancel={() => setConfirmClear(false)}
        />
      )}
    </tr>
  );
}

/** 最近 LLM 调用用量（元数据表：无对话内容） */
function UsageSection() {
  const toast = useToast();
  const [usages, setUsages] = useState<LlmUsageRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const fetchUsages = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const res = await apiGet<{ usages: LlmUsageRow[] }>('/api/admin/llm-usages?limit=50');
      setUsages(res.usages);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchUsages();
  }, [fetchUsages]);

  return (
    <Card className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div>
          <h3 className="text-base font-semibold" style={{ color: 'var(--text-primary)' }}>
            最近调用用量
          </h3>
          <p className="mt-0.5 text-xs" style={{ color: 'var(--text-muted)' }}>
            仅记录 token 数 / 耗时等元数据，不记录任何对话内容
          </p>
        </div>
        <button
          type="button"
          className="btn-secondary shrink-0"
          onClick={() => {
            fetchUsages();
            toast.info('已刷新用量列表');
          }}
          disabled={loading}
        >
          <RefreshCw size={14} aria-hidden="true" />
          刷新
        </button>
      </div>

      {error && <ErrorBanner message={error} />}

      {loading ? (
        <LoadingSpinner text="正在加载用量记录…" />
      ) : usages.length === 0 ? (
        <EmptyState text="暂无用量记录；发起聊天或抽取调用后，这里会展示调用元数据" />
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-left text-xs">
            <thead>
              <tr style={{ color: 'var(--text-muted)' }}>
                <th className="px-3 py-2 font-medium">时间</th>
                <th className="px-3 py-2 font-medium">场景</th>
                <th className="px-3 py-2 font-medium">通道</th>
                <th className="px-3 py-2 font-medium">模型</th>
                <th className="px-3 py-2 font-medium">函数</th>
                <th className="px-3 py-2 text-right font-medium">输入 tokens</th>
                <th className="px-3 py-2 text-right font-medium">输出 tokens</th>
                <th className="px-3 py-2 text-right font-medium">耗时</th>
              </tr>
            </thead>
            <tbody>
              {usages.map((row) => (
                <tr key={row.id} className="border-t" style={{ borderColor: 'var(--border-color)' }}>
                  <td className="whitespace-nowrap px-3 py-2 font-mono" style={{ color: 'var(--text-secondary)' }}>
                    {new Date(row.createdAt).toLocaleString('zh-CN', { hour12: false })}
                  </td>
                  <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
                    {scenarioLabel(row.scenario)}
                  </td>
                  <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
                    {row.channel}
                  </td>
                  <td className="px-3 py-2 font-mono" style={{ color: 'var(--text-primary)' }}>
                    {row.model}
                  </td>
                  <td className="px-3 py-2 font-mono" style={{ color: 'var(--text-muted)' }}>
                    {row.fnName || '—'}
                  </td>
                  <td className="px-3 py-2 text-right font-mono" style={{ color: 'var(--text-secondary)' }}>
                    {row.promptTokens ?? '—'}
                  </td>
                  <td className="px-3 py-2 text-right font-mono" style={{ color: 'var(--text-secondary)' }}>
                    {row.completionTokens ?? '—'}
                  </td>
                  <td className="whitespace-nowrap px-3 py-2 text-right font-mono" style={{ color: 'var(--text-secondary)' }}>
                    {(row.elapsedMs / 1000).toFixed(1)}s
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  );
}

function ModelConfigInner() {
  const [configs, setConfigs] = useState<ModelConfigItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const fetchConfigs = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const res = await apiGet<ModelConfigListResponse>('/api/admin/model-configs');
      setConfigs(res.configs);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfigs();
  }, [fetchConfigs]);

  return (
    <div className="space-y-4">
      <AdminPageHeader
        title="模型配置"
        description="按场景配置各调用点使用的模型；优先级：环境变量 RG_*_MODEL ＞ 本页配置 ＞ 默认值"
      />

      {error && <ErrorBanner message={error} />}

      {loading ? (
        <LoadingSpinner text="正在加载模型配置…" />
      ) : (
        <Card className="overflow-hidden p-0">
          <div className="overflow-x-auto">
            <table className="w-full border-collapse text-left">
              <thead>
                <tr style={{ color: 'var(--text-muted)' }} className="text-xs">
                  <th className="px-3 py-2.5 font-medium">场景</th>
                  <th className="px-3 py-2.5 font-medium">说明</th>
                  <th className="px-3 py-2.5 font-medium">生效模型</th>
                  <th className="px-3 py-2.5 font-medium">配置</th>
                </tr>
              </thead>
              <tbody>
                {configs.map((item) => (
                  <ModelConfigRow key={item.scenario} item={item} onChanged={fetchConfigs} />
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}

      <UsageSection />
    </div>
  );
}

export default function ModelConfigManager() {
  return (
    <ToastProvider>
      <ModelConfigInner />
    </ToastProvider>
  );
}
