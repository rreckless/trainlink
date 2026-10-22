import { request } from './api'

// Sign-in through the core: who is signed in, and sign in, sign up, sign out, and password reset
export const authApi = {
  getSession: () => request('GET', '/api/auth/session'),
  signIn: (email, password) => request('POST', '/api/auth/sign-in', { email, password }),
  signUp: (email, password) => request('POST', '/api/auth/sign-up', { email, password }),
  signOut: () => request('POST', '/api/auth/sign-out'),
  resetPassword: (email) => request('POST', '/api/auth/reset-password', { email }),
}
