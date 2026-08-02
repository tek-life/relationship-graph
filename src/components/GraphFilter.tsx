import { useMemo, useState } from 'react';
import type { Person } from '../types';

export interface FilterState {
  companies: string[];
  locations: string[];
  strengthRange: [number, number];
}

interface Props {
  persons: Person[];
  filter: FilterState;
  onChange: (filter: FilterState) => void;
}

export default function GraphFilter({ persons, filter, onChange }: Props) {
  const [expanded, setExpanded] = useState(false);

  const allCompanies = useMemo(() => {
    const set = new Set<string>();
    for (const p of persons) {
      if (p.company) set.add(p.company);
    }
    return Array.from(set).sort();
  }, [persons]);

  const allLocations = useMemo(() => {
    const set = new Set<string>();
    for (const p of persons) {
      if (p.location) set.add(p.location);
    }
    return Array.from(set).sort();
  }, [persons]);

  const hasActiveFilter = filter.companies.length > 0 || filter.locations.length > 0 || filter.strengthRange[0] > 0 || filter.strengthRange[1] < 1;

  const toggleCompany = (company: string) => {
    const next = filter.companies.includes(company)
      ? filter.companies.filter((c) => c !== company)
      : [...filter.companies, company];
    onChange({ ...filter, companies: next });
  };

  const toggleLocation = (location: string) => {
    const next = filter.locations.includes(location)
      ? filter.locations.filter((l) => l !== location)
      : [...filter.locations, location];
    onChange({ ...filter, locations: next });
  };

  const clearFilter = () => {
    onChange({ companies: [], locations: [], strengthRange: [0, 1] });
  };

  return (
    <div className="absolute left-3 top-3 z-10">
      {/* 折叠按钮 */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className={`rounded-lg p-2 shadow-md transition ${expanded ? 'bg-blue-600 text-white' : 'bg-white/90 text-slate-600 hover:bg-white'} ${hasActiveFilter && !expanded ? 'ring-2 ring-blue-400' : ''}`}
        title="筛选"
      >
        <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
        </svg>
      </button>

      {/* 展开面板 */}
      {expanded && (
        <div className="mt-2 w-64 rounded-xl border bg-white p-4 shadow-lg animate-in fade-in slide-in-from-top-2">
          <div className="flex items-center justify-between mb-3">
            <h4 className="text-sm font-semibold text-slate-700">筛选条件</h4>
            {hasActiveFilter && (
              <button type="button" className="text-xs text-blue-600 hover:underline" onClick={clearFilter}>
                清除
              </button>
            )}
          </div>

          {/* 公司筛选 */}
          {allCompanies.length > 0 && (
            <div className="mb-3">
              <label className="text-xs font-medium text-slate-500">公司</label>
              <div className="mt-1 max-h-28 overflow-y-auto space-y-1">
                {allCompanies.map((company) => (
                  <label key={company} className="flex items-center gap-2 text-sm text-slate-700 cursor-pointer hover:bg-slate-50 rounded px-1">
                    <input
                      type="checkbox"
                      checked={filter.companies.includes(company)}
                      onChange={() => toggleCompany(company)}
                      className="rounded border-slate-300"
                    />
                    <span className="truncate">{company}</span>
                  </label>
                ))}
              </div>
            </div>
          )}

          {/* 城市筛选 */}
          {allLocations.length > 0 && (
            <div className="mb-3">
              <label className="text-xs font-medium text-slate-500">城市/地区</label>
              <div className="mt-1 max-h-28 overflow-y-auto space-y-1">
                {allLocations.map((location) => (
                  <label key={location} className="flex items-center gap-2 text-sm text-slate-700 cursor-pointer hover:bg-slate-50 rounded px-1">
                    <input
                      type="checkbox"
                      checked={filter.locations.includes(location)}
                      onChange={() => toggleLocation(location)}
                      className="rounded border-slate-300"
                    />
                    <span className="truncate">{location}</span>
                  </label>
                ))}
              </div>
            </div>
          )}

          {/* 关系强度滑块 */}
          <div>
            <label className="text-xs font-medium text-slate-500">关系强度范围</label>
            <div className="mt-2 flex items-center gap-2">
              <span className="text-xs text-slate-400">{Math.round(filter.strengthRange[0] * 100)}%</span>
              <input
                type="range"
                min="0"
                max="100"
                value={filter.strengthRange[0] * 100}
                onChange={(e) => {
                  const v = Number(e.target.value) / 100;
                  onChange({ ...filter, strengthRange: [Math.min(v, filter.strengthRange[1]), filter.strengthRange[1]] });
                }}
                className="flex-1 h-1.5 accent-blue-600"
              />
              <input
                type="range"
                min="0"
                max="100"
                value={filter.strengthRange[1] * 100}
                onChange={(e) => {
                  const v = Number(e.target.value) / 100;
                  onChange({ ...filter, strengthRange: [filter.strengthRange[0], Math.max(v, filter.strengthRange[0])] });
                }}
                className="flex-1 h-1.5 accent-blue-600"
              />
              <span className="text-xs text-slate-400">{Math.round(filter.strengthRange[1] * 100)}%</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
