<script setup lang="ts">
import { useCreditsStore } from '../stores/credits'

const credits = useCreditsStore()

function watchAd() {
  credits.claimAdReward()
  uni.showToast({ title: '广告奖励 +1', icon: 'success' })
}
function goRecharge() {
  credits.closeEmptySheet()
  uni.navigateTo({ url: '/pages/credits/index' })
}
</script>

<template>
  <view v-if="credits.showEmptySheet" class="mask" @click="credits.closeEmptySheet()">
    <view class="sheet" @click.stop>
      <text class="title">积分不足</text>
      <text class="desc">出稿需消耗 {{ credits.generateCost }} 积分，当前余额 {{ credits.balance }}</text>
      <view class="btn primary" @click="watchAd">看广告 +{{ credits.adReward }}</view>
      <view class="btn soft" @click="goRecharge">去充值</view>
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
  padding: 48rpx 40rpx calc(48rpx + env(safe-area-inset-bottom));
  display: flex;
  flex-direction: column;
  gap: 20rpx;
}
.title {
  font-size: 36rpx;
  font-weight: 700;
  color: $ink;
}
.desc {
  font-size: 26rpx;
  color: $ink-2;
  margin-bottom: 12rpx;
}
.btn {
  text-align: center;
  padding: 24rpx;
  border-radius: 999rpx;
  font-weight: 600;
  font-size: 28rpx;
}
.primary {
  background: $brand;
  color: #fff;
}
.soft {
  background: $brand-soft;
  color: $brand-dark;
}
.ghost {
  color: $ink-3;
}
</style>
