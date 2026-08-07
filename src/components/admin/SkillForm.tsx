// 技能编辑共享表单：统一 AgentManager SkillPanel 与 AgentDetailPage 技能区的编辑逻辑。
// 形态探测：已有技能且 skillMarkdown 为空 → 旧 JSON 编辑路径（行为不变）；
// 新建或 skillMarkdown 非空 → Markdown 形态（frontmatter + 完整 SKILL 文档）。

import { useMemo, useState } from 'react';
import { apiPost, apiPut } from '../../services/api';
import { clearDigitalAgentsCache } from '../../services/digitalAgents';
import {
  parseFrontmatter,
  serializeFrontmatter,
  validateFrontmatter,
  SKILL_TEMPLATE,
} from '../../services/skillDoc';
import MarkdownContent from '../MarkdownContent';
import { ErrorBanner } from './shared';
import type { AgentSkill, CreateAgentSkillRequest } from './types';
import { TRIGGER_SCENARIOS } from './types';

interface SkillFormProps {
  agentId: string;
  /** null = 新建技能 */
  skill: AgentSkill | null;
  /** 保存成功后回调（父组件刷新列表并关闭表单） */
  onSaved: () => void | Promise<void>;
  onCancel: () => void;
}

/** 触发场景中文标签 */
function triggerLabel(value: string): string {
  return value === 'always' ? '始终' : value === 'on_mention' ? '@提及时' : value === 'manual' ? '手动' : value;
}

export default function SkillForm({ agentId, skill, onSaved, onCancel }: SkillFormProps) {
  // 形态探测：已有数据且 skillMarkdown 为空 → 旧 JSON 编辑路径
  const mode: 'json' | 'markdown' = skill && !skill.skillMarkdown ? 'json' : 'markdown';

  const [skillName, setSkillName] = useState(skill?.skillName ?? '');
  const [skillConfigJson, setSkillConfigJson] = useState(skill?.skillConfigJson ?? '{}');
  const [skillMarkdown, setSkillMarkdown] = useState(skill?.skillMarkdown ?? '');
  const [triggerScenario, setTriggerScenario] = useState(skill?.triggerScenario ?? 'always');
  const [isActive, setIsActive] = useState(skill?.isActive ?? true);
  const [tab, setTab] = useState<'edit' | 'preview'>('edit');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  // 当前 frontmatter 实时解析（description 徽标 / 预览用）
  const parsed = useMemo(() => parseFrontmatter(skillMarkdown), [skillMarkdown]);

  // 旧数据中的非标准触发场景仍保留在下拉中，避免保存时被静默改写
  const triggerOptions = useMemo(() => {
    const list: string[] = [...TRIGGER_SCENARIOS];
    if (triggerScenario && !list.includes(triggerScenario)) list.push(triggerScenario);
    return list;
  }, [triggerScenario]);

  /** 插入模板（仅在文档为空时可用，避免覆盖已有内容） */
  const handleInsertTemplate = () => {
    setSkillMarkdown(SKILL_TEMPLATE);
    setTab('edit');
    setError('');
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!skillName.trim()) {
      setError('技能名称不能为空');
      return;
    }

    const req: CreateAgentSkillRequest = {
      agentId,
      skillName: skillName.trim(),
      triggerScenario,
      isActive,
    };

    if (mode === 'json') {
      // 旧 JSON 编辑路径：行为逐行不变
      try {
        JSON.parse(skillConfigJson);
      } catch {
        setError('技能配置 JSON 格式不正确');
        return;
      }
      req.skillConfigJson = skillConfigJson;
    } else {
      // Markdown 形态：本地校验 frontmatter 必填字段
      const fmError = validateFrontmatter(skillMarkdown);
      if (fmError) {
        setError(fmError);
        return;
      }
      // frontmatter name 自动同步为技能名称
      const { meta, body } = parseFrontmatter(skillMarkdown);
      const finalMd = serializeFrontmatter({ ...meta, name: skillName.trim() }, body);
      req.skillConfigJson = '{}';
      req.skillMarkdown = finalMd;
    }

    setError('');
    setSubmitting(true);
    try {
      if (!skill) {
        await apiPost<AgentSkill>(`/api/admin/digital-agents/${agentId}/skills`, req);
      } else {
        await apiPut(`/api/admin/agent-skills/${skill.id}`, req);
      }
      clearDigitalAgentsCache();
      await onSaved();
    } catch (err) {
      // 后端 400 校验失败时携带中文原因，直接展示
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="space-y-3 rounded-lg border p-3"
      style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
    >
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          {skill ? '编辑技能' : '新建技能'}
        </h4>
        <span
          className="badge"
          style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-muted)' }}
        >
          {mode === 'markdown' ? 'Markdown 形态' : '旧 JSON 形态'}
        </span>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
            技能名称 *
          </label>
          <input
            className="input"
            value={skillName}
            onChange={(e) => setSkillName(e.target.value)}
            placeholder="如：联系人搜索"
            required
          />
        </div>
        <div>
          <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
            触发场景
          </label>
          <select
            className="input"
            value={triggerScenario}
            onChange={(e) => setTriggerScenario(e.target.value)}
          >
            {triggerOptions.map((s) => (
              <option key={s} value={s}>
                {triggerLabel(s)}
              </option>
            ))}
          </select>
        </div>
      </div>

      {mode === 'json' ? (
        <div>
          <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
            技能配置 JSON
          </label>
          <textarea
            className="input min-h-20 font-mono text-xs"
            value={skillConfigJson}
            onChange={(e) => setSkillConfigJson(e.target.value)}
            placeholder='{"key": "value"}'
          />
        </div>
      ) : (
        <div className="space-y-2">
          {/* frontmatter 回显：name 自动同步技能名称（只读），description 徽标 */}
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <span style={{ color: 'var(--text-muted)' }}>frontmatter name（自动同步）：</span>
            <span
              className="rounded px-2 py-0.5 font-mono"
              style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-primary)' }}
            >
              {skillName.trim() || '—'}
            </span>
            {parsed.meta.description ? (
              <span
                className="badge max-w-xs truncate"
                style={{ backgroundColor: 'var(--accent-light)', color: 'var(--accent-color)' }}
                title={parsed.meta.description}
              >
                {parsed.meta.description}
              </span>
            ) : (
              <span style={{ color: 'var(--text-muted)' }}>description 待填写</span>
            )}
          </div>

          <div className="flex items-center justify-between">
            <div className="flex gap-1">
              <button
                type="button"
                className="rounded px-2 py-1 text-xs transition"
                style={
                  tab === 'edit'
                    ? { backgroundColor: 'var(--accent-color)', color: '#fff' }
                    : { backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }
                }
                onClick={() => setTab('edit')}
              >
                编辑
              </button>
              <button
                type="button"
                className="rounded px-2 py-1 text-xs transition"
                style={
                  tab === 'preview'
                    ? { backgroundColor: 'var(--accent-color)', color: '#fff' }
                    : { backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }
                }
                onClick={() => setTab('preview')}
              >
                预览
              </button>
            </div>
            <button
              type="button"
              className="text-xs transition hover:underline disabled:cursor-not-allowed disabled:opacity-40 disabled:no-underline"
              style={{ color: 'var(--accent-color)' }}
              disabled={skillMarkdown.trim() !== ''}
              title={skillMarkdown.trim() ? '文档非空时不可覆盖' : '填入 Claude 风格 SKILL 模板'}
              onClick={handleInsertTemplate}
            >
              + 插入模板
            </button>
          </div>

          {tab === 'edit' ? (
            <textarea
              className="input min-h-64 font-mono text-xs leading-relaxed"
              value={skillMarkdown}
              onChange={(e) => setSkillMarkdown(e.target.value)}
              placeholder={'---\nname: 技能名称\ndescription: 一句话说明用途\n---\n\n# 技能正文…'}
            />
          ) : (
            <div
              className="input min-h-64 overflow-y-auto text-sm"
              style={{ backgroundColor: 'var(--bg-secondary)' }}
            >
              {Object.keys(parsed.meta).length > 0 && (
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
              {parsed.body.trim() ? (
                <MarkdownContent content={parsed.body} />
              ) : (
                <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                  暂无正文内容
                </p>
              )}
            </div>
          )}
        </div>
      )}

      <label className="flex items-center gap-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
        <input
          type="checkbox"
          checked={isActive}
          onChange={(e) => setIsActive(e.target.checked)}
          className="h-4 w-4"
        />
        启用
      </label>

      <div className="flex gap-2">
        <button type="submit" className="btn-primary text-xs" disabled={submitting}>
          {submitting ? '保存中…' : '保存'}
        </button>
        <button type="button" className="btn-secondary text-xs" onClick={onCancel}>
          取消
        </button>
      </div>
    </form>
  );
}
