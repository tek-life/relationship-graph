import { useState } from 'react';
import type { SensitivityLevel } from '../types';

interface Props {
  level: SensitivityLevel;
  children: React.ReactNode;
  fallback?: React.ReactNode;
}

export default function SensitivityGuard({ level, children, fallback }: Props) {
  const [revealed, setRevealed] = useState(level === 'low');

  if (revealed) {
    return <>{children}</>;
  }

  return (
    <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
      {fallback ?? <span>敏感信息已隐藏</span>}
      <button
        type="button"
        onClick={() => setRevealed(true)}
        className="ml-3 rounded bg-amber-600 px-2 py-1 text-white hover:bg-amber-700"
      >
        确认查看
      </button>
    </div>
  );
}
