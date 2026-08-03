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
