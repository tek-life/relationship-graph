/**
 * UX P0-5：主导航 IA 收敛配置。
 *
 * 顶栏主导航只保留三项：AI 助理 / 联系人 / 图谱。
 * - 「导入」降级为联系人页工具栏按钮（仍复用 /import 路由与 ImportWizard）。
 * - 「内观画像」降级为顶栏右侧用户菜单项（仍复用 /profile-qa 路由）。
 * - 「管理后台」仅 admin 可见，收入用户菜单（仍复用 /admin 路由）。
 * 所有既有路由保持不变，深链不受影响。
 */

export interface MainNavItem {
  id: string;
  label: string;
  /** 目标路由路径（不含 query） */
  path: string;
}

export const MAIN_NAV_ITEMS: MainNavItem[] = [
  { id: 'ai-assistant', label: 'AI 助理', path: '/' },
  { id: 'contacts', label: '联系人', path: '/contacts' },
  { id: 'graph', label: '图谱', path: '/graph' },
];

/**
 * 判断某个导航项在当前路径下是否激活。
 * - '/'（AI 助理）仅在首页精确匹配；「导入」(/import) 是从联系人页降级而来，
 *   停留在导入页时归属「联系人」项激活，避免三项全部失焦。
 * - 其余项做前缀匹配（含 '/contacts/:personId' 等子路由）。
 */
export function isNavPathActive(pathname: string, path: string): boolean {
  if (path === '/') {
    return pathname === '/';
  }
  if (path === '/contacts') {
    return pathname === '/contacts' || pathname.startsWith('/contacts/') || pathname.startsWith('/import');
  }
  return pathname === path || pathname.startsWith(`${path}/`);
}
