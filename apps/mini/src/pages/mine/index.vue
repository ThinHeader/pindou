<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { computed, ref } from 'vue'
import { uploadImage } from '../../api/http'
import { useCreditsStore } from '../../stores/credits'
import { useLibraryStore } from '../../stores/library'
import { useUserStore } from '../../stores/user'
import { initialOf, showError, showTip } from '../../utils/ui'

const user = useUserStore()
const credits = useCreditsStore()
const library = useLibraryStore()

const editing = ref(false)
const saving = ref(false)
const editorKey = ref(0)
const draftNickname = ref('')
const pendingAvatarPath = ref('')
const localPreview = ref('')

const shownAvatar = computed(() => localPreview.value || user.avatarUrl)
const showEditor = computed(() => user.isLoggedIn && (editing.value || !user.avatarUrl))
const saveLabel = computed(() => {
  if (saving.value) return '保存中'
  return user.avatarUrl ? '保存' : '完善头像昵称'
})

function displayDraft(name: string) {
  if (!user.avatarUrl && name === '拼豆玩家') return ''
  return name
}

onShow(async () => {
  if (!user.isLoggedIn) return
  try {
    await user.refreshProfile()
    if (user.profile) credits.apply(user.profile.credits)
    if (!editing.value) draftNickname.value = displayDraft(user.nickname)
  } catch (e) {
    if (!user.isLoggedIn) return
    showError(e, '资料加载失败')
  }
})

function goLogin() {
  uni.navigateTo({ url: '/pages/login/index' })
}
function goCredits() {
  uni.navigateTo({ url: '/pages/credits/index' })
}
function goLibrary(tab: string) {
  library.open(tab)
  uni.switchTab({ url: '/pages/library/index' })
}
function openDraft(id: string) {
  uni.navigateTo({ url: `/pages/draft/detail?id=${id}` })
}
function goCreate() {
  uni.switchTab({ url: '/pages/create/index' })
}

async function claim() {
  if (!user.isLoggedIn) {
    goLogin()
    return
  }
  try {
    const data = await credits.claimDaily()
    showTip(`已领取今日 ${data.delta} 积分`)
    await user.refreshProfile()
  } catch (e) {
    const err = e as { error?: string; message?: string }
    if (err.error === 'already_claimed') {
      credits.dailyClaimed = true
      showTip(err.message || '今日已领取')
      return
    }
    showError(e, '领取失败')
  }
}

function startEdit() {
  if (!user.isLoggedIn) {
    goLogin()
    return
  }
  draftNickname.value = displayDraft(user.nickname)
  editorKey.value += 1
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  pendingAvatarPath.value = ''
  localPreview.value = ''
  draftNickname.value = displayDraft(user.nickname)
}

function applyPickedAvatar(path: string) {
  pendingAvatarPath.value = path
  localPreview.value = path
  if (!showEditor.value) startEdit()
}

function onChooseAvatar(e: unknown) {
  const path = (e as { detail?: { avatarUrl?: string } }).detail?.avatarUrl?.trim() || ''
  if (!path) return
  applyPickedAvatar(path)
}

function onAvatarTap() {
  if (!user.isLoggedIn) {
    goLogin()
    return
  }
  // 微信里用 button open-type="chooseAvatar"。H5 / 开发者工具没有该能力时，用选图走同一套上传。
  // #ifdef H5
  uni.chooseImage({
    count: 1,
    sizeType: ['compressed'],
    sourceType: ['album'],
    success: (res) => {
      const path = res.tempFilePaths?.[0]
      if (path) applyPickedAvatar(path)
    },
  })
  // #endif
}

function onNicknameInput(e: unknown) {
  const value = (e as { detail?: { value?: string } }).detail?.value
  if (typeof value === 'string') draftNickname.value = value
}

async function onSubmit(e: unknown) {
  if (saving.value) return
  const fromForm = (e as { detail?: { value?: { nickname?: string } } }).detail?.value?.nickname
  const nickname = String(typeof fromForm === 'string' ? fromForm : draftNickname.value).trim()
  if (!nickname) {
    showTip('请填写昵称')
    return
  }
  if ([...nickname].length > 20) {
    showTip('昵称需要 1 到 20 个字')
    return
  }
  if (!pendingAvatarPath.value && !user.avatarUrl) {
    showTip('请选择头像')
    return
  }
  saving.value = true
  try {
    const body: { nickname: string; avatar_key?: string } = { nickname }
    if (pendingAvatarPath.value) {
      const stored = await uploadImage(pendingAvatarPath.value)
      body.avatar_key = stored.key
    }
    await user.updateProfile(body)
    pendingAvatarPath.value = ''
    localPreview.value = ''
    editing.value = false
    draftNickname.value = displayDraft(user.nickname)
    showTip('已更新资料')
  } catch (err) {
    showError(err, '保存失败')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <view class="page">
    <view class="profile">
      <button
        v-if="user.isLoggedIn"
        class="avatar-btn"
        open-type="chooseAvatar"
        hover-class="none"
        @chooseavatar="onChooseAvatar"
        @click="onAvatarTap"
      >
        <image v-if="shownAvatar" class="avatar-img" :src="shownAvatar" mode="aspectFill" />
        <text v-else class="avatar">{{ initialOf(draftNickname || user.nickname) }}</text>
      </button>
      <text v-else class="avatar" @click="goLogin">豆</text>

      <view v-if="!user.isLoggedIn" class="info" @click="goLogin">
        <text class="name">点击登录</text>
        <text class="sub">微信一键登录</text>
      </view>
      <form v-else-if="showEditor" class="info" @submit="onSubmit">
        <input
          :key="editorKey"
          class="nick"
          type="nickname"
          name="nickname"
          :value="draftNickname"
          placeholder="点击填写昵称"
          maxlength="20"
          @input="onNicknameInput"
          @blur="onNicknameInput"
        />
        <text class="sub">头像和昵称仅用于在「我的」展示当前账号</text>
        <button class="save" form-type="submit" hover-class="none" :disabled="saving">{{ saveLabel }}</button>
        <text v-if="user.avatarUrl" class="cancel" @click="cancelEdit">取消</text>
      </form>
      <view v-else class="info" @click="startEdit">
        <text class="name">{{ user.nickname }}</text>
        <text class="sub">ID {{ user.user?.id.slice(0, 8) }} · 点按可修改</text>
        <text class="sub">头像和昵称仅在本页展示</text>
      </view>
    </view>

    <view v-if="user.profile" class="stats">
      <view class="s"><text class="n">{{ user.profile.stats.drafts }}</text><text>稿件</text></view>
      <view class="s"><text class="n">{{ user.profile.stats.favorites }}</text><text>收藏</text></view>
    </view>

    <view class="credit" @click="goCredits">
      <view>
        <text class="label">当前积分</text>
        <text class="num">{{ credits.balance }}</text>
        <text class="rule">日送 {{ credits.dailyGrant }} · 出稿 −{{ credits.generateCost }}/次 · 失败不扣</text>
      </view>
      <view class="side">
        <text class="state">{{ credits.dailyClaimed ? '今日已领' : '今日未领' }}</text>
        <text class="recharge">去充值</text>
      </view>
    </view>
    <view class="claim" :class="{ done: credits.dailyClaimed }" @click="claim">
      {{ credits.dailyClaimed ? '今日已领' : `领取今日积分 · +${credits.dailyGrant}` }}
    </view>

    <view class="menu">
      <view class="item" @click="goCredits">充值积分<text class="arrow">›</text></view>
      <view class="item" @click="goCredits">积分明细<text class="arrow">›</text></view>
      <view class="item" @click="goLibrary('mine')">我的稿件<text class="arrow">›</text></view>
      <view class="item" @click="goLibrary('favorite')">我的收藏<text class="arrow">›</text></view>
      <view class="item" @click="showTip('豆色库存即将开放')">我的豆色库存<text class="arrow">›</text></view>
    </view>
    <view class="menu">
      <view class="item" @click="showTip('消息通知即将开放')">消息通知<text class="arrow">›</text></view>
      <view class="item" @click="showTip('意见反馈即将开放')">意见反馈<text class="arrow">›</text></view>
      <view class="item" @click="goLogin">{{ user.isLoggedIn ? '切换账号 / 登录' : '登录' }}<text class="arrow">›</text></view>
    </view>

    <view class="sec-title"><text>最近稿件</text></view>
    <view v-if="user.profile?.recent_drafts.length" class="works">
      <view v-for="item in user.profile.recent_drafts.slice(0, 2)" :key="item.id" class="work" @click="openDraft(item.id)">
        <image class="thumb" :src="item.preview_url" mode="aspectFill" />
        <text class="wname">{{ item.title }}</text>
      </view>
      <view class="work add" @click="goCreate">去出稿</view>
    </view>
    <view v-else class="empty">出稿后，最近的稿会出现在这里</view>
</view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 140rpx; }
.profile { display: flex; gap: 20rpx; align-items: center; }
.avatar-btn {
  padding: 0; margin: 0; width: 108rpx; height: 108rpx; min-height: 108rpx;
  border-radius: 32rpx; overflow: hidden; background: transparent; line-height: 108rpx; flex-shrink: 0;
}
.avatar-btn::after { border: none; }
.avatar {
  width: 108rpx; height: 108rpx; border-radius: 32rpx;
  background: linear-gradient(135deg, $brand, $warm); color: #fff;
  font-size: 44rpx; font-weight: 800; text-align: center; line-height: 108rpx;
  display: block;
}
.avatar-img { width: 108rpx; height: 108rpx; display: block; }
.info { flex: 1; min-width: 0; }
.name { display: block; font-size: 36rpx; font-weight: 800; }
.nick {
  height: 72rpx; font-size: 32rpx; font-weight: 800; color: $ink;
  background: $card; border: 1rpx solid $line; border-radius: 16rpx; padding: 0 16rpx;
}
.sub { display: block; margin-top: 6rpx; color: $ink-3; font-size: 24rpx; }
.save {
  margin: 16rpx 0 0; background: $brand; color: #fff; border-radius: 999rpx;
  font-size: 26rpx; font-weight: 750; line-height: 2.2;
}
.save::after { border: none; }
.cancel { display: inline-block; margin-top: 12rpx; color: $ink-3; font-size: 24rpx; }
.stats {
  display: flex; margin-top: 24rpx; background: $card; border-radius: 24rpx; border: 1rpx solid $line;
}
.s { flex: 1; text-align: center; padding: 20rpx 0; color: $ink-3; font-size: 22rpx; }
.n { display: block; color: $ink; font-size: 34rpx; font-weight: 800; }
.credit {
  margin-top: 20rpx; display: flex; justify-content: space-between; align-items: center;
  background: linear-gradient(135deg, #2c2a29, #5a403c); color: #fff; border-radius: 28rpx; padding: 28rpx;
}
.label { font-size: 22rpx; opacity: 0.8; }
.num { display: block; font-size: 64rpx; font-weight: 800; line-height: 1.1; }
.rule { font-size: 22rpx; opacity: 0.85; }
.side { text-align: right; }
.state { display: block; font-size: 22rpx; background: rgba(255,255,255,0.14); border-radius: 999rpx; padding: 6rpx 14rpx; }
.recharge { display: inline-block; margin-top: 16rpx; background: #fff; color: $brand-dark; border-radius: 999rpx; padding: 10rpx 20rpx; font-size: 24rpx; font-weight: 700; }
.claim {
  margin-top: 16rpx; text-align: center; background: $brand; color: #fff;
  border-radius: 999rpx; padding: 22rpx; font-weight: 750;
}
.claim.done { background: $brand-soft; color: $brand-dark; }
.menu { margin-top: 20rpx; background: $card; border-radius: 24rpx; border: 1rpx solid $line; overflow: hidden; }
.item { display: flex; justify-content: space-between; padding: 28rpx; border-bottom: 1rpx solid $line; }
.item:last-child { border-bottom: none; }
.arrow { color: $ink-3; }
.works { display: flex; gap: 12rpx; }
.work { flex: 1; background: $card; border-radius: 20rpx; border: 1rpx solid $line; overflow: hidden; }
.thumb { width: 100%; height: 160rpx; display: block; background: $brand-lighter; }
.wname { display: block; padding: 10rpx 12rpx 14rpx; font-size: 22rpx; font-weight: 650; }
.add {
  display: flex; align-items: center; justify-content: center;
  color: $brand-dark; background: $brand-lighter; border: 1rpx dashed #F0C4BC; min-height: 140rpx;
}
</style>
