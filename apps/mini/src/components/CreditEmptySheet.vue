<script setup lang="ts">
import { useCreditsStore } from '../stores/credits'
import { showError, showTip } from '../utils/ui'

const credits = useCreditsStore()

async function watchAd() {
  try {
    await credits.claimAdReward()
    showTip(`广告奖励 +${credits.adReward}`)
  } catch (e) {
    showError(e, '广告奖励失败')
  }
}
function goRecharge() {
  credits.closeEmptySheet()
  uni.navigateTo({ url: '/pages/credits/index' })
}
</script>

<template>
  <view v-if="credits.showEmptySheet" class="mask" @click="credits.closeEmptySheet()">
    <view class="sheet" @click.stop>
      <view class="handle" />
      <text class="title">积分不足，无法出稿</text>
      <text class="desc">
        当前余额 {{ credits.balance }} · 本次出稿一次性扣除 {{ credits.generateCost }} 积分（含智能去背 + 卡通 + 拼豆量化）。可看激励广告或充值后再试。
      </text>
      <view class="btn primary" @click="watchAd">看广告得 {{ credits.adReward }} 积分</view>
      <view class="btn soft" @click="goRecharge">充值积分</view>
      <view class="btn ghost" @click="credits.closeEmptySheet()">稍后再说</view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.mask {
  position: fixed;
  inset: 0;
  background: rgba(44, 42, 41, 0.45);
  display: flex;
  align-items: flex-end;
  z-index: 1000;
}
.sheet {
  width: 100%;
  background: $card;
  border-radius: 32rpx 32rpx 0 0;
  padding: 28rpx 40rpx calc(40rpx + env(safe-area-inset-bottom));
  display: flex;
  flex-direction: column;
  gap: 20rpx;
}
.handle {
  width: 72rpx;
  height: 8rpx;
  border-radius: 999rpx;
  background: $line;
  align-self: center;
  margin-bottom: 8rpx;
}
.title { font-size: 36rpx; font-weight: 700; color: $ink; }
.desc { font-size: 26rpx; color: $ink-2; line-height: 1.5; margin-bottom: 8rpx; }
.btn { text-align: center; padding: 24rpx; border-radius: 999rpx; font-weight: 650; font-size: 28rpx; }
.primary { background: $brand; color: #fff; }
.soft { background: $brand-soft; color: $brand-dark; }
.ghost { color: $ink-3; }
</style>
