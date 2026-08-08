import { useEffect, useMemo, useState } from 'react';
import { Navigate, Route, Routes, useLocation, useNavigate, useParams, useSearchParams } from 'react-router-dom';
import AdminPanel from './components/AdminPanel';
import ChatView from './components/ChatView';
import GraphView from './components/GraphView';
import ImportWizard from './components/ImportWizard';
import InteractionForm from './components/InteractionForm';
import PersonDetail from './components/PersonDetail';
import PersonForm from './components/PersonForm';
import PersonList from './components/PersonList';
import ProfileQA from './components/ProfileQA';
import RelationshipForm from './components/RelationshipForm';
import ThemeSelector from './components/ThemeSelector';
import { useAuth } from './hooks/useAuth';
import { useTheme } from './hooks/useTheme';
import { createPerson, getGraphData, listInteractionsByPerson, listPersons } from './services/db';
import type { CreatePersonInput, GraphData, Interaction, Person } from './types';

function App() {
  const { theme, setTheme } = useTheme();
  const { user, isAdmin, logout, refreshUser } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();

  // 新用户首次进入时自动引导到内观画像问卷；完成/跳过后本轮不再强制
  const [profilePrompted, setProfilePrompted] = useState(false);

  useEffect(() => {
    if (user && !user.profileCompleted && !profilePrompted) {
      setProfilePrompted(true);
      navigate('/profile-qa', { replace: true });
    }
  }, [user, profilePrompted, navigate]);
  const [persons, setPersons] = useState<Person[]>([]);
  const [selectedPerson, setSelectedPerson] = useState<Person | null>(null);
  const [interactionsByPerson, setInteractionsByPerson] = useState<Record<string, Interaction[]>>({});
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [error, setError] = useState('');

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
    navigate(`/contacts/${id}`);
  };

  const handleNetworkView = (id: string) => {
    navigate(`/graph?focus=${id}`);
  };

  const refreshAfterInteraction = async () => {
    await loadData();
    if (selectedPerson) {
      const list = await listInteractionsByPerson(selectedPerson.id);
      setInteractionsByPerson((prev) => ({ ...prev, [selectedPerson.id]: list }));
    }
  };

  // 导航栏 active 判断：路径匹配
  const isHomeActive = location.pathname === '/';
  const isPathActive = (path: string) => location.pathname.startsWith(path);

  return (
    <div className="flex h-screen flex-col" style={{ backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}>
      {/* 顶部导航栏 */}
      <header
        className="flex shrink-0 items-center justify-between border-b px-4 py-2"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
      >
        <div className="flex items-center gap-4">
          <h1 className="text-base font-bold whitespace-nowrap">Personal AI Platform</h1>
          <nav className="flex gap-1">
            <TabButton active={isHomeActive} onClick={() => navigate('/')}>首页</TabButton>
            <TabButton active={isPathActive('/profile-qa')} onClick={() => navigate('/profile-qa')}>内观画像</TabButton>
            <TabButton active={isPathActive('/contacts')} onClick={() => navigate('/contacts')}>联系人</TabButton>
            <TabButton active={isPathActive('/graph')} onClick={() => navigate('/graph')}>图谱</TabButton>
            <TabButton active={isPathActive('/import')} onClick={() => navigate('/import')}>导入</TabButton>
            {isAdmin && (
              <TabButton active={isPathActive('/admin')} onClick={() => navigate('/admin')}>管理后台</TabButton>
            )}
          </nav>
        </div>

        <div className="flex items-center gap-3">
          {user && (
            <span className="text-sm" style={{ color: 'var(--text-secondary)' }}>
              {user.displayName || user.username}
            </span>
          )}
          <button
            type="button"
            className="text-sm transition hover:opacity-80"
            style={{ color: 'var(--text-muted)' }}
            onClick={logout}
          >
            注销
          </button>
          <ThemeSelector theme={theme} setTheme={setTheme} />
        </div>
      </header>

      {/* 主体内容 */}
      <main className="flex-1 overflow-hidden">
        {error && (
          <div className="mx-4 mt-3 rounded bg-red-50 p-2 text-sm text-red-700">{error}</div>
        )}

        <Routes>
          <Route path="/" element={<ChatView onPersonClick={handleOpenDetail} userId={user?.id} />} />

          <Route path="/contacts" element={
            <div className="mx-auto h-full max-w-7xl overflow-y-auto p-6">
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
            </div>
          } />

          <Route path="/contacts/:personId" element={
            <ContactDetailPage
              personsById={personsById}
              loadData={loadData}
              handleOpenDetail={handleOpenDetail}
              handleNetworkView={handleNetworkView}
            />
          } />

          <Route path="/graph" element={
            <GraphPage
              graphData={graphData}
              personsById={personsById}
              handleOpenDetail={handleOpenDetail}
              loadData={loadData}
            />
          } />

          <Route path="/import" element={
            <div className="mx-auto h-full max-w-7xl overflow-y-auto p-6">
              <ImportWizard onImported={loadData} />
            </div>
          } />

          <Route path="/admin" element={
            isAdmin ? (
              <div className="h-full overflow-y-auto">
                <AdminPanel userId={user?.id} />
              </div>
            ) : (
              <Navigate to="/" replace />
            )
          } />

          <Route path="/profile-qa" element={
            <div className="h-full overflow-y-auto">
              <ProfileQA
                initialProfileDoc={user?.profileDoc ?? null}
                initialCompleted={user?.profileCompleted ?? false}
                onComplete={() => {
                  // 完成后刷新用户信息，使 profileCompleted 状态生效
                  void refreshUser();
                  navigate('/', { replace: true });
                }}
              />
            </div>
          } />

          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
    </div>
  );
}

// 联系人详情包装组件（读取 URL 参数）
function ContactDetailPage({ personsById, loadData, handleOpenDetail, handleNetworkView }: {
  personsById: Record<string, Person>;
  loadData: () => Promise<void>;
  handleOpenDetail: (id: string) => void;
  handleNetworkView: (id: string) => void;
}) {
  const { personId } = useParams();
  const navigate = useNavigate();
  return (
    <div className="mx-auto h-full max-w-7xl overflow-y-auto p-6">
      <PersonDetail
        personId={personId!}
        personsById={personsById}
        onBack={() => navigate('/contacts')}
        onChanged={loadData}
        onOpenPerson={handleOpenDetail}
        onNetworkView={handleNetworkView}
      />
    </div>
  );
}

// 图谱包装组件（读取 ?focus 查询参数）
function GraphPage({ graphData, personsById, handleOpenDetail, loadData }: {
  graphData: GraphData;
  personsById: Record<string, Person>;
  handleOpenDetail: (id: string) => void;
  loadData: () => Promise<void>;
}) {
  const [searchParams] = useSearchParams();
  const focusId = searchParams.get('focus') ?? undefined;
  return (
    <div className="mx-auto h-full max-w-7xl p-6">
      <GraphView
        data={graphData}
        personsById={personsById}
        onNodeClick={handleOpenDetail}
        onRefresh={loadData}
        initialFocusId={focusId}
      />
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-full px-3 py-1.5 text-sm font-medium transition"
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
