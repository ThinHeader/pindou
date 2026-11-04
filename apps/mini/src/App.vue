<script setup lang="ts">
import { onLaunch, onShow } from '@dcloudio/uni-app'
import { useCreditsStore } from './stores/credits'
import { useUserStore } from './stores/user'

onLaunch(() => {
  const user = useUserStore()
  const credits = useCreditsStore()
  user.hydrate()
  if (user.isLoggedIn) credits.refresh()
})

onShow(() => {
  const user = useUserStore()
  if (!user.token) user.hydrate()
  void user.ensureFreshSession()
})
</script>

<style lang="scss">
@import './uni.scss';

page {
  background: $bg;
  color: $ink;
  font-family: $font;
  font-size: 28rpx;
  line-height: 1.45;
}

.sec-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 30rpx;
  font-weight: 700;
  color: $ink;
  margin: 28rpx 0 16rpx;
}
.sec-link { color: $brand; font-size: 24rpx; font-weight: 600; }
.muted { color: $ink-3; font-size: 24rpx; }
.empty {
  background: $card;
  border: 1rpx dashed $line;
  border-radius: 24rpx;
  padding: 36rpx 28rpx;
  color: $ink-3;
  text-align: center;
  font-size: 26rpx;
}
</style>
