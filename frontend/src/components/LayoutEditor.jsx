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
  piece: 'Pick a piece, then click to place it. Near an open end (orange) it joins, unless "Place without connecting" is ticked.',
  operate: 'Click a turnout with a DCC address to switch it. Orange means diverging.',
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
      } else if ((event.key === 'r' || event.key === 'R') && editor.selectedPlaced) {
        // R turns the selected track piece 15 degrees, Shift+R the other way
        editor.rotatePiece(editor.selectedPlaced.id, event.shiftKey ? -15 : 15)
      } else if (event.key === 'Escape') {
        editor.cancelTrack()
        editor.setSelectedKey(null)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [editor.deleteSelected, editor.cancelTrack, editor.setSelectedKey, editor.selectedPlaced, editor.rotatePiece])

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
        onRenameLayout={editor.renameLayout}
        onDeleteLayout={editor.deleteLayout}
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
          <div className="lg:col-span-3 bg-slate-800 rounded-lg p-2">
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
              operating={editor.tool === 'operate'}
              turnoutStates={editor.turnoutStates}
              onThrow={editor.throwPlaced}
              onHandle={editor.moveHandle}
              onEndDrag={editor.dragPlacedEnd}
              onPieceDrag={editor.dragPiece}
              onCanvasClick={editor.handleCanvasClick}
              onCursor={editor.setCursor}
              onSelect={editor.setSelectedKey}
              onMove={editor.moveItem}
            />
            <p className="text-xs text-slate-500 mt-2">
              <span className="text-orange-500">●</span> open end <span className="text-green-500 ml-2">●</span> joined
            </p>
            <p className="text-xs text-slate-500 mt-1">{HINTS[editor.tool] || HINTS.select}</p>
          </div>

          <div className="bg-slate-800 rounded-lg p-4">
            {editor.tool === 'piece' ? (
              <TrackPicker
                library={editor.library}
                trackSystem={editor.trackSystem}
                pieceId={editor.pieceId}
                onPick={editor.choosePiece}
                freePlace={editor.freePlace}
                onToggleFree={editor.setFreePlace}
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
                  onSetDcc={(fields) => editor.setDcc(editor.selectedPlaced.id, fields)}
                  onMakeCurved={() => editor.makeCurved(editor.selectedPlaced.id)}
                  onStraighten={() => editor.straighten(editor.selectedPlaced.id)}
                  onSetLength={(length) => editor.setPlacedLength(editor.selectedPlaced.id, length)}
                  onSmooth={() => editor.smoothCurve(editor.selectedPlaced.id)}
                  onRotate={(degrees) => editor.rotatePiece(editor.selectedPlaced.id, degrees)}
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
