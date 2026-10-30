<script setup lang="ts">
function copyMake() {
  uni.switchTab({ url: '/pages/create/index' })
}
</script>

<template>
  <view class="page">
    <view class="hero" />
    <text class="title">作品详情</text>
    <text class="meta">点赞 · 收藏 · 评论（占位）</text>
    <view class="btn" @click="copyMake">照着做</view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx; }
.hero {
  height: 480rpx;
  border-radius: 24rpx;
  background: linear-gradient(160deg, $warm-soft, $brand-soft);
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
