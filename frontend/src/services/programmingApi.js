import { request } from './api'

// Decoder CVs. On the programming track the Z21 answers, and the answer is read with status(). On the main track a
// write gets no answer, so it cannot be checked
export const programmingApi = {
  status: () => request('GET', '/api/programming/status'),
  read: (cv) => request('POST', '/api/programming/read', { cv }),
  write: (cv, value) => request('POST', '/api/programming/write', { cv, value, confirm: true }),
  pomWrite: (locomotiveId, cv, value) => request('POST', '/api/programming/pom/write', { locomotive_id: locomotiveId, cv, value, confirm: true }),
}
