<script setup lang="ts">
import { useCreditsStore } from '../../stores/credits'

const credits = useCreditsStore()

const packs = [
  { name: '小包', credits: 10, price: '¥6' },
  { name: '中包', credits: 30, price: '¥15' },
  { name: '大包', credits: 68, price: '¥30' },
]

function buy(name: string) {
  uni.showToast({ title: `${name} 支付占位`, icon: 'none' })
}
</script>

<template>
  <view class="page">
    <view class="balance">
      <text class="label">当前余额</text>
      <text class="num">{{ credits.balance }}</text>
      <text class="hint">日送 {{ credits.dailyGrant }} · 出稿 −{{ credits.generateCost }}</text>
    </view>

    <view class="ad" @click="credits.claimAdReward()">看激励广告 +{{ credits.adReward }}</view>

    <text class="sec">充值档位（示意，未接支付）</text>
    <view v-for="p in packs" :key="p.name" class="pack" @click="buy(p.name)">
      <view>
        <text class="n">{{ p.name }}</text>
        <text class="c">{{ p.credits }} 积分</text>
      </view>
      <text class="price">{{ p.price }}</text>
    </view>

    <text class="legal">充值仅为示意档位，正式上线前需完成支付合规与小程序类目审核。</text>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx; }
.balance {
  background: linear-gradient(135deg, $brand, $warm);
  border-radius: 28rpx;
  padding: 40rpx;
  color: #fff;
  margin-bottom: 28rpx;
}
.label { font-size: 24rpx; opacity: 0.9; }
.num { display: block; font-size: 72rpx; font-weight: 800; margin: 8rpx 0; }
.hint { font-size: 24rpx; opacity: 0.9; }
.ad {
  text-align: center;
  padding: 24rpx;
  background: $brand-soft;
  color: $brand-dark;
  border-radius: 999rpx;
  font-weight: 650;
  margin-bottom: 36rpx;
}
.sec { display: block; font-weight: 650; margin-bottom: 16rpx; }
.pack {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 20rpx;
  padding: 28rpx;
  margin-bottom: 16rpx;
}
.n { display: block; font-weight: 650; }
.c { font-size: 24rpx; color: $ink-3; }
.price { color: $brand; font-weight: 700; font-size: 32rpx; }
.legal { display: block; margin-top: 32rpx; font-size: 22rpx; color: $ink-3; line-height: 1.5; }
</style>
