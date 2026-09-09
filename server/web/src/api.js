async function request(method, path, body, token) {
  const headers = { 'Content-Type': 'application/json' }
  if (token) headers['Authorization'] = 'Bearer ' + token
  // 相对路径请求：兼容 fnOS 统一网关（反向代理可能带应用路径前缀）
  const rel = path.replace(/^\//, '')
  const res = await fetch(rel, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined
  })
  let data = {}
  try {
    data = await res.json()
  } catch (e) {
    /* ignore */
  }
  if (!res.ok) throw new Error(data.error || '请求失败: ' + res.status)
  return data
}

export default {
  status: () => request('GET', '/api/status'),
  setup: (u, p) => request('POST', '/api/setup', { username: u, password: p }),
  login: (u, p) => request('POST', '/api/login', { username: u, password: p }),
  register: (data) => request('POST', '/api/register', data),
  publicSettings: () => request('GET', '/api/settings/public'),
  me: (t) => request('GET', '/api/me', undefined, t),
  updateMe: (data, t) => request('PUT', '/api/me', data, t),
  uploadAvatar: (data, t) => request('POST', '/api/me/avatar', data, t),
  listEntries: (t) => request('GET', '/api/entries', undefined, t),
  createEntry: (e, t) => request('POST', '/api/entries', e, t),
  updateEntry: (e, t) => request('PUT', '/api/entries/' + e.id, e, t),
  deleteEntry: (id, t) => request('DELETE', '/api/entries/' + id, undefined, t),
  importEntries: (csv, t) => request('POST', '/api/entries/import', { csv }, t),
  importText: (text, t) => request('POST', '/api/entries/import-text', { text }, t),
  listUsers: (t) => request('GET', '/api/users', undefined, t),
  createUser: (u, t) => request('POST', '/api/users', u, t),
  importUsers: (csv, t) => request('POST', '/api/users/import', { csv }, t),
  deleteUser: (id, t) => request('DELETE', '/api/users/' + id, undefined, t),
  updateUserStatus: (id, status, t) => request('PUT', '/api/users/' + id + '/status', { status }, t),
  updateUser: (id, data, t) => request('PUT', '/api/users/' + id, data, t),
  listLogs: (t) => request('GET', '/api/logs', undefined, t),
  getSettings: (t) => request('GET', '/api/settings', undefined, t),
  updateSettings: (data, t) => request('PUT', '/api/settings', data, t),
  sendRegisterCode: (email) => request('POST', '/api/register/send-code', { email }),
  sendResetCode: (email) => request('POST', '/api/reset/send-code', { email }),
  resetPassword: (data) => request('POST', '/api/reset', data)
}
