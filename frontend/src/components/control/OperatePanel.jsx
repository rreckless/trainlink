import { useCallback, useEffect, useMemo, useState } from 'react'
import LayoutCanvas from '../layout/LayoutCanvas'
import { layoutApi } from '../../services/layoutApi'
import { useTurnoutControl } from '../../hooks/useTurnoutControl'
import { useLiveTrackside } from '../../hooks/useLiveTrackside'

const noop = () => {}

// A layout on the canvas in operate mode. Click a turnout with a DCC address to switch it
export default function OperatePanel() {
  const [layouts, setLayouts] = useState([])
  const [layoutId, setLayoutId] = useState(null)
  const [layout, setLayout] = useState(null)
  const [library, setLibrary] = useState([])
  const [error, setError] = useState('')

  useEffect(() => {
    layoutApi
      .listLayouts()
      .then((list) => {
        setLayouts(list)
        if (list.length > 0) setLayoutId(list[0].id)
      })
      .catch((err) => setError(`Could not load layouts: ${err.message}`))
    layoutApi
      .getTrackLibrary()
      .then(setLibrary)
      .catch((err) => setError(`Could not load the track library: ${err.message}`))
  }, [])

  useEffect(() => {
    if (layoutId === null) return
    layoutApi
      .getLayout(layoutId)
      .then(setLayout)
      .catch((err) => setError(`Could not load the layout: ${err.message}`))
  }, [layoutId])

  const libraryById = useMemo(() => Object.fromEntries(library.map((piece) => [piece.id, piece])), [library])
  const pieces = useMemo(() => layout?.elements?.track_pieces ?? [], [layout])
  const tracks = useMemo(() => layout?.elements?.tracks ?? [], [layout])
  const reportError = useCallback((message) => setError(message), [])
  const { turnoutStates, throwPlaced } = useTurnoutControl(pieces, layout !== null, reportError)
  const live = useLiveTrackside()

  if (layouts.length === 0) {
    return (
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-2">Layout</h3>
        <p className="text-sm text-slate-400">No layouts yet. Create one in the Layout Editor.</p>
      </div>
    )
  }

  return (
    <div className="bg-slate-800 rounded-lg p-4 space-y-3">
      <div className="flex items-center gap-3">
        <h3 className="text-lg font-semibold">Layout</h3>
        <select
          value={layoutId ?? ''}
          onChange={(event) => setLayoutId(Number(event.target.value))}
          className="px-3 py-1 bg-slate-700 rounded border border-slate-600 text-white"
        >
          {layouts.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </div>

      {error && (
        <div className="bg-red-900 border border-red-700 rounded p-2">
          <p className="text-red-200 text-sm">{error}</p>
        </div>
      )}

      {layout && (
        <>
          <LayoutCanvas
            liveOccupied={live.occupied}
            liveAspects={live.aspects}
            layout={layout}
            tracks={tracks}
            pieces={pieces}
            libraryById={libraryById}
            ghost={null}
            items={[]}
            tool="operate"
            selectedKey={null}
            pendingStart={null}
            cursor={null}
            operating
            turnoutStates={turnoutStates}
            onThrow={throwPlaced}
            onCanvasClick={noop}
            onCursor={noop}
            onSelect={noop}
            onMove={noop}
          />
          <p className="text-xs text-slate-500">
            Click a turnout with a DCC address to switch it. Orange means diverging.
          </p>
        </>
      )}
    </div>
  )
}
