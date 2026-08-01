import type { Interaction, Person } from '../types';
import SensitivityGuard from './SensitivityGuard';

interface Props {
  person: Person;
  lastInteraction?: Interaction | null;
  selected?: boolean;
  onSelect?: (person: Person) => void;
}

export default function PersonCard({ person, lastInteraction, selected, onSelect }: Props) {
  const alias = person.aliases[0] || '未设置代称';
  const displayName = person.sensitivityLevel === 'low' ? person.name : alias;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onSelect?.(person)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          onSelect?.(person);
        }
      }}
      className={`w-full cursor-pointer rounded-xl border bg-white p-4 text-left shadow-sm transition hover:shadow-md ${selected ? 'border-blue-500 ring-2 ring-blue-100' : 'border-slate-200'}`}
    >
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-xl font-semibold text-slate-900">{displayName}</h3>
          <p className="mt-1 text-sm text-slate-500">昵称/代称：{alias}</p>
        </div>
        <span className={`badge ${sensitivityClass(person.sensitivityLevel)}`}>{sensitivityText(person.sensitivityLevel)}</span>
      </div>

      {person.sensitivityLevel !== 'low' && (
        <div className="mt-3">
          <SensitivityGuard level={person.sensitivityLevel} fallback={<span>真实姓名已隐藏</span>}>
            <p className="text-sm text-slate-700">真实姓名：{person.name}</p>
            {person.phone && <p className="text-sm text-slate-700">电话：{person.phone}</p>}
            {person.email && <p className="text-sm text-slate-700">邮箱：{person.email}</p>}
          </SensitivityGuard>
        </div>
      )}

      <dl className="mt-4 space-y-2 text-sm text-slate-700">
        <Row label="公司/职位" value={[person.company, person.title].filter(Boolean).join(' / ') || '未填写'} />
        <Row label="认识背景" value={person.background || '未填写'} />
        <Row label="关系强度" value={strengthText(person.relationshipStrength)} />
        <Row label="上次互动" value={lastInteraction ? `${formatDate(lastInteraction.timestamp)}，${lastInteraction.summary || lastInteraction.content}` : '暂无记录'} />
        <Row label="当前状态" value={statusText(person.status)} />
        <Row label="下一步" value={person.nextStep || '未填写'} />
        <Row label="备注" value={person.notes || '未填写'} />
      </dl>

      <div className="mt-4 flex flex-wrap gap-2">
        {person.resourceTags.length === 0 ? (
          <span className="badge bg-slate-100 text-slate-600">无标签</span>
        ) : (
          person.resourceTags.map((tag) => <span key={tag} className="badge bg-blue-50 text-blue-700">{tag}</span>)
        )}
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[5rem_1fr] gap-3">
      <dt className="text-slate-400">{label}</dt>
      <dd className="line-clamp-2">{value}</dd>
    </div>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleDateString('zh-CN');
}

function strengthText(value?: string | null) {
  if (value === 'strong') return '强';
  if (value === 'weak') return '弱';
  return '中';
}

function sensitivityText(value: string) {
  if (value === 'high') return '高敏感';
  if (value === 'medium') return '中敏感';
  return '低敏感';
}

function sensitivityClass(value: string) {
  if (value === 'high') return 'bg-red-50 text-red-700';
  if (value === 'medium') return 'bg-amber-50 text-amber-700';
  return 'bg-green-50 text-green-700';
}

function statusText(value: string) {
  if (value === 'follow-up') return '待跟进';
  if (value === 'cold') return '冷却';
  return '活跃';
}
