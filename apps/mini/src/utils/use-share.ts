import { onShow } from '@dcloudio/uni-app'
import {
  HOME_PATH,
  buildShareAppMessage,
  buildShareTimeline,
  currentPagePath,
  type PageShare,
  type ShareAppMessage,
  type ShareTimeline,
} from './share'

export type ShareSource = PageShare | (() => PageShare)

type Hook<T> = (callback: () => T) => void

function readInput(source: ShareSource): PageShare {
  try {
    return typeof source === 'function' ? source() : source
  } catch {
    return {}
  }
}

function enableCapsuleShare() {
  // #ifdef MP-WEIXIN
  try {
    uni.showShareMenu({
      menus: ['shareAppMessage', 'shareTimeline'],
      fail() {
        uni.showShareMenu({ menus: ['shareAppMessage'] })
      },
    })
  } catch {
    // 旧基础库没有朋友圈入口时，转发仍由 onShareAppMessage 打开。
  }
  // #endif
}

/**
 * 微信右上角「转发给朋友」「分享到朋友圈」「复制链接」默认是灰的，
 * 页面注册了 onShareAppMessage 才会亮起转发和复制链接；朋友圈还要 onShareTimeline，
 * 并在 showShareMenu 的 menus 里带上 shareTimeline。
 *
 * 这个版本的 uni-app 只扫描页面源码里的这三个名字来向微信注册。
 * 所以每个页面都要把生命周期函数传进来，不能只在本文件里调用。
 */
export function usePageShare(
  onShareAppMessage: Hook<ShareAppMessage>,
  onShareTimeline: Hook<ShareTimeline>,
  onCopyUrl: Hook<{ query?: string }>,
  source: ShareSource = {},
) {
  const resolve = () => {
    let current = HOME_PATH
    try {
      current = currentPagePath()
    } catch {
      current = HOME_PATH
    }
    const input = readInput(source)
    return {
      message: buildShareAppMessage(input, current),
      timeline: buildShareTimeline(input, current),
    }
  }

  enableCapsuleShare()
  onShow(() => enableCapsuleShare())
  onShareAppMessage(() => resolve().message)
  onShareTimeline(() => resolve().timeline)
  onCopyUrl(() => {
    const query = resolve().timeline.query
    return query ? { query } : {}
  })
}
