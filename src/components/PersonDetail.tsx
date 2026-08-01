import { useEffect, useState } from 'react';
import {
  deletePerson,
  getPerson,
  listInteractionsByPerson,
  listRelationshipsByPerson,
  updatePerson,
} from '../services/db';
import type { CreatePersonInput, Interaction, Person, Relationship } from '../types';
import PersonForm from './PersonForm';
import SensitivityGuard from './SensitivityGuard';

interface Props {
  personId: string;
  personsById: Record<string, Person>;
  onBack: () => void;
  /** 编辑/删除成功后通知上层刷新全局数据 */
  onChanged: () => void;
  /** 点击关系中的联系人时跳转到其详情页 */
  onOpenPerson: (id: string) => void;
  /** 点击"关系网络"按钮时跳转到图谱页并聚焦该联系人 */
  onNetworkView?: (personId: string) => void;
}

export default function PersonDetail({ personId, personsById, onBack, onChanged, onOpenPerson, onNetworkView }: Props) {
  const [person, setPerson] = useState<Person | null>(null);
  const [interactions, setInteractions] = useState<Interaction[]>([]);
  const [relationships, setRelationships] = useState<Relationship[]>([]);
  const [editing, setEditing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const load = async () => {
    setLoading(true);
    setError('');
    try {
      const [detail, interactionList, relationshipList] = await Promise.all([
        getPerson(personId),
        listInteractionsByPerson(personId),
        listRelationshipsByPerson(personId),
      ]);
      if (!detail) {
        throw new Error('联系人不存在或已被删除');
      }
      setPerson(detail);
      setInteractions(interactionList);
      setRelationships(relationshipList);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
    setEditing(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [personId]);

  const handleUpdate = async (input: CreatePersonInput) => {
    await updatePerson(personId, input);
    setEditing(false);
    await load();
    onChanged();
  };

  const handleDelete = async () => {
    if (!person) return;
    const label = person.sensitivityLevel === 'low' ? person.name : person.aliases[0] || '该联系人';
    if (!window.confirm(`确定删除「${label}」吗？其互动记录与关系也会一并删除，且不可恢复。`)) {
      return;
    }
    try {
      await deletePerson(personId);
      onChanged();
      onBack();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  if (loading) {
    return <div className="rounded-xl border p-8 text-center" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}>加载联系人详情...</div>;
  }

  if (error || !person) {
    return (
      <div className="rounded-xl border p-8 text-center" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
        <p className="text-red-600">{error || '联系人不存在'}</p>
        <button type="button" className="btn-primary mt-4" onClick={onBack}>返回</button>
      </div>
    );
  }

  const sensitive = person.sensitivityLevel !== 'low';
  const displayName = sensitive ? person.aliases[0] || '高敏感联系人' : person.name;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <button type="button" className="rounded px-3 py-1.5 text-sm transition" style={{ color: 'var(--text-secondary)' }} onClick={onBack}>
          ← 返回
        </button>
        <div className="flex gap-2">
          {onNetworkView && (
            <button
              type="button"
              className="rounded bg-purple-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-purple-700"
              onClick={() => onNetworkView(personId)}
            >
              关系网络
            </button>
          )}
          <button
            type="button"
            className="rounded bg-blue-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-blue-700"
            onClick={() => setEditing((prev) => !prev)}
          >
            {editing ? '取消编辑' : '编辑资料'}
          </button>
          <button
            type="button"
            className="rounded bg-red-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-red-700"
            onClick={handleDelete}
          >
            删除联系人
          </button>
        </div>
      </div>

      {editing ? (
        <PersonForm initial={person} heading={`编辑「${displayName}」`} submitLabel="保存修改" onSubmit={handleUpdate} />
      ) : (
        <div className="rounded-xl border p-6 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>{displayName}</h2>
              <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>昵称/代称：{person.aliases.join('、') || '未设置'}</p>
            </div>
            <span className={`badge ${sensitivityClass(person.sensitivityLevel)}`}>{sensitivityText(person.sensitivityLevel)}</span>
          </div>

          {sensitive && (
            <div className="mt-4">
              <SensitivityGuard level={person.sensitivityLevel} fallback={<span>真实姓名与联系方式已隐藏</span>}>
                <div className="rounded-md bg-slate-50 p-3 text-sm text-slate-700">
                  <p>真实姓名：{person.name}</p>
                  {person.phone && <p>电话：{person.phone}</p>}
                  {person.email && <p>邮箱：{person.email}</p>}
                </div>
              </SensitivityGuard>
            </div>
          )}

          <dl className="mt-5 grid grid-cols-1 gap-x-8 gap-y-3 text-sm sm:grid-cols-2" style={{ color: 'var(--text-secondary)' }}>
            {!sensitive && <Field label="电话" value={person.phone} />}
            {!sensitive && <Field label="邮箱" value={person.email} />}
            <Field label="公司" value={person.company} />
            <Field label="职位" value={person.title} />
            <Field label="城市/地域" value={person.location} />
            <Field label="学校" value={person.school} />
            <Field label="参与项目" value={person.projects.length > 0 ? person.projects.join('、') : null} />
            <Field label="关系强度" value={strengthText(person.relationshipStrength)} />
            <Field label="当前状态" value={statusText(person.status)} />
            <Field label="下一步" value={person.nextStep} />
            <Field label="认识背景" value={person.background} wide />
            <Field label="备注" value={person.notes} wide />
            <Field label="创建时间" value={formatDate(person.createdAt)} />
            <Field label="最近更新" value={formatDate(person.updatedAt)} />
          </dl>

          <div className="mt-4 flex flex-wrap gap-2">
            {person.resourceTags.length === 0 ? (
              <span className="badge bg-slate-100 text-slate-600">无标签</span>
            ) : (
              person.resourceTags.map((tag) => <span key={tag} className="badge bg-blue-50 text-blue-700">{tag}</span>)
            )}
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <div className="rounded-xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <h3 className="font-semibold">关系（{relationships.length}）</h3>
          <div className="mt-3 space-y-2 text-sm">
            {relationships.length === 0 ? (
              <p style={{ color: 'var(--text-secondary)' }}>暂无关系记录。</p>
            ) : (
              relationships.map((rel) => {
                const otherId = rel.fromPersonId === personId ? rel.toPersonId : rel.fromPersonId;
                const other = personsById[otherId];
                const otherName = other
                  ? other.sensitivityLevel === 'low' ? other.name : other.aliases[0] || '高敏感联系人'
                  : '未知联系人';
                return (
                  <button
                    key={rel.id}
                    type="button"
                    disabled={!other}
                    onClick={() => other && onOpenPerson(otherId)}
                    className={`block w-full rounded-lg bg-slate-50 p-3 text-left ${other ? 'cursor-pointer transition hover:bg-blue-50' : 'cursor-default'}`}
                  >
                    <span className={`font-medium ${other ? 'text-blue-700 hover:underline' : ''}`}>{otherName}</span>
                    <span className="ml-2 text-slate-500">{relationshipTypeText(rel.relationshipType)}{rel.description ? `（${rel.description}）` : ''}</span>
                  </button>
                );
              })
            )}
          </div>
        </div>

        <div className="rounded-xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <h3 className="font-semibold">互动记录（{interactions.length}）</h3>
          <div className="mt-3 max-h-80 space-y-2 overflow-auto text-sm">
            {interactions.length === 0 ? (
              <p style={{ color: 'var(--text-secondary)' }}>暂无互动记录。</p>
            ) : (
              interactions.map((interaction) => (
                <div key={interaction.id} className="rounded-lg p-3" style={{ backgroundColor: 'var(--bg-secondary)' }}>
                  <p className="font-medium">{new Date(interaction.timestamp).toLocaleString('zh-CN')}</p>
                  <p className="mt-1" style={{ color: 'var(--text-secondary)' }}>{interaction.summary || interaction.content}</p>
                  {interaction.topics.length > 0 && (
                    <p className="mt-1" style={{ color: 'var(--text-muted)' }}>话题：{interaction.topics.join('、')}</p>
                  )}
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Field({ label, value, wide }: { label: string; value?: string | null; wide?: boolean }) {
  return (
    <div className={`grid grid-cols-[6rem_1fr] gap-3 ${wide ? 'sm:col-span-2' : ''}`}>
      <dt style={{ color: 'var(--text-muted)' }}>{label}</dt>
      <dd className="whitespace-pre-wrap">{value || '未填写'}</dd>
    </div>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleString('zh-CN');
}

function strengthText(value?: string | null) {
  if (value === 'strong') return '强';
  if (value === 'weak') return '弱';
  if (value === 'medium') return '中';
  return '未标注';
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

function relationshipTypeText(value: string) {
  const map: Record<string, string> = {
    introduced: '介绍认识',
    colleague: '同事',
    friend: '朋友',
    cooperation: '合作',
    other: '其他',
  };
  return map[value] ?? value;
}
