import { useEffect, useState } from 'react';
import { searchPersonCandidates } from '../services/db';
import type { Person } from '../types';

interface Mention {
  mention: string;
  confidence: number;
}

interface Props {
  mentions: Mention[];
  onResolved: (resolved: Record<string, string | null>) => void;
}

export default function EntityResolver({ mentions, onResolved }: Props) {
  const [candidates, setCandidates] = useState<Record<string, Person[]>>({});
  const [resolved, setResolved] = useState<Record<string, string | null>>({});

  useEffect(() => {
    async function load() {
      const entries = await Promise.all(
        mentions.map(async (mention) => [mention.mention, await searchPersonCandidates(mention.mention)] as const),
      );
      setCandidates(Object.fromEntries(entries));
    }
    if (mentions.length > 0) load();
  }, [mentions]);

  const setChoice = (mention: string, personId: string | null) => {
    const next = { ...resolved, [mention]: personId };
    setResolved(next);
    onResolved(next);
  };

  if (mentions.length === 0) return null;

  return (
    <div className="space-y-3 rounded-lg border bg-card p-3">
      <h4 className="font-medium text-text-primary">确认提到的人</h4>
      {mentions.map((mention) => {
        const list = candidates[mention.mention] || [];
        return (
          <div key={mention.mention} className="rounded-md bg-secondary p-3">
            <div className="flex items-center justify-between">
              <span className="font-medium">“{mention.mention}”</span>
              <span className="text-xs text-text-secondary">置信度 {Math.round(mention.confidence * 100)}%</span>
            </div>
            {list.length === 0 ? (
              <p className="mt-2 text-sm text-text-secondary">没有找到候选联系人，可先保存原始记录。</p>
            ) : (
              <div className="mt-2 flex flex-wrap gap-2">
                {list.map((person) => (
                  <button
                    key={person.id}
                    type="button"
                    className={`rounded border px-3 py-1 text-sm ${resolved[mention.mention] === person.id ? 'border-accent bg-accent text-white' : 'bg-card text-text-primary'}`}
                    onClick={() => setChoice(mention.mention, person.id)}
                  >
                    {person.name} {person.company ? `｜${person.company}` : ''}
                  </button>
                ))}
                <button type="button" className="rounded border bg-card px-3 py-1 text-sm text-text-secondary" onClick={() => setChoice(mention.mention, null)}>
                  忽略
                </button>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
