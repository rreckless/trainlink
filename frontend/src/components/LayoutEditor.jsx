import { useEffect } from 'react'
import { useLayoutEditor } from '../hooks/useLayoutEditor'
import LayoutCanvas from './layout/LayoutCanvas'
import LayoutToolbar from './layout/LayoutToolbar'
import LayoutProperties from './layout/LayoutProperties'

export default function LayoutEditor() {
  const editor = useLayoutEditor()

  // Keyboard shortcuts: Delete removes the selection, Escape cancels track drawing
  useEffect(() => {
    const onKeyDown = (event) => {
      const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(event.target.tagName)
      if (typing) return
      if (event.key === 'Delete' || event.key === 'Backspace') {
        editor.deleteSelected()
      } else if (event.key === 'Escape') {
        editor.cancelTrack()
        editor.setSelectedKey(null)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [editor.deleteSelected, editor.cancelTrack, editor.setSelectedKey])

  return (
    <div className="space-y-4">
      <LayoutToolbar
        layouts={editor.layouts}
        layout={editor.layout}
        tool={editor.tool}
        dirty={editor.dirty}
        busy={editor.busy}
        onChooseTool={editor.chooseTool}
        onSwitchLayout={editor.switchLayout}
        onCreateLayout={editor.createLayout}
        onSave={editor.save}
      />

      {editor.error && (
        <div className="bg-red-900 border border-red-700 rounded p-3">
          <p className="text-red-200 text-sm">{editor.error}</p>
        </div>
      )}
      {editor.message && !editor.error && (
        <div className="bg-green-900 border border-green-700 rounded p-3">
          <p className="text-green-200 text-sm">{editor.message}</p>
        </div>
      )}

      {editor.layout ? (
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-4">
          <div className="lg:col-span-3 bg-slate-800 rounded-lg p-2 overflow-auto">
            <LayoutCanvas
              layout={editor.layout}
              tracks={editor.tracks}
              items={editor.items}
              tool={editor.tool}
              selectedKey={editor.selectedKey}
              pendingStart={editor.pendingStart}
              cursor={editor.cursor}
              onCanvasClick={editor.handleCanvasClick}
              onCursor={editor.setCursor}
              onSelect={editor.setSelectedKey}
              onMove={editor.moveItem}
            />
            <p className="text-xs text-slate-500 mt-2">
              {editor.tool === 'track'
                ? 'Click to start a track, click again to add a segment. Escape stops.'
                : editor.tool === 'select'
                ? 'Click to select. Drag to move. Delete removes the selection.'
                : 'Click the canvas to place the element. Choose Select to go back.'}
            </p>
          </div>

          <div className="bg-slate-800 rounded-lg p-4">
            <h4 className="font-semibold mb-3">Properties</h4>
            <LayoutProperties
              item={editor.selectedItem}
              track={editor.selectedTrack}
              busy={editor.busy}
              onRename={editor.renameSelected}
              onDelete={editor.deleteSelected}
            />
          </div>
        </div>
      ) : (
        <div className="bg-slate-800 rounded-lg p-8 text-center">
          <p className="text-slate-400">
            {editor.layouts.length === 0
              ? 'No layouts yet. Use New Layout to create one.'
              : 'Choose a layout to edit.'}
          </p>
        </div>
      )}
    </div>
  )
}
