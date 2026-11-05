<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { draftsApi } from '../../api/http'
import type { DraftCard } from '../../api/types'
import { useLibraryStore } from '../../stores/library'
import { useUserStore } from '../../stores/user'
import { askConfirm, askText, boardLabel, checkedDraftTitle, ensureLogin, showError, showTip } from '../../utils/ui'

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

function search() {
  if (!ensureLogin()) return
  load()
}

function pick(id: string) {
  if (!ensureLogin()) return
  library.open(id)
  load()
}
function openDraft(id: string) {
  uni.navigateTo({ url: `/pages/draft/detail?id=${id}` })
}

function manage(item: DraftCard) {
  if (!ensureLogin()) return
  uni.showActionSheet({
    itemList: ['改名', '删除'],
    success: (res) => {
      if (res.tapIndex === 0) renameItem(item)
      if (res.tapIndex === 1) removeItem(item)
    },
  })
}

async function renameItem(item: DraftCard) {
  if (!ensureLogin()) return
  const current = item.title === '未命名草稿' ? '' : item.title
  const typed = await askText('修改稿件名称', current, '例如：窗边的猫')
  if (typed === null) return
  const title = checkedDraftTitle(typed)
  if (!title) return
  try {
    await draftsApi.rename(item.id, title)
    showTip('已改名')
    await load()
  } catch (e) {
    showError(e, '改名失败')
  }
}

async function removeItem(item: DraftCard) {
  if (!ensureLogin()) return
  const ok = await askConfirm(
    '删除稿件',
    `删除后无法恢复。确定删除「${item.title}」吗？`,
  )
  if (!ok) return
  try {
    await draftsApi.remove(item.id)
    showTip('已删除')
    await load()
  } catch (e) {
    showError(e, '删除失败')
  }
}
</script>

<template>
  <view class="page">
    <view class="search">
      <text>⌕</text>
      <input v-model="keyword" class="input" placeholder="搜索稿件名" confirm-type="search" @confirm="search" />
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
          <view class="name-row">
            <text class="name">{{ item.title }}</text>
            <text v-if="item.badge === '我的'" class="manage" @click.stop="manage(item)">管理</text>
          </view>
          <text class="tags">{{ boardLabel(item.board_size, item.color_count) }} · {{ item.badge }}</text>
          <text v-if="item.pipeline_tags?.length" class="pipe">{{ item.pipeline_tags.slice(0, 3).join(' · ') }}</text>
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
.grid { @include feed-grid; }
.card {
  @include feed-cell;
  background: $card; border-radius: 24rpx;
  border: 1rpx solid $line; overflow: hidden;
}
.thumb { width: 100%; height: 280rpx; background: $brand-lighter; display: block; }
.meta { padding: 14rpx 16rpx 18rpx; }
.name-row { display: flex; align-items: center; justify-content: space-between; gap: 8rpx; }
.name { flex: 1; min-width: 0; font-weight: 700; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.manage { flex-shrink: 0; color: $brand-dark; font-size: 22rpx; font-weight: 650; }
.tags { display: block; margin-top: 6rpx; color: $ink-3; font-size: 22rpx; }
.pipe { display: block; margin-top: 6rpx; color: $brand-dark; font-size: 20rpx; }
</style>
