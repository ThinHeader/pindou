import { defineConfig } from 'vite'
// 安装依赖后启用：
// import uni from '@dcloudio/vite-plugin-uni'

export default defineConfig({
  // plugins: [uni()],
  server: {
    port: 5173,
    host: true,
  },
})
