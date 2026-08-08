/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      /* ===== UX P0-1 设计令牌 =====
       * 颜色令牌映射 src/index.css 三套主题（light/dark/high-contrast）的 CSS 变量，
       * 供 bg-primary / text-muted / border-line / bg-success 等语义类使用。
       * 约定：rounded-full 只留给头像 / badge / 分段滑块，
       *       其余控件用 rounded-control / rounded-card / rounded-pop。 */
      colors: {
        // 背景层级
        primary: "var(--bg-primary)",
        secondary: "var(--bg-secondary)",
        card: "var(--bg-card)",
        surface: "var(--surface-hover)",
        // 文字层级
        "text-primary": "var(--text-primary)",
        "text-secondary": "var(--text-secondary)",
        muted: "var(--text-muted)",
        // 边框
        line: "var(--border-color)",
        // 强调 / 危险 / 语义状态色
        accent: {
          DEFAULT: "var(--accent-color)",
          hover: "var(--accent-hover)",
          light: "var(--accent-light)",
        },
        danger: {
          DEFAULT: "var(--danger-color)",
          hover: "var(--danger-hover)",
          // UX P0-2：语义软背景（错误/成功/警示的浅底），三主题各自定义
          light: "var(--danger-light)",
        },
        success: { DEFAULT: "var(--success)", light: "var(--success-light)" },
        warning: { DEFAULT: "var(--warning)", light: "var(--warning-light)" },
      },
      // 圆角三档：控件 8px / 卡片 12px / 弹层 16px
      borderRadius: {
        control: "8px",
        card: "12px",
        pop: "16px",
      },
      // 阴影两档：卡片 / 弹层（取主题 --shadow-color 保证深浅主题协调）
      boxShadow: {
        card: "0 1px 3px var(--shadow-color), 0 1px 2px var(--shadow-color)",
        pop: "0 10px 30px var(--shadow-color), 0 4px 10px var(--shadow-color)",
      },
      // UX P2-11 动效统一：交互态过渡默认 150ms ease-out（与 index.css 的
      // --motion-duration / --motion-ease 对齐）；transition / transition-colors 等
      // 不带显式 duration/ease 的工具类自动套用该默认值。
      transitionDuration: {
        DEFAULT: "150ms",
      },
      transitionTimingFunction: {
        DEFAULT: "ease-out",
      },
      // 字体栈：保留 Inter 与西文系统回退，追加中文字体
      fontFamily: {
        sans: [
          "Inter",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          '"Segoe UI"',
          '"PingFang SC"',
          '"Hiragino Sans GB"',
          '"Microsoft YaHei"',
          '"Noto Sans SC"',
          "sans-serif",
        ],
      },
      // 字号型阶（约定：正文 14 / 聊天 15 / 辅助 13）
      fontSize: {
        caption: ["12px", { lineHeight: "18px" }],
        auxiliary: ["13px", { lineHeight: "20px" }],
        body: ["14px", { lineHeight: "22px" }],
        chat: ["15px", { lineHeight: "24px" }],
        lead: ["16px", { lineHeight: "24px" }],
        title: ["20px", { lineHeight: "28px" }],
        display: ["24px", { lineHeight: "32px" }],
      },
    },
  },
  plugins: [],
}
