import { useState } from 'react';
import type { CreatePersonInput, RelationshipStrength, SensitivityLevel } from '../types';

interface Props {
  onSubmit: (input: CreatePersonInput) => Promise<void> | void;
}

export default function PersonForm({ onSubmit }: Props) {
  const [form, setForm] = useState({
    name: '',
    aliases: '',
    phone: '',
    email: '',
    company: '',
    title: '',
    location: '',
    background: '',
    relationshipStrength: 'medium' as RelationshipStrength,
    resourceTags: '',
    sensitivityLevel: 'low' as SensitivityLevel,
    status: 'active' as CreatePersonInput['status'],
    nextStep: '',
    notes: '',
  });
  const [submitting, setSubmitting] = useState(false);

  const update = (key: keyof typeof form, value: string) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    try {
      await onSubmit({
        name: form.name.trim(),
        aliases: splitList(form.aliases),
        phone: emptyToNull(form.phone),
        email: emptyToNull(form.email),
        company: emptyToNull(form.company),
        title: emptyToNull(form.title),
        location: emptyToNull(form.location),
        background: emptyToNull(form.background),
        relationshipStrength: form.relationshipStrength,
        resourceTags: splitList(form.resourceTags),
        sensitivityLevel: form.sensitivityLevel,
        status: form.status,
        nextStep: emptyToNull(form.nextStep),
        notes: emptyToNull(form.notes),
      });
      setForm((prev) => ({ ...prev, name: '', aliases: '', phone: '', email: '', background: '', nextStep: '', notes: '' }));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3 rounded-xl border bg-white p-4 shadow-sm">
      <h2 className="text-lg font-semibold">新增联系人</h2>
      <input className="input" placeholder="姓名" value={form.name} onChange={(e) => update('name', e.target.value)} required />
      <input className="input" placeholder="昵称/代称，如：老张、张工" value={form.aliases} onChange={(e) => update('aliases', e.target.value)} />
      <div className="grid grid-cols-2 gap-3">
        <input className="input" placeholder="公司" value={form.company} onChange={(e) => update('company', e.target.value)} />
        <input className="input" placeholder="职位" value={form.title} onChange={(e) => update('title', e.target.value)} />
        <input className="input" placeholder="城市/地域" value={form.location} onChange={(e) => update('location', e.target.value)} />
        <input className="input" placeholder="电话" value={form.phone} onChange={(e) => update('phone', e.target.value)} />
      </div>
      <input className="input" placeholder="邮箱" value={form.email} onChange={(e) => update('email', e.target.value)} />
      <textarea className="input min-h-20" placeholder="认识背景" value={form.background} onChange={(e) => update('background', e.target.value)} />
      <div className="grid grid-cols-3 gap-3">
        <select className="input" value={form.relationshipStrength} onChange={(e) => update('relationshipStrength', e.target.value)}>
          <option value="strong">关系强</option>
          <option value="medium">关系中</option>
          <option value="weak">关系弱</option>
        </select>
        <select className="input" value={form.sensitivityLevel} onChange={(e) => update('sensitivityLevel', e.target.value)}>
          <option value="low">低敏感</option>
          <option value="medium">中敏感</option>
          <option value="high">高敏感</option>
        </select>
        <select className="input" value={form.status} onChange={(e) => update('status', e.target.value)}>
          <option value="active">活跃</option>
          <option value="follow-up">待跟进</option>
          <option value="cold">冷却</option>
        </select>
      </div>
      <input className="input" placeholder="资源标签，逗号分隔，如：地产,融资" value={form.resourceTags} onChange={(e) => update('resourceTags', e.target.value)} />
      <input className="input" placeholder="下一步" value={form.nextStep} onChange={(e) => update('nextStep', e.target.value)} />
      <textarea className="input min-h-20" placeholder="备注" value={form.notes} onChange={(e) => update('notes', e.target.value)} />
      <button className="btn-primary w-full" type="submit" disabled={submitting}>{submitting ? '保存中...' : '保存联系人'}</button>
    </form>
  );
}

function splitList(value: string): string[] {
  return value.split(/[,，]/).map((item) => item.trim()).filter(Boolean);
}

function emptyToNull(value: string): string | null {
  return value.trim() ? value.trim() : null;
}
