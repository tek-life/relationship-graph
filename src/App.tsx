import { useEffect, useMemo, useState } from 'react';
import { Navigate, Route, Routes, useLocation, useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import AdminPanel from './components/AdminPanel';
import ChatView from './components/ChatView';
import ContactsPage from './components/contacts/ContactsPage';
import GraphView from './components/GraphView';
import ImportWizard from './components/ImportWizard';
import PersonDetail from './components/PersonDetail';
import ProfileQA from './components/ProfileQA';
import ThemeSelector from './components/ThemeSelector';
import UserMenu from './components/UserMenu';
import { useAuth } from './hooks/useAuth';
import { useTheme } from './hooks/useTheme';
import { getGraphData, listPersons } from './services/db';
import { MAIN_NAV_ITEMS, isNavPathActive } from './services/mainNav';
import type { GraphData, Person } from './types';

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
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [error, setError] = useState('');
  // UX P2-12：首次加载中标记，供联系人列表展示骨架屏（仅在列表为空时生效）
  const [personsLoading, setPersonsLoading] = useState(true);

  const personsById = useMemo(
    () => Object.fromEntries(persons.map((person) => [person.id, person])),
    [persons],
  );

  const loadData = async () => {
    try {
      setPersons(await listPersons());
      setGraphData(await getGraphData());
    } catch (err) {
      setError(String(err));
    } finally {
      setPersonsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleOpenDetail = (id: string) => {
    navigate(`/contacts/${id}`);
  };

  const handleNetworkView = (id: string) => {
    navigate(`/graph?focus=${id}`);
  };

  return (
    <div className="flex h-screen flex-col bg-primary text-text-primary">
      {/* 顶部导航栏（UX P0-5：主导航收敛为 AI 助理 / 联系人 / 图谱，tabs 下划线化） */}
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-line bg-card px-4">
        <div className="flex h-full items-center gap-6">
          {/* UX P2-11 品牌字标：logo mark（关系网络节点图形，取 accent 令牌）+ 加粗英文字标 */}
          <div className="flex items-center gap-2 whitespace-nowrap" aria-label="Personal AI Platform">
            <span
              aria-hidden="true"
              className="flex h-7 w-7 items-center justify-center rounded-lg border border-line bg-accent-light"
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path
                  d="M4.5 11.5 8 4.5l3.5 7M4.5 11.5h7"
                  stroke="var(--accent-color)"
                  strokeWidth="1.3"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <circle cx="8" cy="4.5" r="1.8" fill="var(--accent-color)" />
                <circle cx="4.5" cy="11.5" r="1.8" fill="var(--accent-color)" />
                <circle cx="11.5" cy="11.5" r="1.8" fill="var(--accent-color)" />
              </svg>
            </span>
            <h1 className="text-lead font-bold tracking-tight">Personal AI Platform</h1>
          </div>
          <nav className="flex h-full items-stretch gap-1" aria-label="主导航">
            {MAIN_NAV_ITEMS.map((item) => (
              <TabButton
                key={item.id}
                active={isNavPathActive(location.pathname, item.path)}
                onClick={() => navigate(item.path)}
              >
                {item.label}
              </TabButton>
            ))}
          </nav>
        </div>

        <div className="flex items-center gap-3">
          <ThemeSelector theme={theme} setTheme={setTheme} />
          {user && (
            <UserMenu
              displayName={user.displayName || user.username}
              isAdmin={isAdmin}
              onProfile={() => navigate('/profile-qa')}
              onAdmin={() => navigate('/admin')}
              onLogout={logout}
            />
          )}
        </div>
      </header>

      {/* 主体内容 */}
      <main className="flex-1 overflow-hidden">
        {error && (
          <div className="mx-4 mt-3 rounded bg-danger-light p-2 text-sm text-danger">{error}</div>
        )}

        <Routes>
          <Route path="/" element={<ChatView onPersonClick={handleOpenDetail} userId={user?.id} />} />

          {/* UX P1-8：联系人页抽离为 ContactsPage（表单抽屉化 + 列表点击直达详情） */}
          <Route path="/contacts" element={<ContactsPage persons={persons} loading={personsLoading} onRefresh={loadData} />} />

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
              <button
                type="button"
                className="mb-4 inline-flex items-center gap-1.5 rounded-control border border-line bg-card px-3 py-1.5 text-body text-text-secondary transition-colors hover:bg-surface hover:text-text-primary"
                onClick={() => navigate('/contacts')}
              >
                <ArrowLeft size={14} aria-hidden="true" />
                返回联系人
              </button>
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

// UX P0-5：下划线指示器式 tab（替代原 pill 实心样式），激活态以 accent 下划线标示
function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? 'page' : undefined}
      className={`relative flex items-center px-3 text-body font-medium transition-colors ${
        active ? 'text-text-primary' : 'text-text-secondary hover:text-text-primary'
      }`}
    >
      {children}
      <span
        aria-hidden="true"
        className={`absolute inset-x-2 bottom-0 h-0.5 rounded-full transition-colors ${
          active ? 'bg-accent' : 'bg-transparent'
        }`}
      />
    </button>
  );
}

export default App;
