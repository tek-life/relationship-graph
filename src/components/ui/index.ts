// 全局组件基座统一导出入口。
// 业务页面请从 './ui'（或 '@/components/ui'）引入，勿直接引子模块文件。

export { ToastProvider, useToast } from './Toast';
export type { ToastApi, ToastOptions, ToastType } from './Toast';

export { ConfirmDialog } from './ConfirmDialog';
export type { ConfirmDialogProps } from './ConfirmDialog';

export { IconBtn } from './IconBtn';
export type { IconBtnProps, IconBtnSize } from './IconBtn';

export { Segmented } from './Segmented';
export type { SegmentedOption, SegmentedProps } from './Segmented';

export { Badge } from './Badge';
export type { BadgeProps, BadgeVariant } from './Badge';

export { Card } from './Card';
export type { CardProps } from './Card';

export { EmptyState, ErrorBanner, LoadingSpinner, StatusBadge } from './Feedback';
