import { useState } from 'react';
import { createRelationship } from '../services/db';
import type { Person, RelationshipStrength } from '../types';

interface Props {
  persons: Person[];
  onCreated: () => void;
}

export default function RelationshipForm({ persons, onCreated }: Props) {
  const [fromPersonId, setFromPersonId] = useState('');
  const [toPersonId, setToPersonId] = useState('');
  const [relationshipType, setRelationshipType] = useState('introduced');
  const [strength, setStrength] = useState<RelationshipStrength>('medium');
  const [description, setDescription] = useState('');
  const [error, setError] = useState('');

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!fromPersonId || !toPersonId || fromPersonId === toPersonId) {
      setError('请选择两个不同联系人');
      return;
    }
    setError('');
    await createRelationship({
      fromPersonId,
      toPersonId,
      relationshipType: relationshipType as 'introduced' | 'colleague' | 'friend' | 'cooperation' | 'other',
      strength,
      description: description || null,
    });
    setDescription('');
    onCreated();
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3 rounded-xl border bg-card p-4 shadow-sm">
      <h2 className="text-lg font-semibold">关系链路</h2>
      <div className="grid grid-cols-2 gap-3">
        <select className="input" value={fromPersonId} onChange={(event) => setFromPersonId(event.target.value)}>
          <option value="">关系起点/介绍人</option>
          {persons.map((person) => <option key={person.id} value={person.id}>{person.name}</option>)}
        </select>
        <select className="input" value={toPersonId} onChange={(event) => setToPersonId(event.target.value)}>
          <option value="">关系终点/被介绍人</option>
          {persons.map((person) => <option key={person.id} value={person.id}>{person.name}</option>)}
        </select>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <select className="input" value={relationshipType} onChange={(event) => setRelationshipType(event.target.value)}>
          <option value="introduced">介绍</option>
          <option value="colleague">同事</option>
          <option value="friend">朋友</option>
          <option value="cooperation">合作</option>
          <option value="other">其他</option>
        </select>
        <select className="input" value={strength} onChange={(event) => setStrength(event.target.value as RelationshipStrength)}>
          <option value="strong">强</option>
          <option value="medium">中</option>
          <option value="weak">弱</option>
        </select>
      </div>
      <input className="input" placeholder="关系说明" value={description} onChange={(event) => setDescription(event.target.value)} />
      {error && <p className="text-sm text-danger">{error}</p>}
      <button className="btn-primary w-full" type="submit">保存关系</button>
    </form>
  );
}
