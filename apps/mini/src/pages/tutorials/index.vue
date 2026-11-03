<script setup lang="ts">
import { tutorials } from '../../data/tutorials'

function openTutorial(id: string) {
  uni.navigateTo({ url: `/pages/tutorials/detail?id=${id}` })
}
</script>

<template>
  <view class="page">
    <text class="lead">官方编写的出稿和开拼说明，只能阅读。</text>
    <view
      v-for="item in tutorials"
      :key="item.id"
      class="card"
      @click="openTutorial(item.id)"
    >
      <view class="cover" :style="{ background: item.tint }">
        <text class="emoji">{{ item.cover }}</text>
      </view>
      <view class="meta">
        <text class="name">{{ item.title }}</text>
        <text class="kicker">{{ item.kicker }}</text>
        <text class="summary">{{ item.summary }}</text>
      </view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 24rpx 140rpx; }
.lead { display: block; color: $ink-3; font-size: 24rpx; margin-bottom: 20rpx; }
.card {
  display: flex;
  gap: 16rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 24rpx;
  padding: 16rpx;
  margin-bottom: 16rpx;
}
.cover {
  width: 120rpx;
  height: 120rpx;
  border-radius: 20rpx;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.emoji { font-size: 48rpx; }
.meta { flex: 1; min-width: 0; }
.name { display: block; font-size: 30rpx; font-weight: 800; }
.kicker { display: block; margin-top: 6rpx; color: $brand-dark; font-size: 22rpx; }
.summary { display: block; margin-top: 8rpx; color: $ink-2; font-size: 24rpx; line-height: 1.45; }
</style>
