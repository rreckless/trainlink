import { request } from './api'

// Decoder CVs. On the programming track the Z21 answers, and the answer is read with status(). On the main track a
// write gets no answer, so it cannot be checked, but a POM read does come back. A profile limits which CVs can be written
export const programmingApi = {
  status: () => request('GET', '/api/programming/status'),
  read: (cv) => request('POST', '/api/programming/read', { cv }),
  write: (cv, value, profileId) => request('POST', '/api/programming/write', { cv, value, confirm: true, profile_id: profileId ?? null }),
  pomRead: (locomotiveId, cv) => request('POST', '/api/programming/pom/read', { locomotive_id: locomotiveId, cv }),
  pomWrite: (locomotiveId, cv, value, profileId) =>
    request('POST', '/api/programming/pom/write', { locomotive_id: locomotiveId, cv, value, confirm: true, profile_id: profileId ?? null }),
}
