import type { Interaction, Person } from '../types';
import PersonCard from './PersonCard';

interface Props {
  persons: Person[];
  selectedPersonId?: string;
  interactionsByPerson?: Record<string, Interaction[]>;
  onSelect: (person: Person) => void;
}

export default function PersonList({ persons, selectedPersonId, interactionsByPerson = {}, onSelect }: Props) {
  if (persons.length === 0) {
    return <div className="rounded-xl border border-dashed p-8 text-center text-slate-500">暂无联系人，先从左侧新增一位。</div>;
  }

  return (
    <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
      {persons.map((person) => (
        <PersonCard
          key={person.id}
          person={person}
          selected={selectedPersonId === person.id}
          lastInteraction={interactionsByPerson[person.id]?.[0] ?? null}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}
