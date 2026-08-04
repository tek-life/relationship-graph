import { useMemo, useState } from 'react';
import DraftConfirmation from './DraftConfirmation';
import PathResultDisplay from './PathResultDisplay';
import NlqResultCard from './NlqResultCard';
import { confirmAgentDraft, submitAgentQuery } from '../services/agent';
import { getCouncilMembers } from '../services/council';
import { getDefaultProfile } from '../services/profile';
import { queryLocalBridge, type LocalBridgeQueryResult } from '../services/localBridge';
import type { AgentChatMessage } from '../types';

interface AgentWorkspaceProps {
  onPersonClick?: (personId: string) => void;
}

const WELCOME_MESSAGE: AgentChatMessage = {
  id: 'welcome',
  role: 'assistant',
  content: '我可以帮你查询联系人、生成草稿、规划路径，并把结果放进右侧面板里。当前流程已接入 LangGraph 编排与 agent-chat-ui 风格的消息/工件面板。',
  artifact: {
    id: 'welcome-artifact',
    kind: 'context',
    title: '当前工作区',
    summary: '这里是第一版 agent 工作台：消息流负责交互，右侧面板展示搜索结果、草稿确认和路径建议。',
    context: 'profile:default; permission:read-only; target:relationship-graph',
  },
  status: 'success',
};

export default function AgentWorkspace({ onPersonClick }: AgentWorkspaceProps) {
  const [messages, setMessages] = useState<AgentChatMessage[]>([WELCOME_MESSAGE]);
  const [draft, setDraft] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [selectedArtifactId, setSelectedArtifactId] = useState<string | null>(WELCOME_MESSAGE.artifact?.id ?? null);
  const [localBridgeResults, setLocalBridgeResults] = useState<LocalBridgeQueryResult[]>([]);
  const [selectedCouncilId, setSelectedCouncilId] = useState<string | null>(null);
  const profile = getDefaultProfile();
  const councilMembers = getCouncilMembers();

  const selectedArtifact = useMemo(() => {
    const selected = messages.find((message) => message.artifact?.id === selectedArtifactId);
    return selected?.artifact ?? null;
  }, [messages, selectedArtifactId]);

  const submit = async (event?: React.FormEvent) => {
    event?.preventDefault();
    const query = draft.trim();
    if (!query || loading) return;

    const userMessage: AgentChatMessage = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: query,
      status: 'idle',
    };
    setMessages((prev) => [...prev, userMessage]);
    setDraft('');
    setLoading(true);
    setError('');

    try {
      const assistantMessage = await submitAgentQuery(query, { profile, councilMemberId: selectedCouncilId });
      setMessages((prev) => [...prev, assistantMessage]);
      setSelectedArtifactId(assistantMessage.artifact?.id ?? null);
    } catch (err) {
      const fallback: AgentChatMessage = {
        id: `assistant-error-${Date.now()}`,
        role: 'assistant',
        content: `抱歉，处理失败：${String(err)}`,
        artifact: {
          id: `error-${Date.now()}`,
          kind: 'context',
          title: '错误信息',
          summary: '本次请求没有产生可展示结果。',
          context: 'error',
        },
        status: 'error',
      };
      setMessages((prev) => [...prev, fallback]);
      setSelectedArtifactId(fallback.artifact?.id ?? null);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleConfirm = async (intentType: string, data: Record<string, unknown>) => {
    try {
      await confirmAgentDraft(intentType, data);
      const followUpMessage: AgentChatMessage = {
        id: `assistant-confirm-${Date.now()}`,
        role: 'assistant',
        content: '草稿已进入确认链路，后续可继续补充字段或取消。',
        artifact: {
          id: `confirm-${Date.now()}`,
          kind: 'context',
          title: '确认状态',
          summary: '确认动作已提交，下一步将由写入层继续处理。',
          context: intentType,
        },
        status: 'success',
      };
      setMessages((prev) => [...prev, followUpMessage]);
      setSelectedArtifactId(followUpMessage.artifact?.id ?? null);
    } catch (err) {
      setError(String(err));
    }
  };

  const runLocalBridgeProbe = async () => {
    try {
      const results = await queryLocalBridge({ query: '高敏感本地路径预览', mode: 'readonly' });
      setLocalBridgeResults(results);
      const bridgeMessage: AgentChatMessage = {
        id: `assistant-bridge-${Date.now()}`,
        role: 'assistant',
        content: '我已调用浏览器侧本地桥接原型，当前只返回高敏感上下文的脱敏摘要。',
        artifact: {
          id: `bridge-${Date.now()}`,
          kind: 'context',
          title: '本地桥接结果',
          summary: '这是一个受控的浏览器/PWA 本地桥接占位，后续会接入真实高敏感本地查询。',
          context: 'local-bridge-prototype',
        },
        status: 'success',
      };
      setMessages((prev) => [...prev, bridgeMessage]);
      setSelectedArtifactId(bridgeMessage.artifact?.id ?? null);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="grid grid-cols-1 gap-6 xl:grid-cols-[1.25fr_0.75fr]">
      <section className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h2 className="text-xl font-semibold">Agent 工作台</h2>
            <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>基于 LangGraph 的编排层 + 可见工作流节点 + artifact 面板的浏览器/PWA 交互原型。</p>
          </div>
          <span className="rounded-full px-3 py-1 text-xs" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }}>
            Phase 1 / 5
          </span>
        </div>

        <div className="space-y-3">
          <div className="rounded-xl border p-3" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-color)' }}>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>上下文</span>
            </div>
            <p className="text-sm" style={{ color: 'var(--text-primary)' }}>
              画像：{profile.persona}
            </p>
            <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
              价值观：{profile.values.join(' / ')}
            </p>
            <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
              智囊团：{councilMembers.map((member) => `${member.name}(${member.skill})`).join('；')}
            </p>
            <p className="mt-2 text-xs" style={{ color: 'var(--accent-color)' }}>
              当前技能：{selectedCouncilId ? councilMembers.find((member) => member.id === selectedCouncilId)?.name : '通用助手（未选择智囊团成员）'}
            </p>
          </div>

          {messages.map((message) => (
            <div key={message.id} className={`rounded-xl border p-3 ${message.role === 'user' ? 'ml-8' : 'mr-8'}`} style={{ backgroundColor: message.role === 'user' ? 'var(--bg-secondary)' : 'var(--bg-primary)', borderColor: 'var(--border-color)' }}>
              <div className="mb-2 flex items-center justify-between">
                <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{message.role === 'user' ? '你' : '助手'}</span>
                {message.artifact && (
                  <button type="button" className="text-xs hover:underline" style={{ color: 'var(--accent-color)' }} onClick={() => setSelectedArtifactId(message.artifact?.id ?? null)}>
                    查看结果
                  </button>
                )}
              </div>
              <p className="text-sm" style={{ color: 'var(--text-primary)' }}>{message.content}</p>
              {message.role === 'assistant' && message.workflowTrace && (
                <div className="mt-3 rounded-lg border p-2 text-xs" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)' }}>
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium" style={{ color: 'var(--text-primary)' }}>
                      {message.workflowTrace.mode === 'local-bridge' ? '本地桥接路线' : '关系域路线'}
                    </span>
                    <span style={{ color: 'var(--text-secondary)' }}>{message.workflowTrace.policy}</span>
                  </div>
                  <ul className="mt-2 space-y-1" style={{ color: 'var(--text-secondary)' }}>
                    {message.workflowTrace.steps.map((step) => (
                      <li key={step}>• {step}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ))}
        </div>

        <form className="mt-4 space-y-3" onSubmit={submit}>
          <textarea
            rows={3}
            className="w-full rounded-xl border px-3 py-2 text-sm outline-none"
            style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
            placeholder="例如：谁在上海做地产，和我关系比较近？"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            disabled={loading}
          />
          {error && <p className="rounded bg-red-50 p-2 text-sm text-red-700">{error}</p>}
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="text-xs" style={{ color: 'var(--text-secondary)' }}>当前版本先接入现有 NLQ / 草稿确认链路，后续会继续补充 profile、council、local companion。</p>
            <div className="flex gap-2">
              <button type="button" className="rounded-full border px-4 py-2 text-sm" style={{ borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }} onClick={runLocalBridgeProbe}>
                测试本地桥接
              </button>
              <button type="submit" className="btn-primary rounded-full px-4 py-2 text-sm" disabled={loading || !draft.trim()}>
                {loading ? '处理中…' : '发送'}
              </button>
            </div>
          </div>
        </form>
      </section>

      <aside className="space-y-4">
        <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <h3 className="font-semibold">右侧面板</h3>
          <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>这里展示当前消息对应的 artifact：检索结果、草稿确认、路径建议或上下文摘要；同时会把画像上下文和智囊团入口显示出来。</p>
        </div>

        <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <h3 className="font-semibold">画像与智囊团</h3>
          <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>当前使用静态默认画像与虚拟成员占位，后续会替换为真实服务。点击成员即可将其 skill 接入下一次请求。</p>
          <ul className="mt-3 space-y-2 text-sm">
            <li>
              <button
                type="button"
                className="w-full rounded border px-2 py-2 text-left"
                style={{
                  borderColor: selectedCouncilId === null ? 'var(--accent-color)' : 'var(--border-color)',
                  color: 'var(--text-secondary)',
                  backgroundColor: selectedCouncilId === null ? 'var(--surface-hover)' : 'transparent',
                }}
                onClick={() => setSelectedCouncilId(null)}
              >
                <span className="font-medium" style={{ color: 'var(--text-primary)' }}>通用助手</span>：不指定 skill，按默认编排回复
              </button>
            </li>
            {councilMembers.map((member) => (
              <li key={member.id}>
                <button
                  type="button"
                  className="w-full rounded border px-2 py-2 text-left"
                  style={{
                    borderColor: selectedCouncilId === member.id ? 'var(--accent-color)' : 'var(--border-color)',
                    color: 'var(--text-secondary)',
                    backgroundColor: selectedCouncilId === member.id ? 'var(--surface-hover)' : 'transparent',
                  }}
                  onClick={() => setSelectedCouncilId(member.id)}
                  title={member.description}
                >
                  <span className="font-medium" style={{ color: 'var(--text-primary)' }}>{member.name}</span>：{member.skill}
                </button>
              </li>
            ))}
          </ul>
        </div>

        <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <h3 className="font-semibold">本地桥接（浏览器/PWA）</h3>
          <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>这一层用于演示“浏览器端调用本地受控桥接”的方向，默认只返回脱敏摘要和待确认草稿。</p>
          {localBridgeResults.length > 0 ? (
            <ul className="mt-3 space-y-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
              {localBridgeResults.map((result, index) => (
                <li key={`${result.title}-${index}`} className="rounded border px-2 py-2" style={{ borderColor: 'var(--border-color)' }}>
                  <div className="font-medium" style={{ color: 'var(--text-primary)' }}>{result.title}</div>
                  <div className="mt-1">{result.summary}</div>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-3 text-sm" style={{ color: 'var(--text-secondary)' }}>点击“测试本地桥接”后，这里会显示受控结果。</p>
          )}
        </div>

        {selectedArtifact ? (
          <div className="space-y-3">
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <div className="mb-3 flex items-center justify-between">
                <h3 className="font-semibold">{selectedArtifact.title}</h3>
                <span className="rounded-full px-2 py-0.5 text-xs" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }}>{selectedArtifact.kind}</span>
              </div>
              <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>{selectedArtifact.summary}</p>
            </div>

            {selectedArtifact.kind === 'search' && selectedArtifact.results.length > 0 && (
              <div className="space-y-2">
                {selectedArtifact.results.map((result) => (
                  <NlqResultCard key={result.personId} result={result} onPersonClick={onPersonClick} />
                ))}
              </div>
            )}

            {selectedArtifact.kind === 'draft' && selectedArtifact.response && (
              <DraftConfirmation response={selectedArtifact.response} onConfirm={handleConfirm} onCancel={() => setSelectedArtifactId(null)} />
            )}

            {selectedArtifact.kind === 'path' && selectedArtifact.path && (
              <PathResultDisplay path={selectedArtifact.path} onPersonClick={onPersonClick} />
            )}
          </div>
        ) : (
          <div className="rounded-2xl border p-4 text-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}>
            先发出一条自然语言请求，右侧面板就会显示对应 artifact。
          </div>
        )}
      </aside>
    </div>
  );
}
