// UX P1-8：联系人页（从 App.tsx 内联路由抽离）。
// 重构要点：
// 1. 左栏常驻堆叠的 PersonForm / RelationshipForm / InteractionForm 三表单收敛为
//    工具栏「新建联系人」主按钮 + 右侧抽屉；关系 / 互动表单移入 PersonDetail 详情页。
// 2. 列表交互明确化：点击卡片 = 直接跳转详情页（单一动作），不再有「选中」中间态，
//    页底与详情页重复的互动记录面板随之移除。
// 3. 保留 P0-5 的「导入」工具栏按钮（跳 /import）。

import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Upload, UserPlus } from 'lucide-react';
import PersonForm from '../PersonForm';
import PersonList from '../PersonList';
import { PersonListSkeleton } from '../ui';
import { createPerson } from '../../services/db';
import type { CreatePersonInput, Person } from '../../types';
import { Drawer } from './Drawer';

interface Props {
  /** 联系人列表（由 App 全局加载，图谱页 / 导入刷新共用同一份数据） */
  persons: Person[];
  /** UX P2-12：首次加载中时列表区展示骨架屏（仅在列表为空时生效） */
  loading?: boolean;
  /** 新建 / 导入后刷新全局数据 */
  onRefresh: () => Promise<void> | void;
}

export default function ContactsPage({ persons, loading = false, onRefresh }: Props) {
  const navigate = useNavigate();
  const [createOpen, setCreateOpen] = useState(false);
  const [error, setError] = useState('');

  const handleCreate = async (input: CreatePersonInput) => {
    try {
      const created = await createPerson(input);
      setError('');
      setCreateOpen(false);
      await onRefresh();
      // 新建成功后直接进入该联系人详情页，衔接后续关系 / 互动录入
      navigate(`/contacts/${created.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="mx-auto h-full max-w-7xl overflow-y-auto p-6">
      {/* 工具栏：标题 + 计数 | 导入 + 新建联系人 */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-baseline gap-3">
          <h2 className="text-title font-semibold">联系人名片</h2>
          <span className="text-body text-text-secondary">共 {persons.length} 人</span>
        </div>
        <div className="flex items-center gap-3">
          {/* UX P0-5：「导入」从顶栏降级为联系人页工具栏按钮 */}
          <button
            type="button"
            className="inline-flex items-center gap-1.5 rounded-control border border-line bg-card px-3 py-1.5 text-body font-medium text-text-primary transition-colors hover:bg-surface"
            onClick={() => navigate('/import')}
          >
            <Upload size={14} aria-hidden="true" />
            导入
          </button>
          {/* UX P1-8：新建联系人主按钮（抽屉化） */}
          <button
            type="button"
            className="inline-flex items-center gap-1.5 rounded-control bg-accent px-3.5 py-1.5 text-body font-medium text-white transition-colors hover:bg-accent-hover"
            onClick={() => setCreateOpen(true)}
          >
            <UserPlus size={14} aria-hidden="true" />
            新建联系人
          </button>
        </div>
      </div>

      {error && (
        <div className="mt-3 rounded-control bg-danger-light p-2 text-body text-danger">{error}</div>
      )}

      {/* 列表：点击卡片直接跳详情页（交互单一化，无选中态）；首次加载展示骨架屏 */}
      <div className="mt-4">
        {loading && persons.length === 0 ? (
          <PersonListSkeleton />
        ) : (
          <PersonList persons={persons} onSelect={(person) => navigate(`/contacts/${person.id}`)} />
        )}
      </div>

      {/* 新建联系人抽屉 */}
      <Drawer open={createOpen} title="新建联系人" onClose={() => setCreateOpen(false)}>
        <PersonForm onSubmit={handleCreate} />
      </Drawer>
    </div>
  );
}
