// UX P1-9：图谱节点单击浮层的定位纯函数用例（显隐相关的定位/翻转/钳制逻辑）。

import { describe, expect, it } from 'vitest';
import {
  clampNodePopoverPosition,
  NODE_POPOVER_HEIGHT,
  NODE_POPOVER_WIDTH,
} from './GraphNodePopover';

const W = 1000;
const H = 600;

describe('clampNodePopoverPosition', () => {
  it('默认摆在节点正下方并水平居中对齐', () => {
    const { left, top, above } = clampNodePopoverPosition(500, 200, 60, W, H);
    expect(above).toBe(false);
    expect(left).toBe(500 - NODE_POPOVER_WIDTH / 2);
    // 节点半径 30 + 间距 10
    expect(top).toBe(200 + 30 + 10);
  });

  it('节点靠下、下方放不下时翻转到节点上方', () => {
    const { top, above } = clampNodePopoverPosition(500, H - 60, 60, W, H);
    expect(above).toBe(true);
    expect(top).toBe(H - 60 - 30 - 10 - NODE_POPOVER_HEIGHT);
  });

  it('上下都放不下时保持下方位置不被钳出负值', () => {
    // 节点过大导致下方/上方都装不下浮层，退回下方摆放（top 不小于最小边距）
    const { top, above } = clampNodePopoverPosition(500, 380, 400, W, H);
    expect(above).toBe(false);
    expect(top).toBeGreaterThanOrEqual(8);
    expect(top).toBe(380 + 200 + 10);
  });

  it('节点贴近左缘时水平钳制到最小边距', () => {
    const { left } = clampNodePopoverPosition(10, 200, 60, W, H);
    expect(left).toBe(8);
  });

  it('节点贴近右缘时水平钳制到容器内', () => {
    const { left } = clampNodePopoverPosition(W - 10, 200, 60, W, H);
    expect(left).toBe(W - NODE_POPOVER_WIDTH - 8);
  });

  it('支持自定义浮层尺寸', () => {
    const { left, top } = clampNodePopoverPosition(300, 100, 40, W, H, 100, 80);
    expect(left).toBe(300 - 50);
    expect(top).toBe(100 + 20 + 10);
  });
});
