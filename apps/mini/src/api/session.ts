type RefreshFn = () => Promise<boolean>

let refreshImpl: RefreshFn | null = null
let onExpired: (() => void) | null = null
let inflight: Promise<boolean> | null = null
let expiredLatch = false
let expiredTimer: ReturnType<typeof setTimeout> | null = null

export function bindSessionRefresh(refresh: RefreshFn, expired: () => void) {
  refreshImpl = refresh
  onExpired = expired
}

export function sessionRefreshBound(): boolean {
  return refreshImpl != null
}

/** 并发的 onShow 和 401 共用同一次 wx.login。 */
export function refreshSessionSingleFlight(): Promise<boolean> {
  if (!refreshImpl) return Promise.resolve(false)
  if (!inflight) {
    const refresh = refreshImpl
    let run: Promise<boolean>
    try {
      run = Promise.resolve(refresh())
    } catch {
      run = Promise.resolve(false)
    }
    inflight = run
      .then((ok) => Boolean(ok))
      .catch(() => false)
      .finally(() => {
        inflight = null
      })
  }
  return inflight
}

export function notifySessionExpired() {
  if (expiredLatch) return
  expiredLatch = true
  try {
    onExpired?.()
  } finally {
    if (expiredTimer != null) clearTimeout(expiredTimer)
    expiredTimer = setTimeout(() => {
      expiredLatch = false
      expiredTimer = null
    }, 3000)
    const handle = expiredTimer as unknown as { unref?: () => void }
    handle.unref?.()
  }
}

export function resetSessionRefreshForTests() {
  if (expiredTimer != null) clearTimeout(expiredTimer)
  refreshImpl = null
  onExpired = null
  inflight = null
  expiredLatch = false
  expiredTimer = null
}
