import { useState } from 'react';
import { createRelationship } from '../services/db';
import type { Person, RelationshipStrength } from '../types';
import { RELATIONSHIP_TYPES, HOW_ESTABLISHED } from '../types';

interface Props {
  persons: Person[];
  onCreated: () => void;
}

export default function RelationshipForm({ persons, onCreated }: Props) {
  const [fromPersonId, setFromPersonId] = useState('');
  const [toPersonId, setToPersonId] = useState('');
  const [relationshipType, setRelationshipType] = useState('colleague');
  const [strength, setStrength] = useState<RelationshipStrength>('medium');
  const [description, setDescription] = useState('');
  const [howEstablished, setHowEstablished] = useState('');
  const [establishedDate, setEstablishedDate] = useState('');
  const [strengthRating, setStrengthRating] = useState(0.5);
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
      relationshipType,
      strength,
      description: description || null,
      howEstablished: howEstablished || null,
      establishedDate: establishedDate || null,
      strengthRating,
    });
    setDescription('');
    setHowEstablished('');
    setEstablishedDate('');
    setStrengthRating(0.5);
    onCreated();
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3 rounded-xl border bg-white p-4 shadow-sm">
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
          {Object.entries(RELATIONSHIP_TYPES).map(([key, label]) => (
            <option key={key} value={key}>{label}</option>
          ))}
        </select>
        <select className="input" value={strength} onChange={(event) => setStrength(event.target.value as RelationshipStrength)}>
          <option value="strong">强</option>
          <option value="medium">中</option>
          <option value="weak">弱</option>
        </select>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <select className="input" value={howEstablished} onChange={(event) => setHowEstablished(event.target.value)}>
          <option value="">建立方式（可选）</option>
          {Object.entries(HOW_ESTABLISHED).map(([key, label]) => (
            <option key={key} value={key}>{label}</option>
          ))}
        </select>
        <input
          type="date"
          className="input"
          placeholder="建立日期"
          value={establishedDate}
          onChange={(event) => setEstablishedDate(event.target.value)}
        />
      </div>
      <div>
        <label className="flex items-center gap-3 text-sm text-slate-600">
          <span className="w-20 shrink-0">关系强度</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.1"
            value={strengthRating}
            onChange={(event) => setStrengthRating(parseFloat(event.target.value))}
            className="flex-1"
          />
          <span className="w-10 text-right font-mono text-xs">{strengthRating.toFixed(1)}</span>
        </label>
      </div>
      <input className="input" placeholder="关系说明" value={description} onChange={(event) => setDescription(event.target.value)} />
      {error && <p className="text-sm text-red-600">{error}</p>}
      <button className="btn-primary w-full" type="submit">保存关系</button>
    </form>
  );
}
