import { useState } from 'react'
import { openUrl } from '@tauri-apps/plugin-opener'
import { IS_DESKTOP } from '../../services/api'
import { authApi } from '../../services/authApi'

// The desktop app waits while the sign-in finishes in the browser: about three minutes
async function waitForSignIn() {
  for (let attempt = 0; attempt < 90; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 2000))
    const session = await authApi.getSession()
    if (session.signed_in) return
  }
  throw new Error('The Google sign-in did not finish. Try again')
}

const MODES = {
  signin: { title: 'Sign in', action: 'Sign in', switchText: 'New to TrainLink? Create an account', switchTo: 'signup' },
  signup: { title: 'Create an account', action: 'Create account', switchText: 'Already have an account? Sign in', switchTo: 'signin' },
  reset: { title: 'Reset your password', action: 'Send reset email', switchText: 'Back to sign in', switchTo: 'signin' },
}

// The sign-in screen. Shown until someone is signed in
export default function SignIn({ onSignedIn }) {
  const [mode, setMode] = useState('signin')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const copy = MODES[mode]

  const switchMode = (next) => {
    setMode(next)
    setError('')
    setNotice('')
  }

  const signInWithGoogle = async () => {
    setBusy(true)
    setError('')
    setNotice('')
    try {
      const { url } = await authApi.startGoogle(window.location.origin, IS_DESKTOP)
      if (!IS_DESKTOP) {
        window.location.href = url
        return
      }
      await openUrl(url)
      setNotice('Finish signing in with Google in your browser.')
      await waitForSignIn()
      onSignedIn()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const submit = async (event) => {
    event.preventDefault()
    setBusy(true)
    setError('')
    setNotice('')
    try {
      if (mode === 'reset') {
        await authApi.resetPassword(email)
        setNotice('If an account exists for that email, a reset link is on its way.')
      } else if (mode === 'signup') {
        const result = await authApi.signUp(email, password)
        if (result.status === 'confirm_email') {
          setNotice('Check your email and follow the link to confirm your account, then sign in.')
          switchMode('signin')
        } else {
          onSignedIn()
        }
      } else {
        await authApi.signIn(email, password)
        onSignedIn()
      }
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const input = 'mt-1 w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-white focus:outline-none focus:border-slate-500'

  return (
    <div className="min-h-screen flex items-center justify-center bg-slate-900 text-white p-4">
      <div className="w-full max-w-sm">
        <div className="flex items-center gap-2 mb-6 justify-center">
          <div className="w-9 h-9 rounded-lg bg-green-500" aria-hidden="true" />
          <span className="text-xl font-semibold">TrainLink</span>
        </div>
        <form onSubmit={submit} className="bg-slate-800 border border-slate-700 rounded-xl p-6 space-y-4">
          <h2 className="text-lg font-semibold">{copy.title}</h2>
          <label className="block text-sm text-slate-300">
            Email
            <input type="email" required autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} className={input} />
          </label>
          {mode !== 'reset' && (
            <label className="block text-sm text-slate-300">
              Password{mode === 'signup' ? ' (at least 8 characters)' : ''}
              <input
                type="password"
                required
                minLength={8}
                autoComplete={mode === 'signup' ? 'new-password' : 'current-password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className={input}
              />
            </label>
          )}
          {error && <p className="text-sm text-red-300" role="alert">{error}</p>}
          {notice && <p className="text-sm text-green-300">{notice}</p>}
          <button type="submit" disabled={busy} className="w-full py-2 rounded-lg bg-green-600 hover:bg-green-500 font-semibold disabled:opacity-50">
            {busy ? 'Working…' : copy.action}
          </button>
          <div className="flex justify-between text-sm">
            <button type="button" onClick={() => switchMode(copy.switchTo)} className="text-slate-300 hover:text-white">
              {copy.switchText}
            </button>
            {mode === 'signin' && (
              <button type="button" onClick={() => switchMode('reset')} className="text-slate-400 hover:text-white">
                Forgot password?
              </button>
            )}
          </div>
        </form>
        {mode !== 'reset' && (
          <button
            type="button"
            onClick={signInWithGoogle}
            disabled={busy}
            className="mt-3 w-full py-2 rounded-lg bg-white text-slate-900 hover:bg-slate-200 font-semibold disabled:opacity-50"
          >
            Continue with Google
          </button>
        )}
        <p className="text-xs text-slate-500 text-center mt-4">
          Your layouts and locomotives belong to your account. Sync between devices is coming next.
        </p>
      </div>
    </div>
  )
}
