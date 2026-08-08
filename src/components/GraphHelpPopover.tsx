// UX P1-9：图谱页「?」帮助浮层。
// 原画布下方 100+ 字的长说明收纳于此，由工具栏 IconBtn 触发；
// 点击外部或按 Esc 关闭。

import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { IconBtn } from './ui';

export default function GraphHelpPopover() {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    const onDocDown = (event: PointerEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('pointerdown', onDocDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('pointerdown', onDocDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div ref={wrapperRef} className="relative">
      <IconBtn
        title="图谱操作帮助"
        aria-expanded={open}
        onClick={() => setOpen((prev) => !prev)}
      >
        <span className="text-sm font-semibold" aria-hidden="true">?</span>
      </IconBtn>
      {open && (
      <div
        data-help-panel=""
        className="absolute right-0 top-full z-30 mt-2 w-80 rounded-xl border bg-card p-4 text-sm shadow-lg"
      >
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-text-primary">图谱操作说明</h3>
          <IconBtn title="关闭帮助" size="sm" onClick={() => setOpen(false)}>
            <X size={14} aria-hidden="true" />
          </IconBtn>
        </div>
        <dl className="mt-2 space-y-2 text-xs leading-relaxed text-text-secondary">
          <Item title="全景视图" text={'以"我"为中心辐射展示：内圈=强关系、中圈=中等、外圈=弱关系；滚轮缩放，拖拽平移。'} />
          <Item title="焦点视图" text="通过工具栏选择焦点联系人，展示其 2 跳内人脉；点「重置」回到全景。" />
          <Item title="单击节点" text="弹出快捷浮层，可查看详情或将其设为焦点。" />
          <Item title="圈选" text={'开启圈选后按住左键拖拽画框选人，松手只显示选中的人；再次点「圈选」退出（仅全景模式可用；Shift 拖拽随时可框选）。'} />
          <Item title="找路径" text="先设焦点，再选目标联系人，自动搜索最短关系链并给出引荐建议。" />
          <Item title="边的含义" text="边上标注关系类型；实线=已确认关系，橙色虚线=AI 推断待确认（点击边可确认/否认）；冷却联系人显示为半透明。" />
        </dl>
      </div>
      )}
    </div>
  );
}

function Item({ title, text }: { title: string; text: string }) {
  return (
    <div>
      <dt className="font-medium text-text-primary">{title}</dt>
      <dd className="mt-0.5">{text}</dd>
    </div>
  );
}
