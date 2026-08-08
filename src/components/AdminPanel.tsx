// 管理后台主面板：sidebar IA（UX P1-10）
// 左侧分组导航（智能配置 / 系统管理）+ 右侧内容区；
// 各子页统一使用 AdminPageHeader 页头范式（标题 + 描述 + 主操作区）。

import { useState } from 'react';
import { Bot, Brain, Package, Settings, Ticket, Users } from 'lucide-react';
import AgentManager from './admin/AgentManager';
import InviteManager from './admin/InviteManager';
import QaModuleManager from './admin/QaModuleManager';
import SkillPackageManager from './admin/SkillPackageManager';
import SystemConfigManager from './admin/SystemConfigManager';
import UserManager from './admin/UserManager';
import { AdminSideNav } from './admin/shared';
import type { AdminNavGroup } from './admin/shared';

type AdminTab = 'agents' | 'skill-packages' | 'qa-modules' | 'users' | 'invites' | 'settings';

/** sidebar 分组结构：按功能域分为「智能配置」与「系统管理」两组 */
const ADMIN_NAV_GROUPS: AdminNavGroup[] = [
  {
    title: '智能配置',
    items: [
      { key: 'agents', label: '数字人管理', icon: Bot },
      { key: 'skill-packages', label: '技能包', icon: Package },
      { key: 'qa-modules', label: '内观画像指令', icon: Brain },
    ],
  },
  {
    title: '系统管理',
    items: [
      { key: 'users', label: '用户管理', icon: Users },
      { key: 'invites', label: '邀请管理', icon: Ticket },
      { key: 'settings', label: '系统设置', icon: Settings },
    ],
  },
];

interface AdminPanelProps {
  /** 保留以兼容 App.tsx 传参，当前未使用 */
  userId?: string;
}

export default function AdminPanel({ userId: _userId }: AdminPanelProps = {}) {
  const [activeTab, setActiveTab] = useState<AdminTab>('agents');

  return (
    <div className="mx-auto flex max-w-6xl items-start gap-8 p-6">
      <AdminSideNav
        groups={ADMIN_NAV_GROUPS}
        active={activeTab}
        onSelect={(key) => setActiveTab(key as AdminTab)}
      />

      {/* 内容区域 */}
      <main className="min-w-0 flex-1">
        {activeTab === 'agents' && <AgentManager />}
        {activeTab === 'skill-packages' && <SkillPackageManager />}
        {activeTab === 'qa-modules' && <QaModuleManager />}
        {activeTab === 'users' && <UserManager />}
        {activeTab === 'invites' && <InviteManager />}
        {activeTab === 'settings' && <SystemConfigManager />}
      </main>
    </div>
  );
}
