import { useEffect, useState } from 'react';
import type { Interaction, Person } from '../types';
import PersonCard from './PersonCard';

interface Props {
  persons: Person[];
  selectedPersonId?: string;
  interactionsByPerson?: Record<string, Interaction[]>;
  onSelect: (person: Person) => void;
}

const PAGE_SIZE = 30;

export default function PersonList({ persons, selectedPersonId, interactionsByPerson = {}, onSelect }: Props) {
  const [keyword, setKeyword] = useState('');
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);

  // 搜索词变化时重置分页
  useEffect(() => {
    setVisibleCount(PAGE_SIZE);
  }, [keyword]);

  if (persons.length === 0) {
    return <div className="rounded-xl border border-dashed p-8 text-center text-slate-500">暂无联系人，先从左侧新增一位。</div>;
  }

  const terms = keyword.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const filtered = terms.length === 0
    ? persons
    : persons.filter((person) => {
        const text = [
          person.name,
          person.aliases.join(' '),
          person.company ?? '',
          person.title ?? '',
          person.location ?? '',
          person.resourceTags.join(' '),
        ].join(' ').toLowerCase();
        return terms.every((term) => text.includes(term));
      });
  const visible = filtered.slice(0, visibleCount);

  return (
    <div className="space-y-4">
      {persons.length > PAGE_SIZE && (
        <input
          type="search"
          className="input"
          placeholder="快速筛选：姓名、公司、城市、标签…"
          value={keyword}
          onChange={(event) => setKeyword(event.target.value)}
        />
      )}
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        {visible.map((person) => (
          <PersonCard
            key={person.id}
            person={person}
            selected={selectedPersonId === person.id}
            lastInteraction={interactionsByPerson[person.id]?.[0] ?? null}
            onSelect={onSelect}
          />
        ))}
      </div>
      {filtered.length === 0 && <p className="py-8 text-center text-slate-400">没有匹配的联系人</p>}
      {filtered.length > visibleCount && (
        <button
          type="button"
          className="w-full rounded-lg border border-dashed py-2 text-sm text-slate-600 hover:bg-slate-50"
          onClick={() => setVisibleCount((count) => count + PAGE_SIZE)}
        >
          加载更多（已显示 {visible.length} / {filtered.length}）
        </button>
      )}
    </div>
  );
}
