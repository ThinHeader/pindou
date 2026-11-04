import { errorOf } from '../api/http'
import { useUserStore } from '../stores/user'

export function showError(e: unknown, fallback = '操作失败') {
  const err = errorOf(e)
  if (err.silent) return
  const msg = err.message || fallback
  if (msg.length > 16) {
    uni.showModal({ title: '提示', content: msg, showCancel: false })
  } else {
    uni.showToast({ title: msg, icon: 'none' })
  }
}

export function showTip(title: string) {
  uni.showToast({ title, icon: 'none' })
}

export function askConfirm(title: string, content: string): Promise<boolean> {
  return new Promise((resolve) => {
    uni.showModal({
      title,
      content,
      cancelText: '取消',
      confirmText: '删除',
      confirmColor: '#FF6B5B',
      success: (res) => resolve(Boolean(res.confirm)),
      fail: () => resolve(false),
    })
  })
}

/** 点取消返回 null；点确定返回去掉首尾空格后的内容（可能是空字符串）。 */
export function askText(title: string, content = '', placeholder = ''): Promise<string | null> {
  return new Promise((resolve) => {
    uni.showModal({
      title,
      editable: true,
      placeholderText: placeholder,
      cancelText: '取消',
      confirmText: '确定',
      content,
      success: (res) => {
        if (!res.confirm) {
          resolve(null)
          return
        }
        resolve(String((res as { content?: string }).content || '').trim())
      },
      fail: () => resolve(null),
    })
  })
}

export function checkedDraftTitle(raw: string): string | null {
  const title = raw.trim()
  if (!title) {
    showError({ message: '请填写稿件名称' })
    return null
  }
  if ([...title].length > 40) {
    showError({ message: '稿件名称请控制在 40 字以内' })
    return null
  }
  return title
}

export function ensureLogin(): boolean {
  const user = useUserStore()
  if (user.isLoggedIn) return true
  uni.navigateTo({ url: '/pages/login/index' })
  return false
}

export function timeAgo(iso?: string): string {
  if (!iso) return ''
  const t = new Date(iso).getTime()
  if (Number.isNaN(t)) return ''
  const seconds = Math.max(0, (Date.now() - t) / 1000)
  if (seconds < 60) return '刚刚'
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`
  const days = Math.floor(seconds / 86400)
  if (days === 1) return '昨天'
  if (days < 30) return `${days} 天前`
  return iso.slice(0, 10)
}

export function initialOf(name?: string | null): string {
  const text = (name || '豆').trim()
  return text.slice(0, 1) || '豆'
}

export function boardLabel(size: number, colors?: number): string {
  if (colors) return `${size}×${size} · ${colors}色`
  return `${size}×${size}`
}
