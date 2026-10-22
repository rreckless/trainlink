import { useEffect } from 'react'
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { IS_ANDROID } from '../services/api'
import { authApi } from '../services/authApi'

const RETURN_LINK = 'trainlink://auth-callback'

// Takes Google's return on Android. The system browser opens the app's link with the code, and the code finishes the
// sign-in. The link can also arrive when the app is started from it, so the launch link is checked too
export function useGoogleReturn(onSignedIn, onError) {
  useEffect(() => {
    if (!IS_ANDROID) return undefined
    const finish = async (urls) => {
      const link = (urls ?? []).find((url) => url.startsWith(RETURN_LINK))
      if (!link) return
      try {
        const code = new URL(link).searchParams.get('code')
        if (!code) throw new Error('Google sign-in was cancelled or did not finish')
        await authApi.completeGoogle(code)
        onError('')
        onSignedIn()
      } catch (err) {
        onError(err.message)
      }
    }
    getCurrent().then(finish).catch(() => {})
    let stopListening = null
    onOpenUrl(finish).then((stop) => { stopListening = stop }).catch(() => {})
    return () => stopListening?.()
  }, [onSignedIn, onError])
}
