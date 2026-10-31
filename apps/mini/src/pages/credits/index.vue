<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { creditsApi } from '../../api/http'
import type { CreditPack, LedgerItem } from '../../api/types'
import { useCreditsStore } from '../../stores/credits'
import { useUserStore } from '../../stores/user'
import { ensureLogin, showError, showTip, timeAgo } from '../../utils/ui'

const credits = useCreditsStore()
const user = useUserStore()
const packs = ref<CreditPack[]>([])
const ledger = ref<LedgerItem[]>([])
const paying = ref(false)

const fallback: CreditPack[] = [
  { code: 'small', name: '小包', credits: 10, price_cents: 600, price_label: '¥6', blurb: '约 0.6 元/次出稿', popular: false },
  { code: 'popular', name: '推荐', credits: 35, price_cents: 1800, price_label: '¥18', blurb: '多送 5 · 示意', popular: true },
  { code: 'large', name: '大包', credits: 60, price_cents: 3000, price_label: '¥30', blurb: '多送 10 · 示意', popular: false },
]

async function load() {
  try {
    const data = await creditsApi.packs()
    packs.value = data.items
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
  if (!ensureLogin()) return
  try {
    const data = await credits.claimAdReward()
    showTip(`广告奖励 +${data.delta}`)
    await load()
  } catch (e) {
    showError(e, '广告奖励失败')
  }
}

function buy(pack: CreditPack) {
  if (!ensureLogin() || paying.value) return
  uni.showModal({
    title: `${pack.price_label} · ${pack.credits} 积分`,
    content: '这是示意充值，确认后会直接到账，不会产生真实扣款。',
    success: async (res) => {
      if (!res.confirm) return
      paying.value = true
      try {
        const order = await creditsApi.createOrder(pack.code)
        const paid = await creditsApi.mockPay(order.order_id)
        credits.balance = paid.balance
        showTip(paid.delta > 0 ? `到账 ${paid.delta} 积分` : '订单已支付')
        await load()
      } catch (e) {
        showError(e, '下单失败')
      } finally {
        paying.value = false
      }
    },
  })
}
</script>

<template>
  <view class="page">
    <view class="balance">
      <text class="label">当前余额</text>
      <text class="num">{{ credits.balance }}</text>
      <text class="sub">日送 {{ credits.dailyGrant }} · 出稿 −{{ credits.generateCost }} · {{ credits.dailyClaimed ? '今日已领' : '今日未领' }}</text>
    </view>

    <view class="rules">
      <text class="h">积分怎么用</text>
      <text>· 每日登录送 {{ credits.dailyGrant }} 积分（每天一次）</text>
      <text>· 每次成功出稿消耗 {{ credits.generateCost }} 积分（含去背+卡通+拼豆），失败不扣</text>
      <text>· 不足时可看广告 +{{ credits.adReward }}，或充值购买</text>
    </view>

    <view class="ad" @click="watchAd">
      <text class="play">▶</text>
      <view class="ad-txt">
        <text class="h">看激励广告</text>
        <text class="sub">完整观看后得 {{ credits.adReward }} 积分</text>
      </view>
      <text class="badge">+{{ credits.adReward }}</text>
    </view>

    <view class="sec-title"><text>充值档位</text><text class="tag">示意</text></view>
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
    <text class="legal">积分仅用于出稿，不退不提现。充值示意，非正式支付。</text>

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
