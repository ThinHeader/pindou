<script setup lang="ts">
import { ref } from 'vue'

const chips = ['全部', '可爱', '风景', '像素', '文字']
const active = ref(0)

function openWork() {
  uni.navigateTo({ url: '/pages/work/detail' })
}
</script>

<template>
  <view class="page">
    <scroll-view scroll-x class="chips">
      <view
        v-for="(c, i) in chips"
        :key="c"
        class="chip"
        :class="{ on: active === i }"
        @click="active = i"
      >{{ c }}</view>
    </scroll-view>
    <view class="grid">
      <view v-for="n in 4" :key="n" class="card" @click="openWork">
        <view class="thumb" />
        <text class="name">示例作品 {{ n }}</text>
      </view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 16rpx 24rpx 120rpx; }
.chips { white-space: nowrap; margin-bottom: 24rpx; }
.chip {
  display: inline-block;
  padding: 12rpx 28rpx;
  margin-right: 12rpx;
  border-radius: 999rpx;
  background: $card;
  color: $ink-2;
  font-size: 26rpx;
  border: 1rpx solid $line;
}
.chip.on { background: $brand-soft; color: $brand-dark; border-color: transparent; font-weight: 650; }
.grid { display: flex; flex-wrap: wrap; gap: 20rpx; }
.card {
  width: calc(50% - 10rpx);
  background: $card;
  border-radius: 20rpx;
  overflow: hidden;
  border: 1rpx solid $line;
}
.thumb {
  height: 280rpx;
  background: linear-gradient(145deg, $brand-soft, $warm-soft);
}
.name { display: block; padding: 16rpx 20rpx 24rpx; font-size: 26rpx; color: $ink; }
</style>
