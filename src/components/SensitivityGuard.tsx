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
    <div className="rounded-md border border-warning bg-warning-light p-3 text-sm text-warning">
      {fallback ?? <span>敏感信息已隐藏</span>}
      <button
        type="button"
        onClick={() => setRevealed(true)}
        className="ml-3 rounded bg-warning px-2 py-1 text-white hover:opacity-90"
      >
        确认查看
      </button>
    </div>
  );
}
