import { useEffect, useMemo, useRef, useState } from 'react';
import { X } from 'lucide-react';
import cytoscape from 'cytoscape';
import { pinyin } from 'pinyin-pro';
import { inferRelationships, setRelationshipConfirmation } from '../services/db';
import type { GraphData, GraphEdge, Person } from '../types';

interface Props {
  data: GraphData;
  personsById: Record<string, Person>;
  onNodeClick?: (id: string) => void;
  /** 推断/确认关系后通知上层重新拉取图数据 */
  onRefresh?: () => void | Promise<void>;
  /** 外部指定初始焦点（如从联系人详情跳转），存在时直接进入关系网络视图 */
  initialFocusId?: string;
}

const LETTERS = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ', '#'];

export default function GraphView({ data, personsById, onNodeClick, onRefresh, initialFocusId }: Props) {
  const [view, setView] = useState<'directory' | 'network'>(initialFocusId ? 'network' : 'directory');
  const persons = useMemo(() => Object.values(personsById), [personsById]);

  // 外部焦点变化时切到关系网络视图（如详情页点击"关系网络"）
  useEffect(() => {
    if (initialFocusId) setView('network');
  }, [initialFocusId]);

  if (persons.length === 0) {
    return <div className="rounded-xl border border-dashed p-8 text-center text-text-secondary">暂无联系人数据。</div>;
  }

  return (
    <div>
      <div className="mb-3 flex items-center justify-between">
        <div className="flex gap-1 rounded-full bg-surface p-1">
          <ViewButton active={view === 'directory'} onClick={() => setView('directory')}>通讯录</ViewButton>
          <ViewButton active={view === 'network'} onClick={() => setView('network')}>关系网络</ViewButton>
        </div>
        <span className="text-sm text-text-secondary">共 {persons.length} 人、{data.edges.length} 条关系</span>
      </div>
      {view === 'directory' ? (
        <ContactDirectory persons={persons} onSelect={onNodeClick} />
      ) : (
        <NetworkView data={data} personsById={personsById} onNodeClick={onNodeClick} onRefresh={onRefresh} initialFocusId={initialFocusId} />
      )}
    </div>
  );
}

function ViewButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-4 py-1.5 text-sm font-medium ${active ? 'bg-card text-accent shadow' : 'text-text-secondary'}`}
    >
      {children}
    </button>
  );
}

// ==================== 通讯录视图 ====================

interface PersonMeta {
  person: Person;
  displayName: string;
  letter: string;
  sortKey: string;
  searchText: string;
}

function ContactDirectory({ persons, onSelect }: { persons: Person[]; onSelect?: (id: string) => void }) {
  const [keyword, setKeyword] = useState('');
  const [hovered, setHovered] = useState<{ person: Person; x: number; y: number } | null>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const sectionRefs = useRef<Record<string, HTMLDivElement | null>>({});

  // 拼音元数据（首字母、排序键、模糊搜索文本）按人缓存
  const metas = useMemo<PersonMeta[]>(
    () =>
      persons.map((person) => {
        const displayName = displayNameOf(person);
        const full = pinyin(displayName, { toneType: 'none', type: 'array' }).join('');
        const initials = pinyin(displayName, { pattern: 'first', toneType: 'none', type: 'array' }).join('');
        const first = (initials[0] || '').toUpperCase();
        const letter = /[A-Z]/.test(first) ? first : '#';
        const searchText = [
          person.name,
          person.aliases.join(' '),
          full,
          initials,
          person.company ?? '',
          person.title ?? '',
          person.location ?? '',
          person.phone ?? '',
          person.resourceTags.join(' '),
        ]
          .join(' ')
          .toLowerCase();
        return { person, displayName, letter, sortKey: full.toLowerCase(), searchText };
      }),
    [persons],
  );

  // 模糊过滤：多个关键词按空格分词，全部命中才保留
  const filtered = useMemo(() => {
    const terms = keyword.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return metas;
    return metas.filter((meta) => terms.every((term) => meta.searchText.includes(term)));
  }, [metas, keyword]);

  // 按首字母分组并组内按拼音排序
  const groups = useMemo(() => {
    const map = new Map<string, PersonMeta[]>();
    for (const meta of filtered) {
      const list = map.get(meta.letter) ?? [];
      list.push(meta);
      map.set(meta.letter, list);
    }
    for (const list of map.values()) {
      list.sort((a, b) => a.sortKey.localeCompare(b.sortKey));
    }
    return map;
  }, [filtered]);

  const jumpTo = (letter: string) => {
    sectionRefs.current[letter]?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  const showTooltip = (meta: PersonMeta, event: React.MouseEvent<HTMLElement>) => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;
    const wrapperRect = wrapper.getBoundingClientRect();
    const rect = event.currentTarget.getBoundingClientRect();
    setHovered({
      person: meta.person,
      x: rect.left - wrapperRect.left + rect.width / 2,
      y: rect.bottom - wrapperRect.top,
    });
  };

  return (
    <div ref={wrapperRef} className="relative rounded-xl border bg-card shadow-sm">
      <div className="border-b p-4">
        <input
          type="search"
          className="input"
          placeholder="搜索姓名、拼音、公司、职位、城市、标签、电话…（空格分隔多个条件）"
          value={keyword}
          onChange={(event) => setKeyword(event.target.value)}
        />
        {keyword && (
          <p className="mt-2 text-sm text-text-secondary">匹配 {filtered.length} 人{filtered.length === 0 ? '，换个关键词试试' : ''}</p>
        )}
      </div>

      <div className="flex">
        {/* 联系人分组网格 */}
        <div className="h-[600px] flex-1 overflow-y-auto scroll-pt-2 p-4 pr-10">
          {LETTERS.filter((letter) => groups.has(letter)).map((letter) => (
            <div
              key={letter}
              ref={(el) => {
                sectionRefs.current[letter] = el;
              }}
              className="mb-5"
            >
              <div className="sticky top-0 z-[5] -mx-1 bg-card/95 px-1 py-1">
                <span className="text-sm font-bold text-accent">{letter}</span>
                <span className="ml-2 text-xs text-muted">{groups.get(letter)!.length} 人</span>
              </div>
              <div className="mt-1 grid grid-cols-[repeat(auto-fill,minmax(88px,1fr))] gap-x-2 gap-y-4">
                {groups.get(letter)!.map((meta) => (
                  <button
                    key={meta.person.id}
                    type="button"
                    className="group flex flex-col items-center rounded-lg p-2 text-center hover:bg-surface"
                    onClick={() => onSelect?.(meta.person.id)}
                    onMouseEnter={(event) => showTooltip(meta, event)}
                    onMouseLeave={() => setHovered(null)}
                  >
                    <Avatar meta={meta} />
                    <span className="mt-1.5 w-full truncate text-sm text-text-primary">{meta.displayName}</span>
                    {meta.person.company && (
                      <span className="w-full truncate text-xs text-muted">{meta.person.company}</span>
                    )}
                  </button>
                ))}
              </div>
            </div>
          ))}
          {filtered.length === 0 && (
            <p className="py-16 text-center text-muted">没有匹配的联系人</p>
          )}
        </div>

        {/* 右侧拼音字母索引条 */}
        <div className="absolute bottom-2 right-1 top-20 flex w-7 flex-col items-center justify-center gap-0.5 text-xs">
          {LETTERS.map((letter) => {
            const enabled = groups.has(letter);
            return (
              <button
                key={letter}
                type="button"
                disabled={!enabled}
                onClick={() => jumpTo(letter)}
                className={`h-[3.4%] min-h-4 w-6 rounded leading-none ${
                  enabled ? 'font-medium text-accent hover:bg-accent-light' : 'cursor-default text-muted'
                }`}
              >
                {letter}
              </button>
            );
          })}
        </div>
      </div>

      {hovered && <PersonHoverCard person={hovered.person} x={hovered.x} y={hovered.y} />}
    </div>
  );
}

function Avatar({ meta }: { meta: PersonMeta }) {
  const { person, displayName } = meta;
  if (person.avatar) {
    return <img src={person.avatar} alt={displayName} className="h-16 w-16 rounded-full object-cover" />;
  }
  return (
    <span
      className={`flex h-16 w-16 items-center justify-center rounded-full text-xl font-semibold text-white ${avatarClass(person.sensitivityLevel)}`}
    >
      {displayName.slice(0, 1)}
    </span>
  );
}

function avatarClass(level: string) {
  if (level === 'high') return 'bg-danger';
  if (level === 'medium') return 'bg-warning';
  return 'bg-accent';
}

function displayNameOf(person: Person) {
  if (person.sensitivityLevel === 'low') return person.name;
  return person.aliases[0] || '高敏感联系人';
}

// ==================== 悬停名片 ====================

function PersonHoverCard({ person, x, y }: { person: Person; x: number; y: number }) {
  const displayName = displayNameOf(person);
  return (
    <div
      className="pointer-events-none absolute z-20 w-64 rounded-lg border bg-card p-3 shadow-lg"
      style={{ left: Math.max(8, Math.min(x - 128, (typeof window !== 'undefined' ? window.innerWidth : 1200) - 280)), top: y + 6 }}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="font-semibold text-text-primary">{displayName}</span>
        <span className={`badge ${sensitivityClass(person.sensitivityLevel)}`}>{sensitivityText(person.sensitivityLevel)}</span>
      </div>
      <dl className="mt-2 space-y-1 text-xs text-text-secondary">
        {(person.company || person.title) && <Row label="公司/职位" value={[person.company, person.title].filter(Boolean).join(' / ')} />}
        {person.location && <Row label="城市" value={person.location} />}
        <Row label="关系强度" value={strengthText(person.relationshipStrength)} />
        <Row label="状态" value={statusText(person.status)} />
        {person.nextStep && <Row label="下一步" value={person.nextStep} />}
      </dl>
      {person.resourceTags.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {person.resourceTags.slice(0, 4).map((tag) => (
            <span key={tag} className="badge bg-accent-light text-accent">{tag}</span>
          ))}
        </div>
      )}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[4rem_1fr] gap-2">
      <dt className="text-muted">{label}</dt>
      <dd className="truncate">{value}</dd>
    </div>
  );
}

// ==================== 关系网络视图（焦点模式 + 推断确认 + 路径） ====================

interface PathResult {
  nodeIds: string[];
  edgeIds: string[];
  edges: GraphEdge[];
  includesPending: boolean;
}

/** 虚拟中心节点：代表用户本人，不存在于联系人数据中 */
const ME_ID = '__me__';
/** 焦点模式下"我 → 焦点"虚拟边的固定 id */
const ME_EDGE_ID = '__me_edge__';

function NetworkView({ data, personsById, onNodeClick, onRefresh, initialFocusId }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);
  const [focusId, setFocusId] = useState<string | null>(null);
  const [targetId, setTargetId] = useState<string | null>(null);
  const [focusInput, setFocusInput] = useState('');
  const [targetInput, setTargetInput] = useState('');
  const [selectedEdge, setSelectedEdge] = useState<GraphEdge | null>(null);
  const [busy, setBusy] = useState(false);
  const [building, setBuilding] = useState(false);
  // 圈选模式：拖拽框选节点后只显示选中子图
  const [boxMode, setBoxMode] = useState(false);
  const boxModeRef = useRef(false);
  const [selection, setSelection] = useState<Set<string> | null>(null);
  // 实例重建时用于恢复视野（同一子图切换圈选模式不丢失缩放/平移）
  const viewportRef = useRef<{ sig: string; zoom: number; pan: { x: number; y: number } } | null>(null);
  const [notice, setNotice] = useState('');
  /** notice 是否为错误（错误走 danger 语义色，普通提示走 accent） */
  const [noticeIsError, setNoticeIsError] = useState(false);
  const [tooltip, setTooltip] = useState<{ person: Person; x: number; y: number } | null>(null);
  // 外部初始焦点只应用一次，避免数据刷新后覆盖用户手动重置的焦点
  const appliedInitialFocusRef = useRef<string | null>(null);

  const labelById = useMemo(() => {
    const map: Record<string, string> = {};
    for (const node of data.nodes) map[node.id] = node.label;
    return map;
  }, [data.nodes]);

  // 外部指定初始焦点（详情页"关系网络"入口）：等数据就绪后同步焦点与输入框
  useEffect(() => {
    if (!initialFocusId || appliedInitialFocusRef.current === initialFocusId) return;
    if (!labelById[initialFocusId]) return;
    appliedInitialFocusRef.current = initialFocusId;
    setFocusId(initialFocusId);
    setFocusInput(labelById[initialFocusId]);
    setSelectedEdge(null);
    setSelection(null);
  }, [initialFocusId, labelById]);

  const idByLabel = useMemo(() => {
    const map = new Map<string, string>();
    for (const node of data.nodes) {
      if (!map.has(node.label)) map.set(node.label, node.id);
    }
    return map;
  }, [data.nodes]);

  const adjacency = useMemo(() => {
    const map = new Map<string, GraphEdge[]>();
    const push = (id: string, edge: GraphEdge) => {
      const list = map.get(id);
      if (list) list.push(edge);
      else map.set(id, [edge]);
    };
    for (const edge of data.edges) {
      push(edge.source, edge);
      push(edge.target, edge);
    }
    return map;
  }, [data.edges]);

  // 焦点：仅手动指定；默认以"我"为中心展示辐射全景
  const effectiveFocus = useMemo(
    () => (focusId && labelById[focusId] ? focusId : null),
    [focusId, labelById],
  );

  // 焦点 2 跳邻域
  const depthMap = useMemo(() => {
    if (!effectiveFocus) return null;
    const depths = new Map<string, number>([[effectiveFocus, 0]]);
    let frontier = [effectiveFocus];
    for (let depth = 1; depth <= 2; depth++) {
      const next: string[] = [];
      for (const id of frontier) {
        for (const edge of adjacency.get(id) ?? []) {
          const other = edge.source === id ? edge.target : edge.source;
          if (!depths.has(other)) {
            depths.set(other, depth);
            next.push(other);
          }
        }
      }
      frontier = next;
    }
    return depths;
  }, [effectiveFocus, adjacency]);

  // 最短路径：优先只走已确认关系，走不通再放宽到含待确认边
  const path = useMemo<PathResult | null>(() => {
    if (!effectiveFocus || !targetId || effectiveFocus === targetId) return null;
    const bfs = (allowPending: boolean): PathResult | null => {
      const prev = new Map<string, { node: string; edge: GraphEdge }>();
      const visited = new Set([effectiveFocus]);
      let frontier = [effectiveFocus];
      while (frontier.length > 0) {
        const next: string[] = [];
        for (const id of frontier) {
          for (const edge of adjacency.get(id) ?? []) {
            if (!allowPending && edge.confirmationStatus === 'pending') continue;
            const other = edge.source === id ? edge.target : edge.source;
            if (visited.has(other)) continue;
            visited.add(other);
            prev.set(other, { node: id, edge });
            if (other === targetId) {
              const nodeIds = [targetId];
              const edges: GraphEdge[] = [];
              let cursor = targetId;
              while (cursor !== effectiveFocus) {
                const step = prev.get(cursor)!;
                edges.unshift(step.edge);
                cursor = step.node;
                nodeIds.unshift(cursor);
              }
              return {
                nodeIds,
                edges,
                edgeIds: edges.map((item) => item.id),
                includesPending: edges.some((item) => item.confirmationStatus === 'pending'),
              };
            }
            next.push(other);
          }
        }
        frontier = next;
      }
      return null;
    };
    return bfs(false) ?? bfs(true);
  }, [effectiveFocus, targetId, adjacency]);

  // 可见子图 = 焦点邻域 ∪ 路径节点（无焦点时全量）
  const visible = useMemo(() => {
    if (!depthMap) return null;
    const ids = new Set(depthMap.keys());
    path?.nodeIds.forEach((id) => ids.add(id));
    return ids;
  }, [depthMap, path]);

  const pendingCount = useMemo(
    () => data.edges.filter((edge) => edge.confirmationStatus === 'pending').length,
    [data.edges],
  );

  useEffect(() => {
    if (!ref.current) return;
    boxModeRef.current = boxMode;

    const baseNodes = visible ? data.nodes.filter((node) => visible.has(node.id)) : data.nodes;
    // 圈选过滤只在"我"为中心的全景模式下生效：仅保留选中节点
    const nodes = !effectiveFocus && selection
      ? baseNodes.filter((node) => selection.has(node.id))
      : baseNodes;
    const shownIds = new Set(nodes.map((node) => node.id));
    const edges = data.edges.filter((edge) => shownIds.has(edge.source) && shownIds.has(edge.target));
    const pathNodes = new Set(path?.nodeIds ?? []);
    const pathEdges = new Set(path?.edgeIds ?? []);
    const isLarge = nodes.length > 120;
    // 全景模式"我"为中心辐射；焦点模式"我"也保留，仅通过一条虚拟边挂在焦点上
    const includeMe = !effectiveFocus;

    // UX P0-2：画布颜色改从主题 CSS 令牌读取，三套主题下语义一致；
    // data-theme 切换时由下方 MutationObserver 重新套用样式
    const cssVar = (name: string, fallback: string) => {
      const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
      return value || fallback;
    };
    const buildGraphStyle = (): cytoscape.StylesheetJson => {
      const accent = cssVar('--accent-color', '#3b82f6');
      const accentHover = cssVar('--accent-hover', '#2563eb');
      const accentLight = cssVar('--accent-light', '#eff6ff');
      const danger = cssVar('--danger-color', '#dc2626');
      const warning = cssVar('--warning', '#d97706');
      const success = cssVar('--success', '#16a34a');
      const textPrimary = cssVar('--text-primary', '#0f172a');
      const textSecondary = cssVar('--text-secondary', '#64748b');
      const textMuted = cssVar('--text-muted', '#94a3b8');
      const line = cssVar('--border-color', '#e2e8f0');
      const card = cssVar('--bg-card', '#ffffff');
      return [
        {
          selector: 'node',
          style: {
            label: 'data(label)',
            width: 'data(size)',
            height: 'data(size)',
            'background-color': accent,
            color: textPrimary,
            'font-size': isLarge ? '10px' : '12px',
            'text-valign': 'bottom',
            'text-halign': 'center',
            'text-margin-y': 4,
            'min-zoomed-font-size': 8,
          },
        },
        { selector: 'node[sensitivityLevel = "high"]', style: { 'background-color': danger } },
        { selector: 'node[sensitivityLevel = "medium"]', style: { 'background-color': warning } },
        { selector: 'node.cold', style: { 'background-opacity': 0.45 } },
        { selector: 'node.focus', style: { 'border-width': 4, 'border-color': accentHover, 'font-weight': 'bold' } },
        {
          selector: 'node.me',
          style: {
            'background-color': accentHover,
            'border-width': 4,
            'border-color': accentLight,
            color: textPrimary,
            'font-weight': 'bold',
            'font-size': isLarge ? '13px' : '15px',
          },
        },
        {
          selector: 'edge',
          style: {
            width: 2,
            'line-color': textMuted,
            'target-arrow-shape': 'none',
            'curve-style': isLarge ? 'straight' : 'bezier',
            label: 'data(label)',
            'font-size': '10px',
            color: textSecondary,
            'text-rotation': 'autorotate',
            'text-background-color': card,
            'text-background-opacity': 0.85,
            'text-background-padding': '2px',
            // 大图缩得太小时隐藏标签避免糊成一团，放大后自动浮现
            'min-zoomed-font-size': isLarge ? 10 : 5,
          },
        },
        {
          selector: 'edge.me-edge',
          style: { width: 1, 'line-color': line, opacity: 0.6, label: '' },
        },
        {
          // "我 → 焦点"虚拟边：灰色点线，与真实关系边（实线/橙色虚线）区分
          selector: 'edge.me-focus-edge',
          style: {
            width: 1.5,
            'line-style': 'dotted',
            'line-color': line,
            color: textMuted,
            opacity: 0.8,
          },
        },
        {
          selector: 'edge.pending',
          style: { 'line-style': 'dashed', 'line-color': warning, width: 2 },
        },
        { selector: 'node.onpath', style: { 'border-width': 4, 'border-color': success } },
        { selector: 'edge.onpath', style: { 'line-color': success, width: 4 } },
        { selector: '.faded', style: { opacity: 0.15 } },
      ] as unknown as cytoscape.StylesheetJson;
    };

    const cy = cytoscape({
      container: ref.current,
      elements: [
        { data: { id: ME_ID, label: '我', size: effectiveFocus ? 60 : 80 }, classes: 'me', selectable: false },
        ...nodes.map((node) => ({
          data: {
            id: node.id,
            label: node.label,
            sensitivityLevel: node.sensitivityLevel,
            size: nodeSize(node.id, effectiveFocus, personsById),
          },
          classes: [
            node.id === effectiveFocus ? 'focus' : '',
            path ? (pathNodes.has(node.id) ? 'onpath' : 'faded') : '',
            node.status === 'cold' ? 'cold' : '',
          ].join(' '),
        })),
        // "我"到每位联系人的辐射线：仅视觉引导，不参与路径/邻域计算
        ...(includeMe
          ? nodes.map((node) => ({
              data: { id: `me-${node.id}`, source: ME_ID, target: node.id, label: '' },
              classes: 'me-edge',
            }))
          : []),
        // 焦点模式："我 → 焦点"虚拟边，仅展示层追加，不进入 adjacency/BFS
        ...(effectiveFocus
          ? [{
              data: { id: ME_EDGE_ID, source: ME_ID, target: effectiveFocus, label: '我的联系人' },
              classes: 'me-focus-edge',
            }]
          : []),
        ...edges.map((edge) => ({
          data: { id: edge.id, source: edge.source, target: edge.target, label: relationshipLabel(edge.label) },
          classes: [
            edge.confirmationStatus === 'pending' ? 'pending' : '',
            path ? (pathEdges.has(edge.id) ? 'onpath' : 'faded') : '',
          ].join(' '),
        })),
      ],
      style: buildGraphStyle(),
      layout: { name: 'preset' },
      minZoom: 0.05,
      maxZoom: 3,
      userZoomingEnabled: true,
      wheelSensitivity: 0.3,
      // 框选常开：按 cytoscape 源码逻辑，Shift/Ctrl+拖拽任何时候都可框选；
      // 开启圈选模式后再关闭平移，普通左键拖拽也直接画选框
      boxSelectionEnabled: true,
      userPanningEnabled: !boxMode,
      autoungrabify: boxMode,
      pixelRatio: isLarge ? 1 : undefined,
      textureOnViewport: isLarge && !boxMode,
      hideEdgesOnViewport: isLarge && !boxMode,
    });
    cyRef.current = cy;

    // UX P0-2：主题切换（data-theme）时重新套用画布样式，保持与 DOM 同主题
    const themeObserver = new MutationObserver(() => {
      cy.style(buildGraphStyle());
    });
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });

    // 布局手动执行：小图带"从中心展开"的构建动画，大图直接就位避免卡顿
    const layoutOptions = effectiveFocus
      ? {
          name: 'concentric',
          padding: 40,
          minNodeSpacing: 24,
          concentric: (node: any) => {
            // "我"不在 depthMap 中，固定放在与一跳联系人同环（与焦点直接相连）
            if (node.id() === ME_ID) return 2;
            return 3 - (depthMap?.get(node.id()) ?? 3);
          },
          levelWidth: () => 1,
        }
      : {
          // 以"我"为圆心的辐射布局：内圈强关系、中圈中等、外圈弱关系
          name: 'concentric',
          padding: isLarge ? 30 : 40,
          minNodeSpacing: isLarge ? 12 : 28,
          concentric: (node: any) => {
            if (node.id() === ME_ID) return 10;
            const strength = personsById[node.id()]?.relationshipStrength;
            if (strength === 'strong') return 3;
            if (strength === 'medium') return 2;
            return 1;
          },
          levelWidth: () => 1,
        };
    // 视图签名：同一子图仅因圈选模式开关而重建实例时，原样恢复缩放与平移
    const sig = `${effectiveFocus ?? ''}|${!effectiveFocus && selection ? Array.from(selection).sort().join(',') : ''}|${nodes.length}|${edges.length}`;
    const restore = viewportRef.current && viewportRef.current.sig === sig ? viewportRef.current : null;
    setBuilding(!restore);
    const layout = cy.layout({
      ...layoutOptions,
      animate: nodes.length <= 400 && !restore,
      animationDuration: 700,
      animationEasing: 'ease-out',
    } as any);
    layout.one('layoutstop', () => {
      setBuilding(false);
      if (restore) {
        cy.viewport({ zoom: restore.zoom, pan: restore.pan });
      } else if (!effectiveFocus && cy.zoom() < 0.9) {
        // 初始显示保证节点文字可读：全景 fit 过小时放大到 1:1 并居中到"我"
        cy.zoom(1);
        const me = cy.$id(ME_ID);
        if (me.length > 0) cy.center(me);
      }
    });
    layout.run();

    // 框选完成：收集选中节点，切换为"选中节点 + 我"的聚焦子图（全景模式下生效）
    cy.on('boxend', () => {
      if (effectiveFocus) return;
      setTimeout(() => {
        const ids = cy
          .nodes(':selected')
          .map((node) => node.id())
          .filter((id) => id !== ME_ID);
        if (ids.length > 0) {
          setSelection(new Set(ids));
          setBoxMode(false);
          boxModeRef.current = false;
        }
      }, 60);
    });

    // 单击进入联系人详情；双击切换焦点。用定时器区分单击与双击
    let tapTimer: ReturnType<typeof setTimeout> | null = null;
    cy.on('tap', 'node', (event) => {
      const id = event.target.id();
      setTooltip(null);
      if (id === ME_ID || boxModeRef.current) return;
      if (tapTimer) clearTimeout(tapTimer);
      tapTimer = setTimeout(() => {
        tapTimer = null;
        onNodeClick?.(id);
      }, 280);
    });
    cy.on('dbltap', 'node', (event) => {
      if (tapTimer) {
        clearTimeout(tapTimer);
        tapTimer = null;
      }
      const id = event.target.id();
      setTooltip(null);
      if (id === ME_ID) {
        setFocusId(null);
        setFocusInput('');
        return;
      }
      setFocusId(id);
      setFocusInput(labelById[id] ?? '');
      setSelectedEdge(null);
      setSelection(null);
    });
    cy.on('tap', 'edge', (event) => {
      const id = event.target.id();
      if (id.startsWith('me-') || id === ME_EDGE_ID) return;
      const edge = data.edges.find((item) => item.id === id);
      setSelectedEdge(edge ?? null);
    });
    cy.on('mouseover', 'node', (event) => {
      const person = personsById[event.target.id()];
      if (!person) return;
      const pos = event.target.renderedPosition();
      setTooltip({ person, x: pos.x, y: pos.y });
    });
    cy.on('mouseout', 'node', () => setTooltip(null));
    cy.on('pan zoom drag', () => setTooltip(null));

    return () => {
      if (tapTimer) clearTimeout(tapTimer);
      themeObserver.disconnect();
      // 保存视野，便于圈选模式切换重建实例后无缝恢复
      viewportRef.current = { sig, zoom: cy.zoom(), pan: cy.pan() };
      cyRef.current = null;
      cy.destroy();
    };
  }, [data, personsById, onNodeClick, effectiveFocus, depthMap, visible, path, labelById, selection, boxMode]);

  const handleInfer = async () => {
    setBusy(true);
    setNotice('');
    setNoticeIsError(false);
    try {
      const { created } = await inferRelationships();
      setNotice(created > 0 ? `AI 新增 ${created} 条待确认关系（橙色虚线，点击可确认）` : '没有发现新的可推断关系');
      await onRefresh?.();
    } catch (err) {
      setNoticeIsError(true);
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleConfirm = async (status: 'confirmed' | 'rejected') => {
    if (!selectedEdge) return;
    setBusy(true);
    try {
      await setRelationshipConfirmation(selectedEdge.id, status);
      setSelectedEdge(null);
      await onRefresh?.();
    } catch (err) {
      setNoticeIsError(true);
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const resolveInput = (value: string, setter: (id: string | null) => void) => {
    const id = idByLabel.get(value.trim());
    setter(id ?? null);
  };

  const focusLabel = effectiveFocus ? labelById[effectiveFocus] : null;

  return (
    <div className="relative">
      {/* 工具栏：焦点 / 路径目标 / AI 推断 */}
      <div className="mb-3 flex flex-wrap items-center gap-2 rounded-xl border bg-card p-3 text-sm shadow-sm">
        <label className="text-text-secondary">焦点</label>
        <input
          className="input !w-40 !py-1.5"
          list="network-person-options"
          placeholder="输入姓名聚焦"
          value={focusInput}
          onChange={(event) => {
            setFocusInput(event.target.value);
            resolveInput(event.target.value, setFocusId);
          }}
        />
        {focusLabel && (
          <>
            <span className="rounded-full bg-accent-light px-3 py-1 text-accent">当前：{focusLabel}</span>
            <button type="button" className="text-accent hover:underline" onClick={() => effectiveFocus && onNodeClick?.(effectiveFocus)}>
              查看详情
            </button>
            <button
              type="button"
              className="text-text-secondary hover:underline"
              onClick={() => { setFocusId(null); setFocusInput(''); setTargetId(null); setTargetInput(''); }}
            >
              重置
            </button>
          </>
        )}
        <span className="mx-1 h-5 w-px bg-line" />
        <label className="text-text-secondary">找路径到</label>
        <input
          className="input !w-40 !py-1.5"
          list="network-person-options"
          placeholder="目标联系人"
          value={targetInput}
          disabled={!effectiveFocus}
          onChange={(event) => {
            setTargetInput(event.target.value);
            resolveInput(event.target.value, setTargetId);
          }}
        />
        <span className="mx-1 h-5 w-px bg-line" />
        <button type="button" className="btn-secondary !py-1.5" onClick={handleInfer} disabled={busy}>
          {busy ? '处理中...' : 'AI 推断关系'}
        </button>
        {pendingCount > 0 && <span className="rounded-full bg-warning-light px-3 py-1 text-warning">待确认 {pendingCount}</span>}
        <span className="mx-1 h-5 w-px bg-line" />
        <button
          type="button"
          className={`rounded px-3 py-1.5 text-sm font-medium ${boxMode ? 'bg-accent text-white' : 'bg-secondary text-text-secondary hover:bg-surface'}`}
          disabled={!!effectiveFocus}
          title={effectiveFocus ? '圈选仅在全景模式下可用，请先重置焦点' : '开启后按住左键拖拽框选联系人'}
          onClick={() => setBoxMode((prev) => !prev)}
        >
          {boxMode ? '圈选中：拖拽框选' : '圈选'}
        </button>
        {selection && (
          <>
            <span className="rounded-full bg-success-light px-3 py-1 text-success">已圈选 {selection.size} 人</span>
            <button type="button" className="text-text-secondary hover:underline" onClick={() => setSelection(null)}>
              清除圈选
            </button>
          </>
        )}
        <datalist id="network-person-options">
          {data.nodes.map((node) => <option key={node.id} value={node.label} />)}
        </datalist>
      </div>

      {notice && <div className={`mb-3 rounded p-2 text-sm ${noticeIsError ? 'bg-danger-light text-danger' : 'bg-accent-light text-accent'}`}>{notice}</div>}

      <div className="relative">
        <div
          ref={ref}
          className="h-[560px] w-full rounded-xl border bg-card"
          style={{ cursor: boxMode ? 'crosshair' : undefined }}
        />
        {building && (
          <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-xl bg-card/40">
            <div className="flex items-center gap-2.5 rounded-full bg-card px-5 py-2.5 text-sm text-text-secondary shadow-lg">
              <span className="h-4 w-4 animate-spin rounded-full border-2 border-accent border-t-transparent" />
              正在构建关系网络…
            </div>
          </div>
        )}
      </div>
      <p className="mt-2 text-xs text-muted">
        {boxMode
          ? '圈选模式：按住鼠标左键拖拽出矩形框选联系人，松开后只显示选中的人及其与"我"的连线；再次点击"圈选"按钮可退出。'
          : effectiveFocus
            ? '正在展示焦点 2 跳内的人脉；单击节点进入联系人详情，双击节点切换焦点，点"重置"回到以我为中心的全景。'
            : '以"我"为中心辐射展示：内圈=强关系、中圈=中等、外圈=弱关系；滚轮缩放视图，单击节点进入详情，双击节点聚焦其人脉圈，按住 Shift 拖拽可直接框选。'}
        边上标注关系类型；实线=已确认关系，橙色虚线=AI 推断待确认（点击边可确认/否认），冷却联系人显示为半透明。
        <span className="ml-2 select-none text-muted">v20260731-boxfix</span>
      </p>

      {/* 路径结果面板 */}
      {targetId && effectiveFocus && (
        <div className="mt-3 rounded-xl border bg-card p-4 shadow-sm">
          {path ? (
            <>
              <h3 className="font-semibold text-text-primary">
                路径：{path.nodeIds.map((id) => labelById[id]).join(' → ')}
                <span className="ml-2 text-sm font-normal text-text-secondary">（{path.edges.length} 跳{path.includesPending ? '，含待确认关系' : ''}）</span>
              </h3>
              <ol className="mt-2 space-y-1 text-sm text-text-secondary">
                {path.edges.map((edge, index) => (
                  <li key={edge.id}>
                    第 {index + 1} 跳：{labelById[path.nodeIds[index]]} → {labelById[path.nodeIds[index + 1]]}
                    <span className="text-muted">
                      （{edge.inferenceReason ?? relationshipLabel(edge.label)}{edge.confirmationStatus === 'pending' ? '，待确认' : ''}）
                    </span>
                  </li>
                ))}
              </ol>
              <p className="mt-2 rounded bg-success-light p-2 text-sm text-success">
                建议行动：联系 {labelById[path.nodeIds[1]]}
                {path.nodeIds.length > 2 ? `，询问是否认识 ${labelById[path.nodeIds[2]]}，请求引荐` : '，可直接推进'}。
                {path.includesPending ? ' 注意：路径中含未确认关系，建议先向对方核实。' : ''}
              </p>
            </>
          ) : (
            <p className="text-sm text-text-secondary">未找到 {focusLabel} 到 {labelById[targetId]} 的关系路径，可先补录中间人关系。</p>
          )}
        </div>
      )}

      {/* 推断边确认面板 */}
      {selectedEdge && (
        <div className="mt-3 rounded-xl border bg-card p-4 shadow-sm">
          <div className="flex items-center justify-between">
            <h3 className="font-semibold text-text-primary">
              {labelById[selectedEdge.source]} — {labelById[selectedEdge.target]}
              <span className="ml-2 text-sm font-normal text-text-secondary">{relationshipLabel(selectedEdge.label)}</span>
            </h3>
            <button type="button" className="text-muted hover:text-text-secondary" onClick={() => setSelectedEdge(null)}>
              <X size={16} aria-hidden="true" />
            </button>
          </div>
          {selectedEdge.confirmationStatus === 'pending' ? (
            <>
              <p className="mt-1 text-sm text-text-secondary">
                AI 推断依据：{selectedEdge.inferenceReason ?? '未记录'}
                {selectedEdge.confidence != null && `（置信度 ${(selectedEdge.confidence * 100).toFixed(0)}%）`}
              </p>
              <div className="mt-3 flex gap-2">
                <button type="button" className="btn-primary !py-1.5" disabled={busy} onClick={() => handleConfirm('confirmed')}>确认认识</button>
                <button
                  type="button"
                  className="rounded bg-secondary px-4 py-1.5 text-sm text-text-primary hover:bg-surface"
                  disabled={busy}
                  onClick={() => handleConfirm('rejected')}
                >
                  不认识（隐藏）
                </button>
              </div>
            </>
          ) : (
            <p className="mt-1 text-sm text-text-secondary">已确认的关系{selectedEdge.inferenceReason ? `（最初由 AI 推断：${selectedEdge.inferenceReason}）` : ''}。</p>
          )}
        </div>
      )}

      {tooltip && <PersonHoverCard person={tooltip.person} x={tooltip.x} y={tooltip.y} />}
    </div>
  );
}

function nodeSize(id: string, focusId: string | null, personsById: Record<string, Person>): number {
  if (id === focusId) return 70;
  const strength = personsById[id]?.relationshipStrength;
  if (strength === 'strong') return 58;
  if (strength === 'medium') return 48;
  return 38;
}

function relationshipLabel(value: string) {
  const map: Record<string, string> = {
    introduced: '介绍认识',
    colleague: '同事',
    friend: '朋友',
    cooperation: '合作',
    other: '可能认识',
  };
  return map[value] ?? value;
}

// ==================== 文案辅助 ====================

function strengthText(value?: string | null) {
  if (value === 'strong') return '强';
  if (value === 'weak') return '弱';
  if (value === 'medium') return '中';
  return '未标注';
}

function sensitivityText(value: string) {
  if (value === 'high') return '高敏感';
  if (value === 'medium') return '中敏感';
  return '低敏感';
}

function sensitivityClass(value: string) {
  if (value === 'high') return 'bg-danger-light text-danger';
  if (value === 'medium') return 'bg-warning-light text-warning';
  return 'bg-success-light text-success';
}

function statusText(value: string) {
  if (value === 'follow-up') return '待跟进';
  if (value === 'cold') return '冷却';
  return '活跃';
}
