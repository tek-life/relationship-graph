import { describe, expect, it } from 'vitest';
import {
  analyzeSkillPackage,
  extractPackageFileList,
  findRootSkillMd,
  formatCharCount,
  isTextFilePath,
  normalizeRelPath,
  PACKAGE_LIMITS,
  ZIP_LIMITS,
} from './skillDoc';

describe('extractPackageFileList', () => {
  it('正常构造清单并按 relPath 字典序排序', () => {
    const list = extractPackageFileList([
      { relPath: 'references/notes.md', content: '# b' },
      { relPath: 'SKILL.md', content: '# a' },
      { relPath: 'assets/data.txt', content: 'x' },
    ]);
    expect(list.map((f) => f.relPath)).toEqual(['SKILL.md', 'assets/data.txt', 'references/notes.md']);
  });

  it('跳过目录条目（以 / 结尾或 content 为 null）', () => {
    const list = extractPackageFileList([
      { relPath: 'assets/', content: '' },
      { relPath: 'sub', content: null },
      { relPath: 'SKILL.md', content: '# ok' },
    ]);
    expect(list.map((f) => f.relPath)).toEqual(['SKILL.md']);
  });

  it('过滤二进制文件（非文本扩展名）', () => {
    const list = extractPackageFileList([
      { relPath: 'SKILL.md', content: '# ok' },
      { relPath: 'logo.png', content: 'fake-binary' },
      { relPath: 'archive.zip', content: 'fake' },
      { relPath: 'noext', content: 'x' },
    ]);
    expect(list.map((f) => f.relPath)).toEqual(['SKILL.md']);
  });

  it('归一化路径：去 ./ 前缀、反斜杠、折叠重复斜杠', () => {
    const list = extractPackageFileList([
      { relPath: './SKILL.md', content: 'a' },
      { relPath: 'assets\\\\notes.md', content: 'b' },
      { relPath: 'docs//guide.md', content: 'c' },
    ]);
    expect(list.map((f) => f.relPath)).toEqual(['SKILL.md', 'assets/notes.md', 'docs/guide.md']);
  });

  it('同名路径去重（保留后者）', () => {
    const list = extractPackageFileList([
      { relPath: 'SKILL.md', content: 'old' },
      { relPath: './SKILL.md', content: 'new' },
    ]);
    expect(list).toHaveLength(1);
    expect(list[0].content).toBe('new');
  });

  it('扩展名大小写不敏感', () => {
    const list = extractPackageFileList([
      { relPath: 'SKILL.MD', content: 'a' },
      { relPath: 'readme.Txt', content: 'b' },
    ]);
    expect(list).toHaveLength(2);
  });

  it('空输入返回空列表', () => {
    expect(extractPackageFileList([])).toEqual([]);
  });
});

describe('isTextFilePath', () => {
  it('识别常见文本扩展名', () => {
    expect(isTextFilePath('a.md')).toBe(true);
    expect(isTextFilePath('a.txt')).toBe(true);
    expect(isTextFilePath('a.yaml')).toBe(true);
    expect(isTextFilePath('dir/a.py')).toBe(true);
  });

  it('拒绝二进制扩展名', () => {
    expect(isTextFilePath('a.png')).toBe(false);
    expect(isTextFilePath('a.bin')).toBe(false);
    expect(isTextFilePath('a.pdf')).toBe(false);
  });
});

describe('normalizeRelPath', () => {
  it('去除 ./ 前缀并折叠重复斜杠', () => {
    expect(normalizeRelPath('./a/b.md')).toBe('a/b.md');
    expect(normalizeRelPath('a//b.md')).toBe('a/b.md');
    expect(normalizeRelPath('././a.md')).toBe('a.md');
  });

  it('反斜杠统一为 /', () => {
    expect(normalizeRelPath('a\\b\\c.md')).toBe('a/b/c.md');
  });
});

describe('findRootSkillMd（与后端口径对齐）', () => {
  it('根目录存在 SKILL.md 时直接命中', () => {
    expect(findRootSkillMd(['assets/a.txt', 'SKILL.md', 'docs/b.md'])).toBe('SKILL.md');
  });

  it('根目录缺失时命中嵌套一层目录', () => {
    expect(findRootSkillMd(['my-skill/SKILL.md', 'my-skill/refs/a.md'])).toBe('my-skill/SKILL.md');
  });

  it('文件名大小写不敏感', () => {
    expect(findRootSkillMd(['Skill.md'])).toBe('Skill.md');
    expect(findRootSkillMd(['pkg/skill.MD'])).toBe('pkg/skill.MD');
  });

  it('最浅层多个候选视为歧义，返回 null', () => {
    expect(findRootSkillMd(['z-pkg/SKILL.md', 'a-pkg/SKILL.md'])).toBeNull();
    expect(findRootSkillMd(['SKILL.md', 'Skill.md'])).toBeNull();
  });

  it('根 SKILL.md 优先于嵌套（最浅层唯一则命中）', () => {
    expect(findRootSkillMd(['pkg/SKILL.md', 'SKILL.md'])).toBe('SKILL.md');
  });

  it('任意深度取最浅层（不再限一层）', () => {
    expect(findRootSkillMd(['a/b/SKILL.md'])).toBe('a/b/SKILL.md');
    expect(findRootSkillMd(['a/b/SKILL.md', 'a/b/c/SKILL.md'])).toBe('a/b/SKILL.md');
  });

  it('同层歧义但更深一层唯一时，仍视为歧义返回 null', () => {
    expect(findRootSkillMd(['x/SKILL.md', 'y/SKILL.md', 'x/deep/SKILL.md'])).toBeNull();
  });

  it('完全没有 SKILL.md 时返回 null', () => {
    expect(findRootSkillMd(['readme.md', 'docs/a.txt'])).toBeNull();
    expect(findRootSkillMd([])).toBeNull();
  });
});

describe('analyzeSkillPackage', () => {
  const skillMd = '---\nname: demo\ndescription: 测试\n---\n\n正文';

  it('正常包：统计文件数/总字符且无阻断问题', () => {
    const preview = analyzeSkillPackage([
      { relPath: 'SKILL.md', content: skillMd },
      { relPath: 'refs/a.md', content: '12345' },
    ]);
    expect(preview.fileCount).toBe(2);
    expect(preview.totalChars).toBe(skillMd.length + 5);
    expect(preview.hasRootSkillMd).toBe(true);
    expect(preview.rootSkillMdRelPath).toBe('SKILL.md');
    expect(preview.overFileLimit).toBe(false);
    expect(preview.overCharLimit).toBe(false);
    expect(preview.oversizedFiles).toEqual([]);
    expect(preview.hasBlockingIssue).toBe(false);
  });

  it('无 SKILL.md 时标记阻断', () => {
    const preview = analyzeSkillPackage([{ relPath: 'readme.md', content: 'x' }]);
    expect(preview.hasRootSkillMd).toBe(false);
    expect(preview.hasBlockingIssue).toBe(true);
  });

  it('超文件数上限预检', () => {
    const files = Array.from({ length: PACKAGE_LIMITS.maxFiles + 1 }, (_, i) => ({
      relPath: i === 0 ? 'SKILL.md' : `f${i}.md`,
      content: 'x',
    }));
    const preview = analyzeSkillPackage(files);
    expect(preview.overFileLimit).toBe(true);
    expect(preview.hasBlockingIssue).toBe(true);
  });

  it('恰好等于文件数上限不算超限', () => {
    const files = Array.from({ length: PACKAGE_LIMITS.maxFiles }, (_, i) => ({
      relPath: i === 0 ? 'SKILL.md' : `f${i}.md`,
      content: 'x',
    }));
    expect(analyzeSkillPackage(files).overFileLimit).toBe(false);
  });

  it('超总字符上限预检', () => {
    const preview = analyzeSkillPackage([
      { relPath: 'SKILL.md', content: 'x'.repeat(PACKAGE_LIMITS.maxTotalChars + 1) },
    ]);
    expect(preview.overCharLimit).toBe(true);
    expect(preview.hasBlockingIssue).toBe(true);
  });

  it('单文件超限被列出', () => {
    const preview = analyzeSkillPackage([
      { relPath: 'SKILL.md', content: skillMd },
      { relPath: 'big.md', content: 'x'.repeat(PACKAGE_LIMITS.maxSingleFileChars + 1) },
    ]);
    expect(preview.oversizedFiles).toEqual(['big.md']);
    expect(preview.hasBlockingIssue).toBe(true);
  });

  it('空清单标记为阻断（缺少根 SKILL.md）', () => {
    const preview = analyzeSkillPackage([]);
    expect(preview.fileCount).toBe(0);
    expect(preview.totalChars).toBe(0);
    expect(preview.hasBlockingIssue).toBe(true);
  });

  it('frontmatter 缺少 name/description 时预检阻断', () => {
    const preview = analyzeSkillPackage([{ relPath: 'SKILL.md', content: '---\nname: demo\n---\n\n正文' }]);
    expect(preview.frontmatterError).toBe('frontmatter 缺少必填字段 description');
    expect(preview.hasBlockingIssue).toBe(true);

    const noFm = analyzeSkillPackage([{ relPath: 'SKILL.md', content: '无 frontmatter 正文' }]);
    expect(noFm.frontmatterError).toBe('SKILL 文档缺少 frontmatter 头部（以 --- 开始）');
    expect(noFm.hasBlockingIssue).toBe(true);
  });

  it('frontmatter 完整时预检通过', () => {
    const preview = analyzeSkillPackage([{ relPath: 'SKILL.md', content: skillMd }]);
    expect(preview.frontmatterError).toBeNull();
    expect(preview.hasBlockingIssue).toBe(false);
  });

  it('最浅层多个根 SKILL.md 候选标记歧义并阻断', () => {
    const preview = analyzeSkillPackage([
      { relPath: 'a-pkg/SKILL.md', content: skillMd },
      { relPath: 'b-pkg/SKILL.md', content: skillMd },
    ]);
    expect(preview.rootSkillMdAmbiguous).toBe(true);
    expect(preview.hasRootSkillMd).toBe(false);
    expect(preview.hasBlockingIssue).toBe(true);
  });
});

describe('ZIP_LIMITS', () => {
  it('与后端口径协调：zip 上限 20MB、解压上限 8MB', () => {
    expect(ZIP_LIMITS.maxZipBytes).toBe(20 * 1024 * 1024);
    expect(ZIP_LIMITS.maxUncompressedBytes).toBe(8 * 1024 * 1024);
  });
});

describe('formatCharCount', () => {
  it('千位逗号格式化', () => {
    expect(formatCharCount(1234567)).toBe('1,234,567');
    expect(formatCharCount(0)).toBe('0');
  });
});
