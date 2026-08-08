import { useState } from 'react';
import type { DeleteDraft, InteractionDraft, NlqResponse, Person, PersonDraft, UpdateDraft } from '../types';

type DraftResponse = Extract<NlqResponse, { intentType: 'createPersonDraft' | 'updatePersonDraft' | 'deletePersonDraft' | 'addInteractionDraft' }>;

interface DraftConfirmationProps {
  response: DraftResponse;
  onConfirm: (intentType: string, data: Record<string, unknown>) => void;
  onCancel: () => void;
}

export default function DraftConfirmation({ response, onConfirm, onCancel }: DraftConfirmationProps) {
  if (response.intentType === 'createPersonDraft') {
    return <CreatePersonDraftForm draft={response.draft} onConfirm={onConfirm} onCancel={onCancel} />;
  }
  if (response.intentType === 'updatePersonDraft') {
    return <UpdatePersonDraftForm draft={response.draft} onConfirm={onConfirm} onCancel={onCancel} />;
  }
  if (response.intentType === 'deletePersonDraft') {
    return <DeletePersonDraftForm draft={response.draft} onConfirm={onConfirm} onCancel={onCancel} />;
  }
  if (response.intentType === 'addInteractionDraft') {
    return <AddInteractionDraftForm draft={response.draft} onConfirm={onConfirm} onCancel={onCancel} />;
  }
  return null;
}

// === 新增联系人草稿表单 ===
function CreatePersonDraftForm({
  draft,
  onConfirm,
  onCancel,
}: {
  draft: PersonDraft;
  onConfirm: (intentType: string, data: Record<string, unknown>) => void;
  onCancel: () => void;
}) {
  const [form, setForm] = useState({
    name: draft.name,
    company: draft.company || '',
    title: draft.title || '',
    location: draft.location || '',
    school: draft.school || '',
    background: draft.background || '',
    resourceTags: draft.resourceTags.join(', '),
  });

  const update = (field: string, value: string) => setForm((prev) => ({ ...prev, [field]: value }));

  const handleSubmit = () => {
    onConfirm('createPersonDraft', {
      ...form,
      resourceTags: form.resourceTags
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
    });
  };

  return (
    <div className="rounded-xl border p-4 space-y-3" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
      <div className="flex items-center justify-between">
        <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>新增联系人确认</h3>
        {draft.confidence < 50 && (
          <span className="rounded bg-amber-100 px-2 py-0.5 text-xs text-amber-700">置信度较低，请核实</span>
        )}
      </div>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        <FieldInput label="姓名" value={form.name} onChange={(v) => update('name', v)} />
        <FieldInput label="公司" value={form.company} onChange={(v) => update('company', v)} />
        <FieldInput label="职位" value={form.title} onChange={(v) => update('title', v)} />
        <FieldInput label="地区" value={form.location} onChange={(v) => update('location', v)} />
        <FieldInput label="学校" value={form.school} onChange={(v) => update('school', v)} />
        <FieldInput label="标签（逗号分隔）" value={form.resourceTags} onChange={(v) => update('resourceTags', v)} />
      </div>
      {form.background && (
        <div>
          <label className="text-xs" style={{ color: 'var(--text-secondary)' }}>背景</label>
          <textarea
            className="mt-1 w-full rounded border px-3 py-2 text-sm outline-none"
            style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
            rows={2}
            value={form.background}
            onChange={(e) => update('background', e.target.value)}
          />
        </div>
      )}
      <div className="flex justify-end gap-2 pt-2">
        <button type="button" className="rounded px-4 py-2 text-sm" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }} onClick={onCancel}>
          取消
        </button>
        <button type="button" className="btn-primary rounded px-4 py-2 text-sm" onClick={handleSubmit}>
          确认新增
        </button>
      </div>
    </div>
  );
}

// === 更新联系人草稿表单 ===
function UpdatePersonDraftForm({
  draft,
  onConfirm,
  onCancel,
}: {
  draft: UpdateDraft;
  onConfirm: (intentType: string, data: Record<string, unknown>) => void;
  onCancel: () => void;
}) {
  const [selectedPerson, setSelectedPerson] = useState<Person | undefined>(draft.targetPerson);
  const [changes, setChanges] = useState(draft.changes.map((c) => ({ ...c })));

  const updateChange = (index: number, newValue: string) => {
    setChanges((prev) => prev.map((c, i) => (i === index ? { ...c, newValue } : c)));
  };

  const handleSubmit = () => {
    if (!selectedPerson) return;
    onConfirm('updatePersonDraft', { personId: selectedPerson.id, changes });
  };

  return (
    <div className="rounded-xl border p-4 space-y-3" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
      <div className="flex items-center justify-between">
        <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>更新联系人确认</h3>
        {draft.confidence < 50 && (
          <span className="rounded bg-amber-100 px-2 py-0.5 text-xs text-amber-700">置信度较低，请核实</span>
        )}
      </div>
      {draft.errorHint && (
        <p className="text-sm text-amber-700 bg-amber-50 rounded p-2">{draft.errorHint}</p>
      )}
      {!selectedPerson && draft.candidates.length > 0 && (
        <div className="space-y-1">
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>请选择要更新的联系人：</p>
          <div className="flex flex-wrap gap-2">
            {draft.candidates.map((p) => (
              <button
                key={p.id}
                type="button"
                className="rounded border px-3 py-1 text-sm transition hover:opacity-80"
                style={{ borderColor: 'var(--border-color)', color: 'var(--text-primary)' }}
                onClick={() => setSelectedPerson(p)}
              >
                {p.name}{p.company ? ` (${p.company})` : ''}
              </button>
            ))}
          </div>
        </div>
      )}
      {selectedPerson && (
        <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>
          目标：<span className="font-medium" style={{ color: 'var(--text-primary)' }}>{selectedPerson.name}</span>
        </p>
      )}
      <div className="space-y-2">
        {changes.map((c, i) => (
          <div key={c.field} className="flex items-center gap-2 text-sm">
            <span className="min-w-[60px] font-medium" style={{ color: 'var(--text-secondary)' }}>{c.field}</span>
            {c.oldValue && (
              <span className="line-through" style={{ color: 'var(--text-muted)' }}>{c.oldValue}</span>
            )}
            <span style={{ color: 'var(--text-muted)' }}>→</span>
            <input
              className="flex-1 rounded border px-2 py-1 text-sm outline-none"
              style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
              value={c.newValue}
              onChange={(e) => updateChange(i, e.target.value)}
            />
          </div>
        ))}
      </div>
      <div className="flex justify-end gap-2 pt-2">
        <button type="button" className="rounded px-4 py-2 text-sm" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }} onClick={onCancel}>
          取消
        </button>
        <button type="button" className="btn-primary rounded px-4 py-2 text-sm" disabled={!selectedPerson} onClick={handleSubmit}>
          确认更新
        </button>
      </div>
    </div>
  );
}

// === 新增互动草稿表单 ===
function AddInteractionDraftForm({
  draft,
  onConfirm,
  onCancel,
}: {
  draft: InteractionDraft;
  onConfirm: (intentType: string, data: Record<string, unknown>) => void;
  onCancel: () => void;
}) {
  const [selectedPerson, setSelectedPerson] = useState<Person | undefined>(draft.resolvedPerson);
  const [topic, setTopic] = useState(draft.topic || '');
  const [summary, setSummary] = useState(draft.summary || '');
  const [actionItems, setActionItems] = useState(draft.actionItems.join('\n'));

  const handleSubmit = () => {
    if (!selectedPerson) return;
    onConfirm('addInteractionDraft', {
      personId: selectedPerson.id,
      topic,
      summary,
      actionItems: actionItems.split('\n').map((s) => s.trim()).filter(Boolean),
    });
  };

  return (
    <div className="rounded-xl border p-4 space-y-3" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
      <div className="flex items-center justify-between">
        <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>新增互动确认</h3>
        {draft.confidence < 50 && (
          <span className="rounded bg-amber-100 px-2 py-0.5 text-xs text-amber-700">置信度较低，请核实</span>
        )}
      </div>
      {!selectedPerson && draft.candidates.length > 0 && (
        <div className="space-y-1">
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>
            提及 "<span className="font-medium">{draft.personMention}</span>"，请选择联系人：
          </p>
          <div className="flex flex-wrap gap-2">
            {draft.candidates.map((p) => (
              <button
                key={p.id}
                type="button"
                className="rounded border px-3 py-1 text-sm transition hover:opacity-80"
                style={{ borderColor: 'var(--border-color)', color: 'var(--text-primary)' }}
                onClick={() => setSelectedPerson(p)}
              >
                {p.name}{p.company ? ` (${p.company})` : ''}
              </button>
            ))}
          </div>
        </div>
      )}
      {selectedPerson && (
        <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>
          联系人：<span className="font-medium" style={{ color: 'var(--text-primary)' }}>{selectedPerson.name}</span>
        </p>
      )}
      <FieldInput label="话题" value={topic} onChange={setTopic} />
      <div>
        <label className="text-xs" style={{ color: 'var(--text-secondary)' }}>摘要</label>
        <textarea
          className="mt-1 w-full rounded border px-3 py-2 text-sm outline-none"
          style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
          rows={2}
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
        />
      </div>
      <div>
        <label className="text-xs" style={{ color: 'var(--text-secondary)' }}>待办事项（每行一条）</label>
        <textarea
          className="mt-1 w-full rounded border px-3 py-2 text-sm outline-none"
          style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
          rows={3}
          value={actionItems}
          onChange={(e) => setActionItems(e.target.value)}
        />
      </div>
      <div className="flex justify-end gap-2 pt-2">
        <button type="button" className="rounded px-4 py-2 text-sm" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }} onClick={onCancel}>
          取消
        </button>
        <button type="button" className="btn-primary rounded px-4 py-2 text-sm" disabled={!selectedPerson} onClick={handleSubmit}>
          确认新增互动
        </button>
      </div>
    </div>
  );
}

// === 删除联系人草稿表单 ===
function DeletePersonDraftForm({
  draft,
  onConfirm,
  onCancel,
}: {
  draft: DeleteDraft;
  onConfirm: (intentType: string, data: Record<string, unknown>) => void;
  onCancel: () => void;
}) {
  const [selectedPerson, setSelectedPerson] = useState<Person | undefined>(draft.targetPerson);

  const handleSubmit = () => {
    if (!selectedPerson) return;
    onConfirm('deletePersonDraft', { personId: selectedPerson.id });
  };

  return (
    <div className="rounded-xl border p-4 space-y-3" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
      <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>删除联系人确认</h3>
      {draft.errorHint && (
        <p className="text-sm text-amber-700 bg-amber-50 rounded p-2">{draft.errorHint}</p>
      )}
      {!selectedPerson && draft.candidates.length > 0 && (
        <div className="space-y-1">
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>请选择要删除的联系人：</p>
          <div className="flex flex-wrap gap-2">
            {draft.candidates.map((p) => (
              <button
                key={p.id}
                type="button"
                className="rounded border px-3 py-1 text-sm transition hover:opacity-80"
                style={{ borderColor: 'var(--border-color)', color: 'var(--text-primary)' }}
                onClick={() => setSelectedPerson(p)}
              >
                {p.name}{p.company ? ` (${p.company})` : ''}
              </button>
            ))}
          </div>
        </div>
      )}
      {selectedPerson && (
        <>
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>
            目标：<span className="font-medium" style={{ color: 'var(--text-primary)' }}>{selectedPerson.name}</span>
            {selectedPerson.company ? `（${selectedPerson.company}）` : ''}
          </p>
          <p className="text-sm text-red-600 bg-red-50 rounded p-2">删除后其互动记录与关系也会一并删除，且不可恢复。</p>
        </>
      )}
      <div className="flex justify-end gap-2 pt-2">
        <button type="button" className="rounded px-4 py-2 text-sm" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }} onClick={onCancel}>
          取消
        </button>
        <button
          type="button"
          className="rounded px-4 py-2 text-sm text-white"
          style={{ backgroundColor: '#dc2626' }}
          disabled={!selectedPerson}
          onClick={handleSubmit}
        >
          确认删除
        </button>
      </div>
    </div>
  );
}

// === 通用字段输入组件 ===
function FieldInput({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <div>
      <label className="text-xs" style={{ color: 'var(--text-secondary)' }}>{label}</label>
      <input
        className="mt-1 w-full rounded border px-3 py-2 text-sm outline-none"
        style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
