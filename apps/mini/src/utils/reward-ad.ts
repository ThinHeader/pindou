type CloseRes = { isEnded?: boolean }
type AdError = { errMsg?: string; errCode?: number }

interface RewardAd {
  show: () => Promise<unknown>
  load: () => Promise<unknown>
  onClose: (cb: (res: CloseRes) => void) => void
  offClose: (cb: (res: CloseRes) => void) => void
  onError: (cb: (err: AdError) => void) => void
  offError: (cb: (err: AdError) => void) => void
}

let cached: RewardAd | null = null
let cachedUnit = ''

export function rewardAdUnitId(): string {
  return ((import.meta.env.VITE_REWARD_AD_UNIT_ID as string | undefined) || '').trim()
}

/** 播完激励视频才 resolve。中途关掉、没配广告位或当前环境不支持都会 reject，此时不要请求发积分。 */
export function playRewardedVideo(): Promise<void> {
  const adUnitId = rewardAdUnitId()
  if (!adUnitId) {
    return Promise.reject({ message: '未配置激励广告位' })
  }
  const create = uni.createRewardedVideoAd
  if (typeof create !== 'function') {
    return Promise.reject({ message: '请在微信小程序里观看激励广告' })
  }
  if (!cached || cachedUnit !== adUnitId) {
    cached = create({ adUnitId }) as RewardAd
    cachedUnit = adUnitId
  }
  const ad = cached
  return new Promise((resolve, reject) => {
    let settled = false
    const finish = (err?: { message: string }) => {
      if (settled) return
      settled = true
      ad.offClose(onClose)
      ad.offError(onError)
      if (err) reject(err)
      else resolve()
    }
    const onClose = (res: CloseRes) => {
      if (res?.isEnded) finish()
      else finish({ message: '需要看完广告才能领取积分' })
    }
    const onError = (err: AdError) => {
      finish({ message: err?.errMsg || '广告加载失败，请稍后再试' })
    }
    ad.onClose(onClose)
    ad.onError(onError)
    ad.show().catch(() => {
      ad.load()
        .then(() => ad.show())
        .catch(() => finish({ message: '广告加载失败，请稍后再试' }))
    })
  })
}
