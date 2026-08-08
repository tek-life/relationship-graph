// UX P1-9：图谱节点单击浮层。
// 单击节点后在节点下方弹出，提供「查看详情 / 设为焦点」快捷操作，
// 替代原先 280ms 定时器区分单击/双击的交互。
// 颜色全部走设计令牌（Tailwind 主题类），不含硬编码色值。

import { Crosshair, Eye, X } from 'lucide-react';
import type { Person } from '../types';
import { IconBtn } from './ui';

export interface NodePopoverState {
  person: Person;
  /** 节点在画布容器内的渲染坐标（cytoscape renderedPosition） */
  x: number;
  y: number;
  /** 节点渲染直径，用于把浮层摆到节点正下方 */
  size: number;
}

export const NODE_POPOVER_WIDTH = 260;
export const NODE_POPOVER_HEIGHT = 168;

export interface PopoverPlacement {
  left: number;
  top: number;
  /** true = 空间不足，浮层摆在节点上方 */
  above: boolean;
}

/**
 * 浮层定位纯函数：水平居中对齐节点并钳制在容器内，
 * 垂直方向优先摆在节点下方，放不下则翻转到节点上方。
 */
export function clampNodePopoverPosition(
  x: number,
  y: number,
  nodeSize: number,
  containerWidth: number,
  containerHeight: number,
  popWidth = NODE_POPOVER_WIDTH,
  popHeight = NODE_POPOVER_HEIGHT,
): PopoverPlacement {
  const margin = 8;
  const gap = 10;
  const half = popWidth / 2;
  const left = Math.round(
    Math.max(margin, Math.min(x - half, Math.max(margin, containerWidth - popWidth - margin))),
  );
  const below = y + nodeSize / 2 + gap;
  const aboveTop = y - nodeSize / 2 - gap - popHeight;
  const above = below + popHeight > containerHeight - margin && aboveTop >= margin;
  const top = Math.round(Math.max(margin, above ? aboveTop : below));
  return { left, top, above };
}

interface Props {
  popover: NodePopoverState;
  containerWidth: number;
  containerHeight: number;
  /** 该节点当前是否已是焦点（是则隐藏"设为焦点"按钮） */
  isFocus: boolean;
  onViewDetail: (id: string) => void;
  onSetFocus: (id: string) => void;
  onClose: () => void;
}

export default function GraphNodePopover({
  popover,
  containerWidth,
  containerHeight,
  isFocus,
  onViewDetail,
  onSetFocus,
  onClose,
}: Props) {
  const { person } = popover;
  const { left, top } = clampNodePopoverPosition(
    popover.x,
    popover.y,
    popover.size,
    containerWidth,
    containerHeight,
  );
  return (
    <div
      className="absolute z-20 w-[260px] rounded-lg border bg-card p-3 shadow-lg"
      style={{ left, top }}
      onClick={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="truncate font-semibold text-text-primary">{displayNameOf(person)}</p>
          {(person.company || person.title) && (
            <p className="truncate text-xs text-text-secondary">
              {[person.company, person.title].filter(Boolean).join(' / ')}
            </p>
          )}
          {person.location && <p className="truncate text-xs text-muted">{person.location}</p>}
        </div>
        <IconBtn title="关闭" size="sm" onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </IconBtn>
      </div>
      <div className="mt-3 flex flex-col gap-1.5">
        <button
          type="button"
          className="flex items-center gap-2 rounded-lg border border-line bg-secondary px-3 py-1.5 text-sm text-text-primary transition-colors hover:bg-surface"
          onClick={() => onViewDetail(person.id)}
        >
          <Eye size={15} aria-hidden="true" />
          查看详情
        </button>
        {!isFocus && (
          <button
            type="button"
            className="flex items-center gap-2 rounded-lg bg-accent px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-accent-hover"
            onClick={() => onSetFocus(person.id)}
          >
            <Crosshair size={15} aria-hidden="true" />
            设为焦点
          </button>
        )}
      </div>
    </div>
  );
}

/** 与通讯录/悬停名片一致的脱敏展示名规则 */
function displayNameOf(person: Person) {
  if (person.sensitivityLevel === 'low') return person.name;
  return person.aliases[0] || '高敏感联系人';
}
