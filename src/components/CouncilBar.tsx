/**
 * 数字人头像栏组件
 * 水平排列所有激活数字人的头像 + 名字标签，点击即插入 @提及并选中该数字人
 * hover 显示 tooltip（数字人名称 + 功能描述）
 * 渲染在输入框容器内部，与输入框左右边缘对齐
 */
import { useEffect, useState } from 'react';
import { fetchDigitalAgents, type DigitalAgent } from '../services/digitalAgents';

interface CouncilBarProps {
  selectedAgentIds: string[];
  onPickAgent: (agent: DigitalAgent) => void;
}

export default function CouncilBar({ selectedAgentIds, onPickAgent }: CouncilBarProps) {
  const [agents, setAgents] = useState<DigitalAgent[]>([]);

  useEffect(() => {
    fetchDigitalAgents().then(setAgents);
  }, []);

  const activeAgents = agents.filter((a) => a.isActive);

  if (activeAgents.length === 0) return null;

  return (
    <div className="flex items-center gap-3 pb-2">
      <span className="text-xs shrink-0" style={{ color: 'var(--text-secondary)' }}>
        幕僚团：
      </span>
      {activeAgents.map((agent) => {
        const isSelected = selectedAgentIds.includes(agent.id);
        return (
          <div key={agent.id} className="relative group">
            <button
              type="button"
              onClick={() => onPickAgent(agent)}
              aria-label={`${agent.displayName}${isSelected ? '（已选中）' : ''}`}
              className="flex items-center gap-1.5 rounded-full transition-all"
            >
              <span
                className={`block w-10 h-10 rounded-full overflow-hidden border-2 transition-all
                  ${
                    isSelected
                      ? 'border-accent shadow-lg scale-110'
                      : 'border-transparent hover:border-line hover:scale-105'
                  }`}
              >
                {agent.avatar ? (
                  <img
                    src={agent.avatar}
                    alt={agent.displayName}
                    className="w-full h-full object-cover"
                  />
                ) : (
                  <span className="w-full h-full bg-accent-light flex items-center justify-center text-accent font-bold text-sm">
                    {agent.displayName[0]}
                  </span>
                )}
              </span>
              {/* 名字标签：小字体，选中态高亮 */}
              <span
                className="text-xs whitespace-nowrap"
                style={{ color: isSelected ? 'var(--accent-color)' : 'var(--text-secondary)' }}
              >
                {agent.displayName}
              </span>
            </button>
            {/* Tooltip */}
            <div
              className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 
                          bg-text-primary text-bg-primary text-xs rounded-lg opacity-0 group-hover:opacity-100 
                          transition-opacity pointer-events-none whitespace-nowrap z-50"
            >
              <div className="font-semibold">{agent.displayName}</div>
              {agent.description && (
                <div className="mt-1 opacity-80">{agent.description}</div>
              )}
              {/* 小三角 */}
              <div className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-text-primary" />
            </div>
          </div>
        );
      })}
    </div>
  );
}
