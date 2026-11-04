<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { creditsApi, errorOf, requestVirtualPayment, requestWxPayment, virtualPayUnavailable } from '../../api/http'
import { playRewardedVideo } from '../../utils/reward-ad'
import type { CreditOrder, CreditPack, LedgerItem } from '../../api/types'
import { useCreditsStore } from '../../stores/credits'
import { useUserStore } from '../../stores/user'
import { ensureLogin, showError, showTip, timeAgo } from '../../utils/ui'

const credits = useCreditsStore()
const user = useUserStore()
const packs = ref<CreditPack[]>([])
const ledger = ref<LedgerItem[]>([])
const paying = ref(false)
const watching = ref(false)

const fallback: CreditPack[] = [
  { code: 'small', name: '小包', credits: 10, price_cents: 600, price_label: '¥6', blurb: '约 0.6 元/次出稿', popular: false },
  { code: 'popular', name: '推荐', credits: 35, price_cents: 1800, price_label: '¥18', blurb: '多送 5', popular: true },
  { code: 'large', name: '大包', credits: 60, price_cents: 3000, price_label: '¥30', blurb: '多送 10', popular: false },
]

const payMode = ref<'mock' | 'wechat' | 'virtual' | 'misconfigured' | 'unknown'>('unknown')

async function load() {
  try {
    const data = await creditsApi.packs()
    packs.value = data.items
    if (
      data.pay_mode === 'mock' ||
      data.pay_mode === 'wechat' ||
      data.pay_mode === 'virtual' ||
      data.pay_mode === 'misconfigured'
    ) {
      payMode.value = data.pay_mode
    }
  } catch {
    packs.value = fallback
  }
  if (!user.isLoggedIn) return
  await credits.refresh()
  try {
    const data = await creditsApi.ledger()
    ledger.value = data.items
  } catch {
    ledger.value = []
  }
}

onShow(load)

async function watchAd() {
  if (!ensureLogin() || watching.value) return
  if (credits.adClaimed) {
    showTip('今日已领取广告奖励')
    return
  }
  watching.value = true
  try {
    await playRewardedVideo()
    const data = await credits.claimAdReward()
    showTip(`广告奖励 +${data.delta}`)
    await load()
  } catch (e) {
    const err = errorOf(e)
    if (err.error === 'already_claimed') {
      credits.adClaimed = true
      showTip(err.message || '今日已领取')
      return
    }
    showError(e, '广告奖励失败')
  } finally {
    watching.value = false
  }
}

function askPay(title: string, content: string, confirmText: string): Promise<boolean> {
  return new Promise((resolve) => {
    uni.showModal({
      title,
      content,
      confirmText,
      cancelText: '取消',
      confirmColor: '#FF6B5B',
      success: (res) => resolve(Boolean(res.confirm)),
      fail: () => resolve(false),
    })
  })
}

function tell(content: string) {
  if (content.length > 14) {
    uni.showModal({ title: '支付', content, showCancel: false })
    return
  }
  showTip(content)
}

function payErrText(e: unknown): string {
  if (typeof e === 'string') return e
  if (e && typeof e === 'object' && 'errMsg' in e) {
    const msg = (e as { errMsg?: unknown }).errMsg
    return typeof msg === 'string' ? msg : ''
  }
  return ''
}

function payErrCode(e: unknown): number | null {
  if (!e || typeof e !== 'object' || !('errCode' in e)) return null
  const code = (e as { errCode?: unknown }).errCode
  if (typeof code === 'number') return code
  if (typeof code === 'string' && code.trim()) {
    const parsed = Number(code)
    return Number.isFinite(parsed) ? parsed : null
  }
  return null
}

function virtualFailText(e: unknown): string {
  const code = payErrCode(e)
  if (code === -2) return '已取消支付'
  if (code === -15011) return '正式版不能使用沙箱支付'
  if (code === -15005 || code === -15007) return ''
  if (code === -15010 || code === -15014 || code === -15018) return '道具未生效，请稍后再试'
  if (code === -15013) return '价格与道具不一致'
  const msg = payErrText(e).toLowerCase()
  if (msg.includes('cancel') || msg.includes('取消')) return '已取消支付'
  if (msg.includes('unsupported') || msg.includes('不支持')) return '请升级微信后再支付'
  return '支付未完成，积分不会到账'
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function pollOrder(orderId: string): Promise<CreditOrder> {
  let last = await creditsApi.getOrder(orderId)
  for (let i = 0; i < 4 && last.status !== 'paid'; i += 1) {
    await sleep(800)
    last = await creditsApi.getOrder(orderId)
  }
  return last
}

async function buy(pack: CreditPack) {
  if (!ensureLogin() || paying.value) return
  if (payMode.value === 'misconfigured') {
    tell('支付暂不可用')
    return
  }
  paying.value = true
  try {
    const order = await creditsApi.createOrder(pack.code)
    if (order.mock) {
      const ok = await askPay(
        '开发示意充值',
        `${pack.price_label} · ${pack.credits} 积分\n当前未配置支付，确认后直接到账，不会产生真实扣款。`,
        '确认到账',
      )
      if (!ok) return
      const paid = await creditsApi.mockPay(order.order_id)
      credits.balance = paid.balance
      tell(paid.delta > 0 ? `到账 ${paid.delta} 积分` : '订单已支付')
      await load()
      return
    }

    if (order.virtual_payment) {
      const blocked = virtualPayUnavailable(order.virtual_payment.signData)
      if (blocked) {
        tell(blocked)
        return
      }
      const ok = await askPay(
        '微信支付',
        `支付 ${pack.price_label}，到账 ${pack.credits} 积分。取消或失败不会加积分。`,
        '去支付',
      )
      if (!ok) return
      try {
        await requestVirtualPayment(order.virtual_payment)
      } catch (e) {
        const code = payErrCode(e)
        if (code === -15005 || code === -15007) {
          try {
            await user.login()
            tell('登录已刷新，请再试一次')
          } catch {
            tell('登录已过期，请重新登录后再支付')
          }
          return
        }
        tell(virtualFailText(e))
        return
      }
      const result = await pollOrder(order.order_id)
      if (result.balance != null) credits.balance = result.balance
      if (result.status === 'paid') {
        tell('支付成功，积分已到账')
      } else {
        tell('支付已提交，到账可能稍有延迟，请稍后下拉刷新')
      }
      await load()
      return
    }

    if (!order.pay_params) {
      tell('支付参数缺失，请稍后重试')
      return
    }
    const ok = await askPay(
      '微信支付',
      `支付 ${pack.price_label}，到账 ${pack.credits} 积分。取消或失败不会加积分。`,
      '去支付',
    )
    if (!ok) return
    try {
      await requestWxPayment(order.pay_params)
    } catch (e) {
      const msg = payErrText(e).toLowerCase()
      tell(msg.includes('cancel') || msg.includes('取消') ? '已取消支付' : '支付未完成，积分不会到账')
      return
    }
    const result = await pollOrder(order.order_id)
    if (result.balance != null) credits.balance = result.balance
    if (result.status === 'paid') {
      tell('支付成功，积分已到账')
    } else {
      tell('支付已提交，到账可能稍有延迟，请稍后下拉刷新')
    }
    await load()
  } catch (e) {
    if ((errorOf(e).message || '').includes('重新登录')) {
      try {
        await user.login()
        tell('登录已刷新，请再试一次')
      } catch {
        showError(e, '下单失败')
      }
    } else {
      showError(e, '下单失败')
    }
  } finally {
    paying.value = false
  }
}
</script>

<template>
  <view class="page">
    <view class="balance">
      <text class="label">当前余额</text>
      <text class="num">{{ credits.balance }}</text>
      <text class="sub">出稿 −{{ credits.generateCost }} · 激励广告每日 +{{ credits.adReward }}</text>
    </view>

    <view class="rules">
      <text class="h">积分怎么用</text>
      <text>· 每次成功出稿消耗 {{ credits.generateCost }} 积分（含色块整理与拼豆色号），失败不扣</text>
      <text>· 看完一条激励广告得 {{ credits.adReward }} 积分，每天一次</text>
      <text>· 不足时也可以充值购买</text>
    </view>

    <view class="ad" :class="{ done: credits.adClaimed }" @click="watchAd">
      <text class="play">▶</text>
      <view class="ad-txt">
        <text class="h">{{ credits.adClaimed ? '今日已领取' : '看激励广告' }}</text>
        <text class="sub">{{ credits.adClaimed ? '明天再来' : `完整观看后得 ${credits.adReward} 积分` }}</text>
      </view>
      <text class="badge">+{{ credits.adReward }}</text>
    </view>

    <view class="sec-title">
      <text>充值档位</text>
      <text v-if="payMode === 'misconfigured'" class="tag">暂不可用</text>
      <text v-else-if="payMode === 'mock'" class="tag">开发示意</text>
    </view>
    <view class="tiers">
      <view
        v-for="pack in packs"
        :key="pack.code"
        class="tier"
        :class="{ popular: pack.popular }"
        @click="buy(pack)"
      >
        <text v-if="pack.popular" class="flag">推荐</text>
        <text class="credits">{{ pack.credits }} 积分</text>
        <text class="price">{{ pack.price_label }}</text>
        <text class="blurb">{{ pack.blurb }}</text>
      </view>
    </view>
    <text v-if="payMode === 'mock'" class="legal">积分仅用于出稿，不退不提现。当前是开发示意充值，不会产生真实扣款。</text>
    <text v-else-if="payMode === 'misconfigured'" class="legal">支付配置不完整，暂不能充值。</text>
    <text v-else-if="payMode === 'virtual'" class="legal">积分仅用于出稿，不退不提现。支付成功后到账。</text>
    <text v-else class="legal">积分仅用于出稿，不退不提现。支付由微信支付完成，成功后到账。</text>

    <view class="sec-title"><text>积分明细</text></view>
    <view v-if="!user.isLoggedIn" class="empty">登录后查看明细</view>
    <view v-else-if="ledger.length" class="ledger">
      <view v-for="item in ledger" :key="item.id" class="row">
        <view>
          <text class="h">{{ item.reason_label }}</text>
          <text class="sub">{{ timeAgo(item.created_at) }}</text>
        </view>
        <text class="delta" :class="{ plus: item.delta > 0 }">{{ item.delta > 0 ? `+${item.delta}` : item.delta }}</text>
      </view>
    </view>
    <view v-else class="empty">还没有积分记录</view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 48rpx; }
.balance {
  background: linear-gradient(135deg, $brand, $warm);
  border-radius: 28rpx; padding: 36rpx; color: #fff;
}
.label { font-size: 24rpx; opacity: 0.9; }
.num { display: block; font-size: 76rpx; font-weight: 800; line-height: 1.1; }
.sub { font-size: 24rpx; opacity: 0.92; }
.rules {
  margin-top: 20rpx; background: $card; border-radius: 24rpx; border: 1rpx solid $line;
  padding: 24rpx; display: flex; flex-direction: column; gap: 8rpx; color: $ink-2; font-size: 26rpx;
}
.h { font-weight: 750; color: $ink; }
.ad {
  margin-top: 20rpx; display: flex; align-items: center; gap: 16rpx;
  background: $card; border-radius: 24rpx; border: 1rpx solid $line; padding: 20rpx 24rpx;
}
.play { width: 64rpx; height: 64rpx; border-radius: 50%; background: $brand-soft; color: $brand-dark; text-align: center; line-height: 64rpx; }
.ad-txt { flex: 1; }
.ad.done { opacity: 0.72; }
.ad .sub { display: block; color: $ink-3; font-size: 22rpx; }
.badge { color: $brand-dark; font-weight: 800; }
.tag { font-size: 22rpx; color: $ink-3; font-weight: 500; }
.tiers { display: flex; gap: 12rpx; }
.tier {
  flex: 1; background: $card; border: 1rpx solid $line; border-radius: 24rpx; padding: 20rpx 12rpx; text-align: center;
}
.tier.popular { border-color: $brand; background: $brand-lighter; }
.flag { display: block; color: $brand; font-size: 20rpx; font-weight: 750; }
.credits { display: block; margin-top: 8rpx; font-weight: 800; font-size: 26rpx; }
.price { display: block; margin-top: 6rpx; color: $brand; font-size: 32rpx; font-weight: 800; }
.blurb { display: block; margin-top: 6rpx; color: $ink-3; font-size: 20rpx; }
.legal { display: block; margin-top: 16rpx; color: $ink-3; font-size: 22rpx; line-height: 1.5; }
.ledger { background: $card; border-radius: 24rpx; border: 1rpx solid $line; }
.row { display: flex; justify-content: space-between; align-items: center; padding: 22rpx 24rpx; border-bottom: 1rpx solid $line; }
.row:last-child { border-bottom: none; }
.delta { font-weight: 800; color: $ink; }
.plus { color: $success; }
</style>
