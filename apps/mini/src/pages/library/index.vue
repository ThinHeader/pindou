<script setup lang="ts">
function openDraft() {
  uni.navigateTo({ url: '/pages/draft/detail' })
}
</script>

<template>
  <view class="page">
    <view class="search">搜索稿件（占位）</view>
    <view class="grid">
      <view v-for="n in 6" :key="n" class="card" @click="openDraft">
        <view class="thumb" />
        <text class="name">我的稿件 {{ n }}</text>
      </view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx; padding-bottom: 120rpx; }
.search {
  background: $card;
  border: 1rpx solid $line;
  border-radius: 999rpx;
  padding: 20rpx 32rpx;
  color: $ink-3;
  margin-bottom: 24rpx;
  font-size: 26rpx;
}
.grid { display: flex; flex-wrap: wrap; gap: 20rpx; }
.card {
  width: calc(50% - 10rpx);
  background: $card;
  border-radius: 20rpx;
  border: 1rpx solid $line;
  overflow: hidden;
}
.thumb { height: 240rpx; background: $warm-soft; }
.name { display: block; padding: 16rpx 20rpx 24rpx; font-size: 26rpx; }
</style>
