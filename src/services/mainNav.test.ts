import { describe, expect, it } from 'vitest';
import { MAIN_NAV_ITEMS, isNavPathActive } from './mainNav';

describe('MAIN_NAV_ITEMS', () => {
  it('主导航收敛为三项：AI 助理 / 联系人 / 图谱', () => {
    expect(MAIN_NAV_ITEMS.map((item) => item.label)).toEqual(['AI 助理', '联系人', '图谱']);
  });

  it('三项路径与既有路由保持一致', () => {
    expect(MAIN_NAV_ITEMS.map((item) => item.path)).toEqual(['/', '/contacts', '/graph']);
  });

  it('每项 id 唯一', () => {
    const ids = MAIN_NAV_ITEMS.map((item) => item.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe('isNavPathActive', () => {
  it('AI 助理仅在首页精确激活', () => {
    expect(isNavPathActive('/', '/')).toBe(true);
    expect(isNavPathActive('/contacts', '/')).toBe(false);
    expect(isNavPathActive('/profile-qa', '/')).toBe(false);
  });

  it('联系人页及其子路由激活联系人项', () => {
    expect(isNavPathActive('/contacts', '/contacts')).toBe(true);
    expect(isNavPathActive('/contacts/p-123', '/contacts')).toBe(true);
    expect(isNavPathActive('/graph', '/contacts')).toBe(false);
  });

  it('导入页（降级入口）归属联系人项激活', () => {
    expect(isNavPathActive('/import', '/contacts')).toBe(true);
    expect(isNavPathActive('/import', '/')).toBe(false);
  });

  it('图谱页激活图谱项', () => {
    expect(isNavPathActive('/graph', '/graph')).toBe(true);
    expect(isNavPathActive('/graph/extra', '/graph')).toBe(true);
    expect(isNavPathActive('/contacts', '/graph')).toBe(false);
  });

  it('内观画像与管理后台不激活任何主导航项', () => {
    for (const item of MAIN_NAV_ITEMS) {
      expect(isNavPathActive('/profile-qa', item.path)).toBe(false);
      expect(isNavPathActive('/admin', item.path)).toBe(false);
    }
  });
});
