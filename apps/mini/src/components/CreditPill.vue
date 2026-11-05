<script setup lang="ts">
import { useCreditsStore } from '../stores/credits'
import { ensureLogin } from '../utils/ui'

defineProps<{ compact?: boolean }>()
const credits = useCreditsStore()

function goCredits() {
  if (!ensureLogin()) return
  uni.navigateTo({ url: '/pages/credits/index' })
}
</script>

<template>
  <view class="pill" @click="goCredits">
    <text class="dot">●</text>
    <text class="txt">{{ compact ? credits.balance : `积分 ${credits.balance}` }}</text>
  </view>
</template>

<style scoped lang="scss">
.pill {
  display: inline-flex;
  align-items: center;
  gap: 8rpx;
  padding: 8rpx 20rpx;
  background: $brand-soft;
  color: $brand-dark;
  border-radius: 999rpx;
  font-size: 24rpx;
  font-weight: 600;
}
.dot { color: $brand; font-size: 18rpx; }
</style>
