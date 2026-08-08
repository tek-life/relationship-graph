/**
 * NLQ 搜索结果名片卡
 * 结构化排版：头像 + 姓名/公司职位两级字阶 + 强度/状态/敏感级 Badge chips，
 * 底部浅底建议条；高敏感联系人经 SensitivityGuard 二次确认后展示。
 */
import { ChevronRight } from 'lucide-react';
import type { NlqResult, PersonStatus, RelationshipStrength, SensitivityLevel } from '../types';
import { Badge, type BadgeVariant } from './ui';
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

export function sensitivityText(level: SensitivityLevel) {
  if (level === 'high') return '高敏感';
  if (level === 'medium') return '中敏感';
  return '低敏感';
}

/** 关系强度 → Badge 变体 */
export function strengthVariant(strength?: RelationshipStrength | null): BadgeVariant {
  if (strength === 'strong') return 'info';
  return 'default';
}

/** 状态 → Badge 变体（待跟进=警示，活跃=正向，冷却=中性） */
export function statusVariant(status: PersonStatus): BadgeVariant {
  if (status === 'follow-up') return 'warning';
  if (status === 'active') return 'success';
  return 'default';
}

/** 敏感级 → Badge 变体 */
export function sensitivityVariant(level: SensitivityLevel): BadgeVariant {
  if (level === 'high') return 'danger';
  if (level === 'medium') return 'warning';
  return 'default';
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

  const subtitle = [result.company, result.title].filter(Boolean).join(' · ');

  const main = (
    <div className="flex items-start gap-3 p-4">
      {/* 头像（姓名首字） */}
      <div
        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-base font-semibold"
        style={{ backgroundColor: 'var(--accent-light)', color: 'var(--accent-color)' }}
        aria-hidden="true"
      >
        {result.displayName.slice(0, 1)}
      </div>

      <div className="min-w-0 flex-1">
        {/* 姓名 + 语义 chips */}
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          <h3 className="text-base font-semibold leading-6" style={{ color: 'var(--text-primary)' }}>
            {result.displayName}
          </h3>
          <Badge variant={strengthVariant(result.relationshipStrength)}>
            关系{strengthText(result.relationshipStrength)}
          </Badge>
          <Badge variant={statusVariant(result.status)}>{statusText(result.status)}</Badge>
          <Badge variant={sensitivityVariant(result.sensitivityLevel)}>
            {sensitivityText(result.sensitivityLevel)}
          </Badge>
        </div>

        {/* 公司 · 职位（次级字阶） */}
        <p className="mt-1 truncate text-sm" style={{ color: 'var(--text-secondary)' }}>
          {subtitle || '未填写公司职位'}
        </p>

        {/* 上次互动摘要 */}
        <p className="mt-1 truncate text-xs" style={{ color: 'var(--text-muted)' }}>
          上次互动：{result.lastInteractionSummary || '暂无摘要'}
        </p>
      </div>

      {clickable && (
        <ChevronRight
          size={16}
          className="mt-1 shrink-0"
          style={{ color: 'var(--text-muted)' }}
          aria-hidden="true"
        />
      )}
    </div>
  );

  return (
    <div
      className={`overflow-hidden rounded-xl border shadow-sm transition${
        clickable ? ' cursor-pointer hover:shadow-md active:opacity-80' : ''
      }`}
      style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}
      onClick={handleClick}
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      onKeyDown={clickable ? (e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(); } : undefined}
    >
      {result.realNameHidden ? (
        <div className="p-4">
          <SensitivityGuard level={result.sensitivityLevel} fallback={<span>存在高敏感联系人，默认已脱敏。</span>}>
            {main}
          </SensitivityGuard>
        </div>
      ) : (
        main
      )}

      {/* 浅底建议条 */}
      <div
        className="border-t px-4 py-2 text-xs"
        style={{
          borderColor: 'var(--border-color)',
          backgroundColor: 'var(--bg-secondary)',
          color: 'var(--text-secondary)',
        }}
      >
        建议下一步：{result.suggestion}
      </div>
    </div>
  );
}
