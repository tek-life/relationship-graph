/// <reference types="vite/client" />

// mammoth 浏览器构建产物无自带类型声明，此处按需声明用到的 API
declare module 'mammoth/mammoth.browser' {
  export function extractRawText(input: {
    arrayBuffer: ArrayBuffer;
  }): Promise<{ value: string; messages: unknown[] }>;
}
