<script setup lang="ts">
import CreditPill from '../../components/CreditPill.vue'
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import { useCreditsStore } from '../../stores/credits'

const credits = useCreditsStore()

function goParams() {
  if (!credits.canGenerate) {
    credits.openEmptySheet()
    return
  }
  uni.navigateTo({ url: '/pages/create/params' })
}
</script>

<template>
  <view class="page">
    <view class="top">
      <text class="title">出稿</text>
      <CreditPill compact />
    </view>
    <view class="upload" @click="goParams">
      <text class="big">📷</text>
      <text class="hint">拍照或从相册选择</text>
      <text class="sub">生成色号稿 · 消耗 {{ credits.generateCost }} 积分</text>
    </view>
    <view class="tips">
      <text class="t">小贴士</text>
      <text class="d">光线均匀、主体清晰，出稿更准。日送 {{ credits.dailyGrant }} 积分。</text>
    </view>
    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 120rpx; }
.top { display: flex; justify-content: space-between; align-items: center; margin-bottom: 28rpx; }
.title { font-size: 36rpx; font-weight: 700; }
.upload {
  background: $card;
  border: 2rpx dashed $brand;
  border-radius: 32rpx;
  padding: 80rpx 40rpx;
  text-align: center;
  margin-bottom: 32rpx;
}
.big { font-size: 64rpx; display: block; margin-bottom: 16rpx; }
.hint { display: block; font-size: 30rpx; font-weight: 650; color: $ink; }
.sub { display: block; margin-top: 8rpx; font-size: 24rpx; color: $ink-3; }
.tips {
  background: $brand-lighter;
  border-radius: 20rpx;
  padding: 28rpx;
}
.t { display: block; font-weight: 650; margin-bottom: 8rpx; color: $brand-dark; }
.d { font-size: 24rpx; color: $ink-2; }
</style>
