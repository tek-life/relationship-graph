import { useEffect, useMemo, useState } from 'react';
import AuthPage from './components/AuthPage';
import GraphView from './components/GraphView';
import ImportWizard from './components/ImportWizard';
import InteractionForm from './components/InteractionForm';
import MultimodalQuery from './components/MultimodalQuery';
import NaturalLanguageQuery from './components/NaturalLanguageQuery';
import OnboardingWizard, { isOnboardingCompleted } from './components/OnboardingWizard';
import PersonDetail from './components/PersonDetail';
import PersonForm from './components/PersonForm';
import PersonList from './components/PersonList';
import RelationshipForm from './components/RelationshipForm';
import ThemeSelector from './components/ThemeSelector';
import { useAuth } from './hooks/useAuth';
import { useTheme } from './hooks/useTheme';
import { createPerson, getGraphData, listInteractionsByPerson, listPersons } from './services/db';
import type { CreatePersonInput, GraphData, Interaction, Person } from './types';

const LEGACY_AUTH = import.meta.env.VITE_LEGACY_AUTH === 'true';

type Tab = 'home' | 'contacts' | 'graph' | 'query' | 'import';

const FOOTER_LINKS: { tab: Tab; label: string }[] = [
  { tab: 'contacts', label: '联系人' },
  { tab: 'graph', label: '图谱' },
  { tab: 'query', label: 'AI 查询(旧)' },
  { tab: 'import', label: '导入' },
];

function App() {
  const { theme, setTheme } = useTheme();
  const { isAuthenticated, user, loading: authLoading, login, register, logout } = useAuth();
  const [activeTab, setActiveTab] = useState<Tab>('home');
  const [persons, setPersons] = useState<Person[]>([]);
  const [selectedPerson, setSelectedPerson] = useState<Person | null>(null);
  const [interactionsByPerson, setInteractionsByPerson] = useState<Record<string, Interaction[]>>({});
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [detailPersonId, setDetailPersonId] = useState<string | null>(null);
  // 从联系人详情"关系网络"进入图谱页时的初始焦点，手动切 tab 时清除
  const [graphFocusId, setGraphFocusId] = useState<string | null>(null);
  const [error, setError] = useState('');
  // 新用户引导
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [dataLoaded, setDataLoaded] = useState(false);

  // 新认证模式下：未登录时显示AuthPage
  if (!LEGACY_AUTH) {
    if (authLoading) {
      return (
        <div className="flex min-h-screen items-center justify-center" style={{ backgroundColor: 'var(--bg-primary)', color: 'var(--text-secondary)' }}>
          正在检查登录状态...
        </div>
      );
    }
    if (!isAuthenticated) {
      return <AuthPage onLogin={login} onRegister={register} />;
    }
  }

  const personsById = useMemo(
    () => Object.fromEntries(persons.map((person) => [person.id, person])),
    [persons],
  );

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
      setGraphData(await getGraphData());
      // 首次加载完毕，检查是否需要触发新用户引导
      if (!dataLoaded) {
        setDataLoaded(true);
        if (list.length === 0 && !isOnboardingCompleted()) {
          setShowOnboarding(true);
        }
      }
    } catch (err) {
      setError(String(err));
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  // 互动记录按需加载：只拉取当前选中联系人，避免联系人多时逐个请求拖垮页面
  useEffect(() => {
    if (!selectedPerson) return;
    let cancelled = false;
    listInteractionsByPerson(selectedPerson.id)
      .then((list) => {
        if (!cancelled) {
          setInteractionsByPerson((prev) => ({ ...prev, [selectedPerson.id]: list }));
        }
      })
      .catch((err) => setError(String(err)));
    return () => {
      cancelled = true;
    };
  }, [selectedPerson?.id]);

  const handleCreatePerson = async (input: CreatePersonInput) => {
    const created = await createPerson(input);
    setSelectedPerson(created);
    await loadData();
  };

  const handleOpenDetail = (id: string) => {
    setDetailPersonId(id);
  };

  const handleNetworkView = (id: string) => {
    setDetailPersonId(null);
    setGraphFocusId(id);
    setActiveTab('graph');
  };

  const refreshAfterInteraction = async () => {
    await loadData();
    if (selectedPerson) {
      const list = await listInteractionsByPerson(selectedPerson.id);
      setInteractionsByPerson((prev) => ({ ...prev, [selectedPerson.id]: list }));
    }
  };

  const switchTab = (tab: Tab) => {
    setDetailPersonId(null);
    setGraphFocusId(null);
    setActiveTab(tab);
  };

  return (
    <div className="flex min-h-screen flex-col" style={{ backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}>
      {/* 新用户引导 */}
      {showOnboarding && (
        <OnboardingWizard
          onComplete={() => { setShowOnboarding(false); loadData(); }}
          onManualAdd={() => { setShowOnboarding(false); setActiveTab('home'); }}
          onQuerySubmit={(query) => {
            setShowOnboarding(false);
            setActiveTab('home');
            // 将查询填入首页 MultimodalQuery，使用自定义事件传递
            setTimeout(() => {
              window.dispatchEvent(new CustomEvent('rg_onboarding_query', { detail: query }));
            }, 100);
          }}
        />
      )}
      {/* 主题切换器 + 用户信息 - 固定右上角 */}
      <div className="fixed right-4 top-4 z-50 flex items-center gap-3">
        {!LEGACY_AUTH && user && (
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium" style={{ color: 'var(--text-secondary)' }}>
              {user.displayName || user.username}
            </span>
            <button
              type="button"
              onClick={logout}
              className="rounded-md px-2 py-1 text-xs transition hover:opacity-80"
              style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }}
            >
              退出
            </button>
          </div>
        )}
        <ThemeSelector theme={theme} setTheme={setTheme} />
      </div>

      {activeTab !== 'home' && (
        <header className="border-b px-6 py-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
          <div className="mx-auto flex max-w-7xl items-center justify-between">
            <div>
              <h1 className="text-2xl font-bold">个人关系图谱</h1>
              <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>本地优先、加密存储、端侧智能辅助</p>
            </div>
            <nav className="flex gap-2 pr-16">
              <TabButton active={false} onClick={() => switchTab('home')}>首页</TabButton>
              <TabButton active={activeTab === 'contacts'} onClick={() => switchTab('contacts')}>联系人</TabButton>
              <TabButton active={activeTab === 'graph'} onClick={() => switchTab('graph')}>图谱</TabButton>
              <TabButton active={activeTab === 'query'} onClick={() => switchTab('query')}>AI 查询(旧)</TabButton>
              <TabButton active={activeTab === 'import'} onClick={() => switchTab('import')}>导入</TabButton>
            </nav>
          </div>
        </header>
      )}

      <main className="mx-auto w-full max-w-7xl flex-1 p-6">
        {error && <div className="mb-4 rounded bg-red-50 p-3 text-sm text-red-700">{error}</div>}

        {activeTab === 'home' ? (
          <div className="flex min-h-[calc(100vh-10rem)] flex-col items-center justify-center px-4 text-center">
            <h1 className="text-4xl font-bold tracking-tight">个人关系图谱</h1>
            <p className="mt-3" style={{ color: 'var(--text-secondary)' }}>本地优先、加密存储，用一句话查询你的人脉网络</p>
            <div className="mt-8 w-full max-w-2xl">
              <MultimodalQuery onPersonClick={(personId) => {
                setDetailPersonId(personId);
                setActiveTab('contacts');
              }} />
            </div>
          </div>
        ) : detailPersonId ? (
          <PersonDetail
            personId={detailPersonId}
            personsById={personsById}
            onBack={() => setDetailPersonId(null)}
            onChanged={loadData}
            onOpenPerson={handleOpenDetail}
            onNetworkView={handleNetworkView}
          />
        ) : (
          <>
            {activeTab === 'contacts' && (
              <div className="grid grid-cols-1 gap-6 lg:grid-cols-[360px_1fr]">
                <aside className="space-y-4">
                  <PersonForm onSubmit={handleCreatePerson} />
                  <RelationshipForm persons={persons} onCreated={loadData} />
                  <InteractionForm person={selectedPerson} onCreated={refreshAfterInteraction} />
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
                    onSelect={(person) => {
                      setSelectedPerson(person);
                      handleOpenDetail(person.id);
                    }}
                  />
                  {selectedPerson && (
                    <div className="rounded-xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
                      <div className="flex items-center justify-between">
                        <h3 className="font-semibold">{selectedPerson.name} 的互动记录</h3>
                        <button
                          type="button"
                          className="text-sm hover:underline"
                          style={{ color: 'var(--accent-color)' }}
                          onClick={() => handleOpenDetail(selectedPerson.id)}
                        >
                          查看详情 →
                        </button>
                      </div>
                      <div className="mt-3 space-y-3">
                        {selectedInteractions.length === 0 ? (
                          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>暂无互动记录。</p>
                        ) : selectedInteractions.map((interaction) => (
                          <div key={interaction.id} className="rounded-lg p-3 text-sm" style={{ backgroundColor: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
                            <p className="font-medium" style={{ color: 'var(--text-primary)' }}>{new Date(interaction.timestamp).toLocaleString('zh-CN')}</p>
                            <p className="mt-1">{interaction.summary || interaction.content}</p>
                            <p className="mt-1" style={{ color: 'var(--text-muted)' }}>话题：{interaction.topics.join('、') || '无'}</p>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </section>
              </div>
            )}

            {activeTab === 'graph' && (
              <GraphView
                data={graphData}
                personsById={personsById}
                onNodeClick={handleOpenDetail}
                onRefresh={loadData}
                initialFocusId={graphFocusId ?? undefined}
              />
            )}
            {activeTab === 'query' && <NaturalLanguageQuery />}
            {activeTab === 'import' && <ImportWizard onImported={loadData} />}
          </>
        )}
      </main>

      <footer className="px-6 py-4">
        <div className="mx-auto flex max-w-7xl items-center justify-center text-xs" style={{ color: 'var(--text-muted)' }}>
          {FOOTER_LINKS.map((link, index) => (
            <span key={link.tab} className="flex items-center">
              {index > 0 && <span className="mx-2">·</span>}
              <button
                type="button"
                className="text-xs transition hover:opacity-80"
                style={{ color: 'var(--text-muted)' }}
                onClick={() => switchTab(link.tab)}
              >
                {link.label}
              </button>
            </span>
          ))}
        </div>
      </footer>
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-full px-4 py-2 text-sm font-medium transition"
      style={
        active
          ? { backgroundColor: 'var(--accent-color)', color: '#fff' }
          : { backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }
      }
    >
      {children}
    </button>
  );
}

export default App;
