import { useState } from 'react';
import { createEntityMention, createInteraction } from '../services/db';
import { extractFromText } from '../services/ollama';
import type { Interaction, Person } from '../types';
import EntityResolver from './EntityResolver';
import VoiceRecorder from './VoiceRecorder';

interface Props {
  person: Person | null;
  onCreated: (interaction: Interaction) => void;
}

export default function InteractionForm({ person, onCreated }: Props) {
  const [content, setContent] = useState('');
  const [summary, setSummary] = useState('');
  const [topics, setTopics] = useState<string[]>([]);
  const [actionItems, setActionItems] = useState<string[]>([]);
  const [mentions, setMentions] = useState<{ mention: string; confidence: number }[]>([]);
  const [resolved, setResolved] = useState<Record<string, string | null>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleAnalyze = async (text = content) => {
    if (!text.trim()) return;
    setLoading(true);
    setError('');
    try {
      const result = await extractFromText(text);
      setSummary(result.summary);
      setTopics(result.topics);
      setActionItems(result.actionItems);
      setMentions(result.persons);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleTranscript = (text: string) => {
    // 语音结果追加到已有内容，不覆盖用户手动输入
    const combined = content.trim() ? `${content.trim()}\n${text}` : text;
    setContent(combined);
    handleAnalyze(combined);
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!person) {
      setError('请先选择联系人');
      return;
    }
    setLoading(true);
    setError('');
    try {
      const interaction = await createInteraction({
        personId: person.id,
        timestamp: new Date().toISOString(),
        content,
        summary: summary || content.slice(0, 80),
        topics,
        actionItems,
      });

      await Promise.all(
        mentions.map((mention) => createEntityMention({
          interactionId: interaction.id,
          personId: resolved[mention.mention] ?? null,
          mentionText: mention.mention,
          confidence: mention.confidence,
          resolved: Object.prototype.hasOwnProperty.call(resolved, mention.mention),
        })),
      );

      setContent('');
      setSummary('');
      setTopics([]);
      setActionItems([]);
      setMentions([]);
      setResolved({});
      onCreated(interaction);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3 rounded-xl border bg-white p-4 shadow-sm">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">互动记录</h2>
        <span className="text-sm text-slate-500">{person ? `当前联系人：${person.name}` : '请先选择联系人'}</span>
      </div>
      <VoiceRecorder onTranscript={handleTranscript} />
      <textarea
        className="input min-h-32"
        placeholder="输入或粘贴沟通内容，例如：今天和老张聊了懂车帝投标，他建议先找设计圈资源..."
        value={content}
        onChange={(event) => setContent(event.target.value)}
      />
      <div className="flex gap-2">
        <button type="button" className="btn-secondary" onClick={() => handleAnalyze()} disabled={loading || !content.trim()}>
          {loading ? '处理中...' : '提取人物/话题/待办'}
        </button>
        <button type="submit" className="btn-primary" disabled={loading || !person || !content.trim()}>保存互动</button>
      </div>
      {summary && (
        <div className="rounded-lg bg-slate-50 p-3 text-sm text-slate-700">
          <p><span className="font-medium">摘要：</span>{summary}</p>
          <p><span className="font-medium">话题：</span>{topics.join('、') || '无'}</p>
          <p><span className="font-medium">待办：</span>{actionItems.join('、') || '无'}</p>
        </div>
      )}
      <EntityResolver mentions={mentions} onResolved={setResolved} />
      {error && <p className="rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}
    </form>
  );
}
