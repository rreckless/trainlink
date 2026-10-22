// Questions asked in the page, and shown by DialogHost. The browser's own confirm and prompt dialogs are not shown
// reliably inside the desktop app's webview, so the app asks its questions in the page instead

let open = null
const listeners = new Set()

const notify = () => listeners.forEach((listener) => listener(open))

export function subscribeDialog(listener) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

// Ends the open question with a result. A question that is still open when another one starts is cancelled
export function closeDialog(result) {
  const dialog = open
  open = null
  notify()
  dialog?.resolve(result)
}

function ask(dialog) {
  if (open) closeDialog(dialog.kind === 'text' ? null : false)
  return new Promise((resolve) => {
    open = { ...dialog, resolve }
    notify()
  })
}

// Resolves true when the user confirms, false when they cancel
export function confirmAction(message, { okLabel = 'OK', danger = false } = {}) {
  return ask({ kind: 'confirm', message, okLabel, danger })
}

// Resolves with the text the user typed, or null when they cancel
export function askText(message, initial = '', { okLabel = 'OK' } = {}) {
  return ask({ kind: 'text', message, value: initial, okLabel })
}
