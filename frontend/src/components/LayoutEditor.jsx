import { useEffect, useState } from 'react'
import { useLayoutEditor } from '../hooks/useLayoutEditor'
import AnyRailImport from './layout/AnyRailImport'
import LayoutCanvas from './layout/LayoutCanvas'
import LayoutToolbar from './layout/LayoutToolbar'
import LayoutProperties from './layout/LayoutProperties'
import TrackPicker from './layout/TrackPicker'

const HINTS = {
  select: 'Click to select. Drag to move. Delete removes the selection.',
  track: 'Click to start a track, click again to add a segment. Escape stops.',
  piece: 'Pick a piece, then move the cursor near an open track end (orange). Click to place.',
}

export default function LayoutEditor() {
  const editor = useLayoutEditor()
  const [importing, setImporting] = useState(false)

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
        importing={importing}
        onChooseTool={editor.chooseTool}
        onSwitchLayout={editor.switchLayout}
        onCreateLayout={editor.createLayout}
        onToggleImport={() => setImporting(!importing)}
        onSave={editor.save}
      />

      {importing && (
        <AnyRailImport
          busy={editor.busy}
          report={editor.importReport}
          onImport={editor.importAnyRail}
          onClose={() => setImporting(false)}
        />
      )}

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
              pieces={editor.pieces}
              libraryById={editor.libraryById}
              ghost={editor.ghost}
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
            <p className="text-xs text-slate-500 mt-2">{HINTS[editor.tool] || HINTS.select}</p>
          </div>

          <div className="bg-slate-800 rounded-lg p-4">
            {editor.tool === 'piece' ? (
              <TrackPicker
                library={editor.library}
                trackSystem={editor.trackSystem}
                pieceId={editor.pieceId}
                onPick={editor.choosePiece}
              />
            ) : (
              <>
                <h4 className="font-semibold mb-3">Properties</h4>
                <LayoutProperties
                  item={editor.selectedItem}
                  track={editor.selectedTrack}
                  placed={editor.selectedPlaced}
                  piece={editor.selectedPlaced ? editor.libraryById[editor.selectedPlaced.piece_id] : null}
                  busy={editor.busy}
                  onRename={editor.renameSelected}
                  onDelete={editor.deleteSelected}
                />
              </>
            )}
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
