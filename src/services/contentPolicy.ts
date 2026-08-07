/**
 * 长内容展示策略（统一入口）
 *
 * 规则：
 * - 回复含 resultType 结构化结果 → 维持现有渲染不变（调用方不应用本策略）；
 * - 纯文本回复 ≥1500 字 → FilePanel 附件（气泡内留摘要卡片）；
 * - 400–1500 字 → 气泡内完整渲染 + "收起/展开全文"；
 * - <400 字 → 直接渲染。
 */

/** 小于该长度直接渲染 */
export const INLINE_MAX = 400;
/** 达到该长度进入"气泡内可折叠"区间 */
export const COLLAPSE_MIN = 400;
/** 达到该长度进入 FilePanel 附件 */
export const ATTACHMENT_MIN = 1500;

export type ContentDisplayMode = 'inline' | 'collapsible' | 'attachment';

export interface ContentDisplayDecision {
  mode: ContentDisplayMode;
  /** attachment 模式下气泡内的摘要文案 */
  summary?: string;
}

/** 根据纯文本长度决定展示方式 */
export function resolveTextDisplay(content: string): ContentDisplayDecision {
  const text = content.trim();
  if (text.length >= ATTACHMENT_MIN) {
    return { mode: 'attachment', summary: buildSummary(text) };
  }
  if (text.length >= COLLAPSE_MIN) {
    return { mode: 'collapsible' };
  }
  return { mode: 'inline' };
}

/** 生成附件摘要卡片文案：优先取首段，截断到 140 字 */
function buildSummary(text: string): string {
  const firstParagraph = text.split(/\n{2,}/)[0].trim();
  const base = firstParagraph || text;
  return base.length > 140 ? `${base.slice(0, 140)}…` : base;
}
