import { useState } from 'react';
import { naturalLanguageQuery } from '../services/db';
import type { NlqResult } from '../types';
import NlqResultCard from './NlqResultCard';

export const NLQ_EXAMPLES = [
  '谁在上海做地产，和我关系比较近？',
  '上次聊过融资的人里，还没跟进的有谁？',
  '这个懂车帝的投标，谁能帮上忙？',
  '最近3个月没联系但标记了待跟进的人有哪些？',
];

export default function NaturalLanguageQuery() {
  const [query, setQuery] = useState(NLQ_EXAMPLES[0]);
  const [results, setResults] = useState<NlqResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setLoading(true);
    setError('');
    try {
      setResults(await naturalLanguageQuery(query));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <section className="space-y-4 rounded-xl border bg-white p-4 shadow-sm">
      <div>
        <h2 className="text-lg font-semibold">自然语言查询</h2>
        <p className="mt-1 text-sm text-slate-500">初版限定查询类型，后端只执行白名单规则，不让模型直接生成 SQL。</p>
      </div>
      <form onSubmit={handleSubmit} className="flex gap-2">
        <input className="input" value={query} onChange={(event) => setQuery(event.target.value)} />
        <button className="btn-primary whitespace-nowrap" type="submit" disabled={loading}>{loading ? '查询中...' : '查询'}</button>
      </form>
      <div className="flex flex-wrap gap-2">
        {NLQ_EXAMPLES.map((example) => (
          <button key={example} type="button" className="rounded-full bg-slate-100 px-3 py-1 text-sm text-slate-600 hover:bg-slate-200" onClick={() => setQuery(example)}>
            {example}
          </button>
        ))}
      </div>
      {error && <p className="rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}
      <div className="space-y-3">
        {results.map((result) => (
          <NlqResultCard key={result.personId} result={result} />
        ))}
      </div>
    </section>
  );
}
