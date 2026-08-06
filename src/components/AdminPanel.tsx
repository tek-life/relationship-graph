// 管理后台主面板：Tab 式管理界面
// 包含数字人管理、QA 指令管理、用户管理、邀请管理四个模块

import { useState } from 'react';
import AgentManager from './admin/AgentManager';
import InviteManager from './admin/InviteManager';
import QaModuleManager from './admin/QaModuleManager';
import UserManager from './admin/UserManager';
import { AdminTabButton } from './admin/shared';

type AdminTab = 'agents' | 'qa-modules' | 'users' | 'invites';

interface AdminPanelProps {
  /** 保留以兼容 App.tsx 传参，当前未使用 */
  userId?: string;
}

export default function AdminPanel({ userId: _userId }: AdminPanelProps = {}) {
  const [activeTab, setActiveTab] = useState<AdminTab>('agents');

  return (
    <div className="mx-auto max-w-5xl p-6">
      <h2 className="mb-6 text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>
        管理后台
      </h2>

      {/* Tab 导航 */}
      <div
        className="mb-6 flex gap-2 border-b pb-2"
        style={{ borderColor: 'var(--border-color)' }}
      >
        <AdminTabButton active={activeTab === 'agents'} onClick={() => setActiveTab('agents')}>
          数字人管理
        </AdminTabButton>
        <AdminTabButton
          active={activeTab === 'qa-modules'}
          onClick={() => setActiveTab('qa-modules')}
        >
          QA 指令管理
        </AdminTabButton>
        <AdminTabButton active={activeTab === 'users'} onClick={() => setActiveTab('users')}>
          用户管理
        </AdminTabButton>
        <AdminTabButton active={activeTab === 'invites'} onClick={() => setActiveTab('invites')}>
          邀请管理
        </AdminTabButton>
      </div>

      {/* 内容区域 */}
      {activeTab === 'agents' && <AgentManager />}
      {activeTab === 'qa-modules' && <QaModuleManager />}
      {activeTab === 'users' && <UserManager />}
      {activeTab === 'invites' && <InviteManager />}
    </div>
  );
}
