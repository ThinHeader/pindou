<script setup lang="ts">
import type { PaletteColor } from '../api/types'

defineProps<{ items: PaletteColor[] }>()
</script>

<template>
  <view class="legend">
    <view v-for="item in items" :key="item.code" class="item">
      <view class="swatch" :style="{ background: item.hex }" />
      <text>{{ item.code }} · {{ item.name }} · {{ item.count }}</text>
    </view>
  </view>
</template>

<style scoped lang="scss">
.legend { display: flex; flex-wrap: wrap; gap: 12rpx; }
.item {
  display: flex;
  align-items: center;
  gap: 8rpx;
  padding: 8rpx 16rpx;
  background: $card;
  border-radius: 999rpx;
  border: 1rpx solid $line;
  font-size: 22rpx;
  color: $ink-2;
}
.swatch {
  width: 22rpx;
  height: 22rpx;
  border-radius: 50%;
  border: 1rpx solid rgba(0, 0, 0, 0.08);
}
</style>
