<script setup lang="ts">
function start() {
  uni.showToast({ title: '开拼占位', icon: 'none' })
}
</script>

<template>
  <view class="page">
    <view class="hero" />
    <text class="title">稿件详情</text>
    <text class="meta">52×52 · 约 24 色 · 示意统计</text>
    <view class="btn" @click="start">开拼</view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx; }
.hero {
  height: 480rpx;
  border-radius: 24rpx;
  background: $brand-soft;
  margin-bottom: 28rpx;
}
.title { display: block; font-size: 36rpx; font-weight: 700; }
.meta { display: block; color: $ink-3; margin: 12rpx 0 40rpx; font-size: 26rpx; }
.btn {
  text-align: center;
  padding: 28rpx;
  background: $brand;
  color: #fff;
  border-radius: 999rpx;
  font-weight: 700;
}
</style>
