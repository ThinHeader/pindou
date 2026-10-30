<script setup lang="ts">
function saveLibrary() {
  uni.showToast({ title: '已存入稿件库（示意）', icon: 'success' })
  setTimeout(() => uni.switchTab({ url: '/pages/library/index' }), 500)
}
</script>

<template>
  <view class="page">
    <view class="preview" />
    <text class="hint">色号稿预览占位 · 后续接渲染结果</text>
    <view class="actions">
      <view class="btn soft" @click="uni.showToast({ title: '分享占位', icon: 'none' })">分享</view>
      <view class="btn primary" @click="saveLibrary">存入稿件库</view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx; }
.preview {
  height: 560rpx;
  border-radius: 24rpx;
  background:
    repeating-linear-gradient(0deg, transparent, transparent 10rpx, rgba(255,107,91,0.08) 10rpx, rgba(255,107,91,0.08) 12rpx),
    repeating-linear-gradient(90deg, transparent, transparent 10rpx, rgba(255,107,91,0.08) 10rpx, rgba(255,107,91,0.08) 12rpx),
    $brand-lighter;
  margin-bottom: 24rpx;
}
.hint { display: block; color: $ink-3; font-size: 24rpx; margin-bottom: 40rpx; }
.actions { display: flex; gap: 20rpx; }
.btn {
  flex: 1;
  text-align: center;
  padding: 24rpx;
  border-radius: 999rpx;
  font-weight: 650;
}
.soft { background: $brand-soft; color: $brand-dark; }
.primary { background: $brand; color: #fff; }
</style>
