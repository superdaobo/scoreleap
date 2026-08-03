import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

const RISK_KEY = 'scoreleap-risk-accepted'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'library',
    component: () => import('../views/LibraryPage.vue'),
  },
  {
    path: '/doc/:docId',
    name: 'document',
    component: () => import('../views/DocumentPage.vue'),
  },
  {
    path: '/arrange/:seqId',
    name: 'arrange',
    component: () => import('../views/ArrangePage.vue'),
  },
  {
    path: '/test',
    name: 'test',
    component: () => import('../views/TestPage.vue'),
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('../views/SettingsPage.vue'),
  },
  {
    path: '/risk',
    name: 'risk',
    component: () => import('../views/RiskPage.vue'),
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})

/** 首次启动（未接受风险确认）时强制进入 /risk 确认页 */
router.beforeEach((to) => {
  const accepted = (() => {
    try {
      return localStorage.getItem(RISK_KEY) === 'true'
    } catch {
      return false
    }
  })()
  if (to.name !== 'risk' && !accepted) {
    return { name: 'risk' }
  }
  if (to.name === 'risk' && accepted) {
    return { name: 'library' }
  }
  return true
})
