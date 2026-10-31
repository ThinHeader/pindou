<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { draftsApi } from '../../api/http'
import type { DraftCard } from '../../api/types'
import { useLibraryStore } from '../../stores/library'
import { useUserStore } from '../../stores/user'
import { boardLabel, ensureLogin, showError } from '../../utils/ui'

const user = useUserStore()
const library = useLibraryStore()
const pills = [
  { id: 'all', label: '全部' },
  { id: 'mine', label: '我的稿' },
  { id: 'favorite', label: '收藏' },
  { id: 'template', label: '模板' },
  { id: 'official', label: '官方' },
]
const keyword = ref('')
const items = ref<DraftCard[]>([])
const loading = ref(false)

async function load() {
  if (!user.isLoggedIn) {
    items.value = []
    return
  }
  loading.value = true
  try {
    const data = await draftsApi.list(library.tab, keyword.value.trim())
    items.value = data.items
  } catch (e) {
    items.value = []
    showError(e, '稿件库加载失败')
  } finally {
    loading.value = false
  }
}

onShow(() => {
  if (!user.isLoggedIn) return
  load()
})

function pick(id: string) {
  if (!ensureLogin()) return
  library.open(id)
  load()
}
function openDraft(id: string) {
  uni.navigateTo({ url: `/pages/draft/detail?id=${id}` })
}
</script>

<template>
  <view class="page">
    <view class="search">
      <text>⌕</text>
      <input v-model="keyword" class="input" placeholder="搜索稿件名" confirm-type="search" @confirm="load" />
    </view>
    <view class="pills">
      <view
        v-for="pill in pills"
        :key="pill.id"
        class="pill"
        :class="{ on: library.tab === pill.id }"
        @click="pick(pill.id)"
      >{{ pill.label }}</view>
    </view>
    <view v-if="!user.isLoggedIn" class="empty" @click="ensureLogin()">登录后查看我的稿、收藏和模板</view>
    <view v-else-if="loading" class="muted">加载中…</view>
    <view v-else-if="items.length" class="grid">
      <view v-for="item in items" :key="item.id" class="card" @click="openDraft(item.id)">
        <image class="thumb" :src="item.preview_url" mode="aspectFill" />
        <view class="meta">
          <text class="name">{{ item.title }}</text>
          <text class="tags">{{ boardLabel(item.board_size, item.color_count) }} · {{ item.badge }}</text>
        </view>
      </view>
    </view>
    <view v-else class="empty">这里还是空的</view>
</view>
</template>

<style scoped lang="scss">
.page { padding: 16rpx 24rpx 140rpx; }
.search {
  display: flex; align-items: center; gap: 12rpx;
  background: $card; border: 1rpx solid $line; border-radius: 999rpx;
  padding: 14rpx 24rpx; color: $ink-3;
}
.input { flex: 1; font-size: 26rpx; color: $ink; }
.pills { display: flex; gap: 12rpx; margin: 20rpx 0; flex-wrap: wrap; }
.pill {
  padding: 10rpx 22rpx; border-radius: 999rpx; background: $card;
  border: 1rpx solid $line; color: $ink-2; font-size: 24rpx;
}
.pill.on { background: $ink; color: #fff; border-color: $ink; }
.grid {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  justify-content: space-between;
}
.card {
  flex: 0 0 340rpx;
  width: 340rpx;
  max-width: 340rpx;
  margin-bottom: 16rpx;
  box-sizing: border-box; background: $card; border-radius: 24rpx;
  border: 1rpx solid $line; overflow: hidden;
}
.thumb { width: 100%; height: 280rpx; background: $brand-lighter; display: block; }
.meta { padding: 14rpx 16rpx 18rpx; }
.name { display: block; font-weight: 700; }
.tags { display: block; margin-top: 6rpx; color: $ink-3; font-size: 22rpx; }
</style>
