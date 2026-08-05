// 本地化字体（离线可用）：标题 serif / 正文 sans / 等宽 + Material Symbols 图标
import '@fontsource/playfair-display/400.css'
import '@fontsource/playfair-display/600.css'
import '@fontsource/playfair-display/700.css'
import '@fontsource/geist/400.css'
import '@fontsource/geist/500.css'
import '@fontsource/geist/600.css'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/500.css'
import 'material-symbols/outlined.css'

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { router } from './router'
import { usePlaybackStore } from './stores/playbackStore'
import './style.css'

const app = createApp(App)

app.use(createPinia())
app.use(router)
app.mount('#app')

// 订阅播放事件（非 Tauri 环境——纯浏览器预览——下静默失败）
const playback = usePlaybackStore()
void playback.setupEventListeners().catch(() => {
  // 忽略：浏览器环境没有 Tauri 事件系统
})
