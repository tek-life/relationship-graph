import { ArrowRight } from 'lucide-react';
import type { PathData } from '../types';

interface PathResultDisplayProps {
  path: PathData;
  onPersonClick?: (id: string) => void;
}

export default function PathResultDisplay({ path, onPersonClick }: PathResultDisplayProps) {
  return (
    <div className="rounded-xl border p-4 space-y-4" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
      <div className="flex items-center justify-between">
        <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>路径查找结果</h3>
        <div className="flex items-center gap-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
          <span>{path.hops} 跳</span>
          {path.includesPending && (
            <span className="rounded bg-warning-light px-2 py-0.5 text-warning">含待确认关系</span>
          )}
        </div>
      </div>

      <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>{path.summary}</p>

      {/* 路径节点链可视化 */}
      <div className="flex flex-wrap items-center gap-1">
        {path.nodes.map((node, index) => {
          const edge = index < path.edges.length ? path.edges[index] : null;
          return (
            <span key={node.id} className="flex items-center gap-1">
              <button
                type="button"
                className="inline-flex flex-col items-center rounded-lg border px-3 py-2 text-sm transition hover:opacity-80"
                style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)' }}
                onClick={() => onPersonClick?.(node.id)}
              >
                <span className="font-medium">{node.name}</span>
                {node.company && (
                  <span className="text-xs" style={{ color: 'var(--text-muted)' }}>{node.company}</span>
                )}
              </button>
              {edge && (
                <span className="flex items-center gap-0.5 text-xs" style={{ color: 'var(--text-secondary)' }}>
                  <ArrowRight size={16} aria-hidden="true" />
                  <span className="whitespace-nowrap rounded px-1 py-0.5" style={{ backgroundColor: 'var(--surface-hover)' }}>
                    {edge.relationshipType}
                    {edge.confirmationStatus === 'pending' && (
                      <span className="ml-1 text-warning">?</span>
                    )}
                  </span>
                  <ArrowRight size={16} aria-hidden="true" />
                </span>
              )}
            </span>
          );
        })}
      </div>
    </div>
  );
}
