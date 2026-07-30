import { useEffect, useRef } from 'react';
import cytoscape from 'cytoscape';
import type { GraphData } from '../types';

interface Props {
  data: GraphData;
  onNodeClick?: (id: string) => void;
}

export default function GraphView({ data, onNodeClick }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!ref.current) return;

    const cy = cytoscape({
      container: ref.current,
      elements: [
        ...data.nodes.map((node) => ({
          data: {
            id: node.id,
            label: node.label,
            sensitivityLevel: node.sensitivityLevel,
          },
        })),
        ...data.edges.map((edge) => ({
          data: {
            id: edge.id,
            source: edge.source,
            target: edge.target,
            label: edge.label,
          },
        })),
      ],
      style: [
        {
          selector: 'node',
          style: {
            label: 'data(label)',
            width: 64,
            height: 64,
            'background-color': '#2563eb',
            color: '#0f172a',
            'font-size': '12px',
            'text-valign': 'bottom',
            'text-halign': 'center',
            'text-margin-y': 8,
          },
        },
        {
          selector: 'node[sensitivityLevel = "high"]',
          style: { 'background-color': '#dc2626' },
        },
        {
          selector: 'node[sensitivityLevel = "medium"]',
          style: { 'background-color': '#d97706' },
        },
        {
          selector: 'edge',
          style: {
            width: 2,
            'line-color': '#94a3b8',
            'target-arrow-color': '#94a3b8',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            label: 'data(label)',
            'font-size': '10px',
          },
        },
      ],
      layout: { name: 'cose', padding: 40 },
    });

    cy.on('tap', 'node', (event) => onNodeClick?.(event.target.id()));
    return () => cy.destroy();
  }, [data, onNodeClick]);

  if (data.nodes.length === 0) {
    return <div className="rounded-xl border border-dashed p-8 text-center text-slate-500">暂无图谱数据。</div>;
  }

  return <div ref={ref} className="h-[560px] w-full rounded-xl border bg-white" />;
}
