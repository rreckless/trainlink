import { useEffect, useState } from 'react'
import { Info, MousePointer2, Puzzle, Spline, Upload } from 'lucide-react'
import { useLayoutEditor } from '../hooks/useLayoutEditor'
import { useLiveTrackside } from '../hooks/useLiveTrackside'
import AnyRailImport from './layout/AnyRailImport'
import LayoutCanvas from './layout/LayoutCanvas'
import LayoutToolbar from './layout/LayoutToolbar'
import LayoutProperties from './layout/LayoutProperties'
import TrackPicker from './layout/TrackPicker'
import EditDialog from './shell/EditDialog'

const HINTS = {
  select: 'Click to select. Drag to move. Delete removes the selection.',
  track: 'Click to start a track, click again to add a segment. Escape stops.',
  piece: 'Pick a piece, then click to place it. Near an open end (orange) it joins, unless "Place without connecting" is ticked.',
  operate: 'Click a turnout with a DCC address to switch it. Orange means diverging.',
  block: 'Choose a block, then click the track it covers.',
  signal: 'Choose a signal, then click where it stands.',
  feedback: 'Choose a feedback, then click the track it reads.',
}

// The tool's name and what a click does with it, shown above the palette when no piece is being picked
const TOOL_GUIDE = {
  select: { title: 'Select', icon: MousePointer2, text: 'Choose a piece or a track to see and change it.' },
  track: { title: 'Draw track', icon: Spline, text: 'Draw straight and curved track with the mouse.' },
  piece: { title: 'Place piece', icon: Puzzle, text: 'Pick a piece from the list, then place it on the layout.' },
  operate: { title: 'Operate', icon: Spline, text: 'Throw turnouts that have a DCC address.' },
  block: { title: 'Block', icon: Spline, text: 'Mark the track each block covers.' },
  signal: { title: 'Signal', icon: Spline, text: 'Place signals on the track.' },
  feedback: { title: 'Feedback', icon: Spline, text: 'Link feedback sensors to the track they read.' },
}

const SHORTCUTS = [
  ['R', 'Rotate'],
  ['Shift+R', 'Rotate back'],
  ['Del', 'Delete'],
  ['Esc', 'Stop'],
]

// The guide for the tool in use, above the palette
function ToolGuide({ tool }) {
  const guide = TOOL_GUIDE[tool] ?? TOOL_GUIDE.select
  const Icon = guide.icon
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Icon className="w-4 h-4 text-slate-400" aria-hidden="true" />
        <h4 className="font-semibold">{guide.title}</h4>
      </div>
      <p className="text-sm text-slate-400">{guide.text}</p>
    </div>
  )
}

// The panel's answer when nothing is selected: an icon and a hint, not an empty box
function NoSelection() {
  return (
    <div className="flex flex-col items-center justify-center text-center gap-2 py-10">
      <span className="w-10 h-10 rounded-full bg-slate-800 flex items-center justify-center">
        <Info className="w-5 h-5 text-slate-400" aria-hidden="true" />
      </span>
      <p className="text-sm font-semibold text-slate-200">Nothing selected</p>
      <p className="text-xs text-slate-400 max-w-[14rem]">Click a piece or a track on the layout to see its properties here.</p>
    </div>
  )
}

// What to do first on an empty layout. It sits over the canvas and goes once something is drawn
function EmptyStart({ onDraw, onPlace, onImport }) {
  return (
    <div className="absolute inset-0 flex items-center justify-center p-4 pointer-events-none">
      <div className="pointer-events-auto w-full max-w-md rounded-xl border border-slate-700 bg-slate-900/95 p-6 text-center shadow-2xl">
        <span className="mx-auto mb-3 flex w-12 h-12 items-center justify-center rounded-xl bg-sky-600">
          <Spline className="w-6 h-6 text-white" aria-hidden="true" />
        </span>
        <h3 className="text-lg font-semibold">Start the track plan</h3>
        <p className="mt-1 text-sm text-slate-400">Draw track with the mouse, place pieces from the library, or import an AnyRail file. Save keeps your changes.</p>
        <div className="mt-5 flex flex-wrap justify-center gap-2">
          <button onClick={onDraw} className="px-4 py-2 rounded-md bg-blue-600 text-sm font-semibold hover:bg-blue-500">
            Draw track
          </button>
          <button onClick={onPlace} className="px-4 py-2 rounded-md bg-slate-700 text-sm hover:bg-slate-600">
            Place a piece
          </button>
          <button onClick={onImport} className="flex items-center gap-1.5 px-4 py-2 rounded-md bg-slate-700 text-sm hover:bg-slate-600">
            <Upload className="w-4 h-4" aria-hidden="true" />
            Import AnyRail
          </button>
        </div>
      </div>
    </div>
  )
}


export default function LayoutEditor() {
  const editor = useLayoutEditor()
  const live = useLiveTrackside()
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

  const selected = Boolean(editor.selectedItem || editor.selectedTrack || editor.selectedPlaced)
  const empty = editor.tracks.length === 0 && editor.items.length === 0

  return (
    <div className="flex flex-col gap-3">
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
        <EditDialog title="Import AnyRail" subtitle="Read a track plan from an AnyRail file" icon={Upload} accent="bg-sky-500" busy={editor.busy} onClose={() => setImporting(false)}>
          <AnyRailImport
            busy={editor.busy}
            report={editor.importReport}
            onImport={editor.importAnyRail}
            onClose={() => setImporting(false)}
          />
        </EditDialog>
      )}

      {editor.error && (
        <div className="rounded-lg border border-red-700 bg-red-900/60 px-4 py-2">
          <p className="text-red-200 text-sm">{editor.error}</p>
        </div>
      )}
      {editor.message && !editor.error && (
        <div className="rounded-lg border border-green-700 bg-green-900/60 px-4 py-2">
          <p className="text-green-200 text-sm">{editor.message}</p>
        </div>
      )}

      {editor.layout ? (
        <div className="grid gap-3 lg:grid-cols-[15rem_minmax(0,1fr)_18rem]">
          {/* Palette: the pieces to place, or the guide to the tool in use */}
          <aside className="rounded-xl border border-slate-800 bg-slate-900 p-3 lg:max-h-[calc(100vh-14rem)] lg:overflow-y-auto">
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
              <ToolGuide tool={editor.tool} />
            )}
          </aside>

          {/* The workspace: the layout is a sheet on a darker desk, and the status line says what the tool does */}
          <section className="flex min-w-0 flex-col overflow-hidden rounded-xl border border-slate-800 bg-slate-950">
            <div className="relative min-h-[26rem] flex-1">
              <div className="p-2">
                <LayoutCanvas
                  liveOccupied={live.occupied}
                  liveAspects={live.aspects}
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
              </div>
              {empty && (
                <EmptyStart
                  onDraw={() => editor.chooseTool('track')}
                  onPlace={() => editor.chooseTool('piece')}
                  onImport={() => setImporting(true)}
                />
              )}
            </div>
            <footer className="flex flex-wrap items-center gap-x-5 gap-y-1 border-t border-slate-800 bg-slate-900 px-4 py-2 text-xs text-slate-400">
              <span className="flex items-center gap-3">
                <span><span className="text-orange-500">●</span> open end</span>
                <span><span className="text-green-500">●</span> joined</span>
              </span>
              <span className="flex-1 min-w-[12rem] text-slate-300">{HINTS[editor.tool] || HINTS.select}</span>
              <span className="flex items-center gap-3">
                {SHORTCUTS.map(([key, text]) => (
                  <span key={key} className="flex items-center gap-1">
                    <kbd className="rounded border border-slate-700 bg-slate-800 px-1.5 py-0.5 font-mono text-[11px] text-slate-200">{key}</kbd>
                    {text}
                  </span>
                ))}
              </span>
            </footer>
          </section>

          {/* Properties of what is selected */}
          <aside className="rounded-xl border border-slate-800 bg-slate-900 p-4 lg:max-h-[calc(100vh-14rem)] lg:overflow-y-auto">
            <h4 className="mb-3 font-semibold">Properties</h4>
            {selected ? (
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
            ) : (
              <NoSelection />
            )}
          </aside>
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-slate-700 bg-slate-900 p-12 text-center">
          <p className="text-slate-300">{editor.layouts.length === 0 ? 'No layouts yet' : 'Choose a layout to edit'}</p>
          <p className="text-sm text-slate-400">
            {editor.layouts.length === 0 ? 'Use the + beside the layout tabs to create one.' : 'Pick a layout from the tabs above.'}
          </p>
        </div>
      )}
    </div>
  )
}
