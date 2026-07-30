import { useEffect, useMemo, useState } from 'react';
import GraphView from './components/GraphView';
import InteractionForm from './components/InteractionForm';
import NaturalLanguageQuery from './components/NaturalLanguageQuery';
import PersonForm from './components/PersonForm';
import PersonList from './components/PersonList';
import RelationshipForm from './components/RelationshipForm';
import { createPerson, getGraphData, listInteractionsByPerson, listPersons } from './services/db';
import type { CreatePersonInput, GraphData, Interaction, Person } from './types';

function App() {
  const [activeTab, setActiveTab] = useState<'contacts' | 'graph' | 'query'>('contacts');
  const [persons, setPersons] = useState<Person[]>([]);
  const [selectedPerson, setSelectedPerson] = useState<Person | null>(null);
  const [interactionsByPerson, setInteractionsByPerson] = useState<Record<string, Interaction[]>>({});
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [error, setError] = useState('');

  const selectedInteractions = useMemo(
    () => (selectedPerson ? interactionsByPerson[selectedPerson.id] || [] : []),
    [interactionsByPerson, selectedPerson],
  );

  const loadData = async () => {
    try {
      const list = await listPersons();
      setPersons(list);
      if (!selectedPerson && list.length > 0) {
        setSelectedPerson(list[0]);
      }
      const pairs = await Promise.all(list.map(async (person) => [person.id, await listInteractionsByPerson(person.id)] as const));
      setInteractionsByPerson(Object.fromEntries(pairs));
      setGraphData(await getGraphData());
    } catch (err) {
      setError(String(err));
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleCreatePerson = async (input: CreatePersonInput) => {
    const created = await createPerson(input);
    setSelectedPerson(created);
    await loadData();
  };

  const handleGraphNodeClick = (id: string) => {
    const person = persons.find((item) => item.id === id);
    if (person) {
      setSelectedPerson(person);
      setActiveTab('contacts');
    }
  };

  return (
    <div className="min-h-screen bg-slate-100 text-slate-900">
      <header className="border-b bg-white px-6 py-4 shadow-sm">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold">个人关系图谱</h1>
            <p className="text-sm text-slate-500">本地优先、加密存储、端侧智能辅助</p>
          </div>
          <nav className="flex gap-2">
            <TabButton active={activeTab === 'contacts'} onClick={() => setActiveTab('contacts')}>联系人</TabButton>
            <TabButton active={activeTab === 'graph'} onClick={() => setActiveTab('graph')}>图谱</TabButton>
            <TabButton active={activeTab === 'query'} onClick={() => setActiveTab('query')}>AI 查询</TabButton>
          </nav>
        </div>
      </header>

      <main className="mx-auto max-w-7xl p-6">
        {error && <div className="mb-4 rounded bg-red-50 p-3 text-sm text-red-700">{error}</div>}
        {activeTab === 'contacts' && (
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-[360px_1fr]">
            <aside className="space-y-4">
              <PersonForm onSubmit={handleCreatePerson} />
              <RelationshipForm persons={persons} onCreated={loadData} />
              <InteractionForm person={selectedPerson} onCreated={loadData} />
            </aside>
            <section className="space-y-4">
              <div className="flex items-center justify-between">
                <h2 className="text-xl font-semibold">联系人名片</h2>
                <span className="text-sm text-slate-500">共 {persons.length} 人</span>
              </div>
              <PersonList
                persons={persons}
                selectedPersonId={selectedPerson?.id}
                interactionsByPerson={interactionsByPerson}
                onSelect={setSelectedPerson}
              />
              {selectedPerson && (
                <div className="rounded-xl border bg-white p-4 shadow-sm">
                  <h3 className="font-semibold">{selectedPerson.name} 的互动记录</h3>
                  <div className="mt-3 space-y-3">
                    {selectedInteractions.length === 0 ? (
                      <p className="text-sm text-slate-500">暂无互动记录。</p>
                    ) : selectedInteractions.map((interaction) => (
                      <div key={interaction.id} className="rounded-lg bg-slate-50 p-3 text-sm text-slate-700">
                        <p className="font-medium">{new Date(interaction.timestamp).toLocaleString('zh-CN')}</p>
                        <p className="mt-1">{interaction.summary || interaction.content}</p>
                        <p className="mt-1 text-slate-500">话题：{interaction.topics.join('、') || '无'}</p>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </section>
          </div>
        )}

        {activeTab === 'graph' && <GraphView data={graphData} onNodeClick={handleGraphNodeClick} />}
        {activeTab === 'query' && <NaturalLanguageQuery />}
      </main>
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-4 py-2 text-sm font-medium ${active ? 'bg-blue-600 text-white' : 'bg-slate-100 text-slate-600 hover:bg-slate-200'}`}
    >
      {children}
    </button>
  );
}

export default App;
