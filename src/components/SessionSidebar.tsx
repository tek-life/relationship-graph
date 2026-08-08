/**
 * 会话侧边栏组件
 * 从左侧滑出的抽屉式面板，展示会话列表，支持新建、搜索、重命名、删除
 */
import { useState, useMemo } from 'react';
import { Search, X } from 'lucide-react';
import type { Session } from '../types';

interface SessionSidebarProps {
  sessions: Session[];
  currentSessionId: string | null;
  onSelectSession: (sessionId: string) => void;
  onNewSession: () => void;
  onRenameSession: (sessionId: string, title: string) => Promise<void>;
  onDeleteSession: (sessionId: string) => void;
  isOpen: boolean;
  onClose: () => void;
}

export default function SessionSidebar({
  sessions,
  currentSessionId,
  onSelectSession,
  onNewSession,
  onRenameSession,
  onDeleteSession,
  isOpen,
  onClose,
}: SessionSidebarProps) {
  const [search, setSearch] = useState('');
  /** 正在内联重命名的会话 ID */
  const [editingId, setEditingId] = useState<string | null>(null);
  /** 重命名输入草稿 */
  const [draft, setDraft] = useState('');
  /** 防止 blur 与 Enter 重复提交 */
  const [committing, setCommitting] = useState(false);

  const startRename = (session: Session) => {
    setEditingId(session.id);
    setDraft(session.title || '');
  };

  const cancelRename = () => {
    setEditingId(null);
    setDraft('');
    setCommitting(false);
  };

  const commitRename = async () => {
    if (!editingId || committing) return;
    const title = draft.trim();
    setCommitting(true);
    try {
      if (title) {
        await onRenameSession(editingId, title);
      }
    } catch {
      window.alert('重命名失败，请重试');
    } finally {
      cancelRename();
    }
  };

  const filteredSessions = useMemo(
    () =>
      sessions
        .filter((s) => {
          if (!search.trim()) return true;
          const title = s.title || '新会话';
          return title.toLowerCase().includes(search.toLowerCase());
        })
        .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()),
    [sessions, search],
  );

  const formatTime = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return '刚刚';
    if (diffMins < 60) return `${diffMins} 分钟前`;
    if (diffHours < 24) return `${diffHours} 小时前`;
    if (diffDays < 7) return `${diffDays} 天前`;
    return date.toLocaleDateString('zh-CN');
  };

  return (
    <>
      {/* 遮罩层 */}
      {isOpen && (
        <div
          className="fixed inset-0 bg-black/30 z-40 transition-opacity"
          onClick={onClose}
        />
      )}

      {/* 侧边栏 */}
      <div
        className={`fixed left-0 top-0 h-full w-72 shadow-xl z-50 transform transition-transform duration-300 ease-out
          ${isOpen ? 'translate-x-0' : '-translate-x-full'}`}
        style={{ backgroundColor: 'var(--bg-card)', borderRight: '1px solid var(--border-color)' }}
      >
        <div className="flex flex-col h-full p-4">
          {/* 头部 */}
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold" style={{ color: 'var(--text-primary)' }}>
              会话历史
            </h2>
            <button
              type="button"
              onClick={onClose}
              className="w-8 h-8 rounded-full flex items-center justify-center transition hover:bg-surface"
              style={{ color: 'var(--text-secondary)' }}
            >
              <X size={16} aria-hidden="true" />
            </button>
          </div>

          {/* 新建会话按钮 */}
          <button
            type="button"
            onClick={() => {
              onNewSession();
              onClose();
            }}
            className="w-full rounded-lg border-2 border-dashed py-2.5 text-sm font-medium 
                       hover:opacity-80 transition mb-3"
            style={{ borderColor: 'var(--accent-color)', color: 'var(--accent-color)' }}
          >
            + 新建会话
          </button>

          {/* 搜索框 */}
          <div className="relative mb-3">
            <input
              type="text"
              placeholder="搜索会话..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full rounded-lg border px-3 py-2 pl-8 text-sm outline-none
                       focus:ring-2 focus:ring-accent-light transition"
              style={{
                borderColor: 'var(--border-color)',
                backgroundColor: 'var(--bg-primary)',
                color: 'var(--text-primary)',
              }}
            />
            <Search
              size={14}
              className="absolute left-2.5 top-1/2 -translate-y-1/2"
              style={{ color: 'var(--text-tertiary, #aaa)' }}
              aria-hidden="true"
            />
          </div>

          {/* 会话列表 */}
          <div className="flex-1 overflow-y-auto space-y-1 -mx-1">
            {filteredSessions.length === 0 ? (
              <div className="text-center py-8">
                <p className="text-sm" style={{ color: 'var(--text-tertiary, #aaa)' }}>
                  {search ? '没有匹配的会话' : '暂无会话，点击上方按钮新建'}
                </p>
              </div>
            ) : (
              filteredSessions.map((session) => (
                <div
                  key={session.id}
                  className={`group flex items-center rounded-lg px-3 py-2.5 cursor-pointer transition-all duration-150
                    ${
                      session.id === currentSessionId
                        ? 'bg-accent-light border-l-2 border-accent'
                        : 'hover:bg-surface border-l-2 border-transparent'
                    }`}
                  onClick={() => {
                    if (editingId === session.id) return;
                    onSelectSession(session.id);
                    onClose();
                  }}
                >
                  <div className="flex-1 min-w-0">
                    {editingId === session.id ? (
                      <input
                        type="text"
                        autoFocus
                        value={draft}
                        onChange={(e) => setDraft(e.target.value)}
                        onClick={(e) => e.stopPropagation()}
                        onBlur={() => void commitRename()}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault();
                            void commitRename();
                          } else if (e.key === 'Escape') {
                            cancelRename();
                          }
                        }}
                        className="w-full rounded border px-2 py-1 text-sm outline-none
                                 focus:ring-2 focus:ring-accent-light"
                        style={{
                          borderColor: 'var(--border-color)',
                          backgroundColor: 'var(--bg-primary)',
                          color: 'var(--text-primary)',
                        }}
                      />
                    ) : (
                      <p
                        className={`text-sm font-medium truncate ${
                          session.id === currentSessionId
                            ? 'text-accent'
                            : ''
                        }`}
                        style={
                          session.id !== currentSessionId
                            ? { color: 'var(--text-primary)' }
                            : undefined
                        }
                      >
                        {session.title || '新会话'}
                      </p>
                    )}
                    <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted, #999)' }}>
                      {formatTime(session.updatedAt)}
                    </p>
                  </div>
                  {editingId !== session.id && (
                    <>
                      <button
                        type="button"
                        title="重命名"
                        onClick={(e) => {
                          e.stopPropagation();
                          startRename(session);
                        }}
                        className="opacity-0 group-hover:opacity-100 hover:text-accent
                                 text-xs ml-2 px-1.5 py-0.5 rounded transition-all shrink-0"
                        style={{ color: 'var(--text-secondary)' }}
                      >
                        重命名
                      </button>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteSession(session.id);
                        }}
                        className="opacity-0 group-hover:opacity-100 text-danger hover:text-danger-hover 
                                 text-xs ml-1 px-1.5 py-0.5 rounded transition-all shrink-0"
                      >
                        删除
                      </button>
                    </>
                  )}
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </>
  );
}
