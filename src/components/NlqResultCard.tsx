import type { NlqResult } from '../types';
import SensitivityGuard from './SensitivityGuard';

export function strengthText(value?: string | null) {
  if (value === 'strong') return '强';
  if (value === 'weak') return '弱';
  return '中';
}

export function statusText(value: string) {
  if (value === 'follow-up') return '待跟进';
  if (value === 'cold') return '冷却';
  return '活跃';
}

interface NlqResultCardProps {
  result: NlqResult;
  onPersonClick?: (personId: string) => void;
}

export default function NlqResultCard({ result, onPersonClick }: NlqResultCardProps) {
  const clickable = !!onPersonClick;

  const handleClick = () => {
    if (onPersonClick) {
      onPersonClick(result.personId);
    }
  };

  const content = (
    <div className="space-y-1">
      <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>{result.displayName}</h3>
      <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>{[result.company, result.title].filter(Boolean).join(' / ') || '未填写公司职位'}</p>
      <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>关系强度：{strengthText(result.relationshipStrength)}｜状态：{statusText(result.status)}</p>
      <p className="text-sm" style={{ color: 'var(--text-primary)' }}>上次互动：{result.lastInteractionSummary || '暂无摘要'}</p>
      <p className="text-sm" style={{ color: 'var(--accent-color)' }}>建议下一步：{result.suggestion}</p>
    </div>
  );

  return (
    <div
      className={`rounded-lg border p-4 transition${clickable ? ' cursor-pointer active:opacity-80' : ''}`}
      style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}
      onClick={handleClick}
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      onKeyDown={clickable ? (e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(); } : undefined}
    >
      {result.realNameHidden ? (
        <SensitivityGuard level={result.sensitivityLevel} fallback={<span>存在高敏感联系人，默认已脱敏。</span>}>
          {content}
        </SensitivityGuard>
      ) : content}
    </div>
  );
}
