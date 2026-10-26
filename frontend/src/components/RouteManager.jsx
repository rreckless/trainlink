import { useEffect, useMemo, useState } from 'react'
import { confirmAction } from '../services/dialogs'
import { layoutApi } from '../services/layoutApi'
import { locomotiveApi } from '../services/locomotiveApi'
import { routesApi } from '../services/routesApi'
import { tracksideApi } from '../services/tracksideApi'
import { useLiveTrackside } from '../hooks/useLiveTrackside'
import { fromServer } from '../layout/model'
import LayoutCanvas from './layout/LayoutCanvas'

const STEPS = ['Blocks', 'Options', 'Review']
const REFRESH_WHILE_RUNNING_MS = 1000
const noop = () => {}

// The draft a route is edited in: the blocks in order, and for each block an optional speed and dwell time
function draftFrom(route) {
  if (!route) return { name: '', enabled: true, autoReturn: false, steps: [], speeds: [], dwells: [] }
  const entries = (list) => route.block_sequence.map((_, index) => list?.[index] ?? '')
  return {
    name: route.name,
    enabled: route.enabled,
    autoReturn: route.auto_return,
    steps: [...route.block_sequence],
    speeds: entries(route.speed_profile),
    dwells: entries(route.dwell_times),
  }
}

// Build the request body from the draft. A list with no value in it is sent as null
function bodyFrom(draft) {
  const list = (values, convert) => (values.some((value) => value !== '') ? values.map((value) => (value === '' ? null : convert(value))) : null)
  return {
    name: draft.name.trim(),
    enabled: draft.enabled,
    auto_return: draft.autoReturn,
    block_sequence: draft.steps,
    speed_profile: list(draft.speeds, Number),
    dwell_times: list(draft.dwells, Number),
  }
}

// Problems with the draft that the core would refuse, shown before saving
function problemsWith(draft) {
  const problems = []
  if (draft.name.trim() === '') problems.push('Give the route a name')
  if (draft.steps.length === 0) problems.push('Add at least one block')
  draft.speeds.forEach((value, index) => {
    if (value !== '' && !(Number(value) >= 0 && Number(value) <= 126)) problems.push(`Step ${index + 1}: speed must be 0 to 126`)
  })
  draft.dwells.forEach((value, index) => {
    if (value !== '' && !(Number(value) >= 0 && Number(value) <= 3600)) problems.push(`Step ${index + 1}: dwell must be 0 to 3600 seconds`)
  })
  return problems
}

export default function RouteManager() {
  const [layouts, setLayouts] = useState([])
  const [layoutId, setLayoutId] = useState(null)
  const [blocks, setBlocks] = useState([])
  const [routes, setRoutes] = useState([])
  const [notices, setNotices] = useState([])
  const [locomotives, setLocomotives] = useState([])
  const [runLocomotive, setRunLocomotive] = useState('')
  const [selectedId, setSelectedId] = useState(null)
  const [draft, setDraft] = useState(null)
  const [step, setStep] = useState(0)
  const [pick, setPick] = useState('')
  const [preview, setPreview] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const live = useLiveTrackside()

  useEffect(() => {
    layoutApi
      .listLayouts()
      .then((list) => {
        setLayouts(list)
        if (list.length > 0) setLayoutId(list[0].id)
      })
      .catch((err) => setError(`Could not load layouts: ${err.message}`))
    tracksideApi.blocks().then(setBlocks).catch((err) => setError(`Could not load blocks: ${err.message}`))
    locomotiveApi
      .list()
      .then((list) => {
        setLocomotives(list)
        if (list.length > 0) setRunLocomotive(String(list[0].id))
      })
      .catch(() => {})
  }, [])

  const refreshRoutes = (id) =>
    routesApi
      .notices()
      .then(setNotices)
      .catch(() => {})
      .then(() => routesApi.list(id))
      .then((list) => {
        setRoutes(list)
        return list
      })
      .catch((err) => {
        setError(`Could not load routes: ${err.message}`)
        return []
      })

  useEffect(() => {
    if (layoutId === null) return
    setSelectedId(null)
    setDraft(null)
    refreshRoutes(layoutId)
  }, [layoutId])

  const selectedRoute = routes.find((route) => route.id === selectedId) ?? null
  const running = selectedRoute?.is_running === true

  // While a route runs, its state is read again every second, so the editor shows where the train is
  useEffect(() => {
    if (!running || layoutId === null) return undefined
    const timer = setInterval(() => refreshRoutes(layoutId), REFRESH_WHILE_RUNNING_MS)
    return () => clearInterval(timer)
  }, [running, layoutId]) // eslint-disable-line react-hooks/exhaustive-deps

  const blockById = useMemo(() => Object.fromEntries(blocks.map((block) => [block.id, block])), [blocks])
  const layoutBlocks = blocks.filter((block) => block.layout_id === layoutId)

  // Steps whose block does not link to the next step, as the editor sees them right now
  const unlinked = useMemo(() => {
    if (!draft) return []
    const pairs = []
    for (let index = 0; index + 1 < draft.steps.length; index += 1) {
      const from = blockById[draft.steps[index]]
      if (from && from.next_block_id !== draft.steps[index + 1]) pairs.push(index)
    }
    return pairs
  }, [draft, blockById])

  // Choosing a route, or New, loads its steps into the wizard at its first step
  const choose = (route) => {
    setSelectedId(route ? route.id : 'new')
    setDraft(draftFrom(route))
    setStep(0)
    setPick('')
    setMessage('')
    setError('')
  }

  const setDraftField = (key, value) => setDraft((prev) => ({ ...prev, [key]: value }))

  const move = (index, delta) =>
    setDraft((prev) => {
      const target = index + delta
      if (target < 0 || target >= prev.steps.length) return prev
      const swap = (list) => {
        const next = [...list]
        ;[next[index], next[target]] = [next[target], next[index]]
        return next
      }
      return { ...prev, steps: swap(prev.steps), speeds: swap(prev.speeds), dwells: swap(prev.dwells) }
    })

  const removeStep = (index) =>
    setDraft((prev) => {
      const drop = (list) => list.filter((_, position) => position !== index)
      return { ...prev, steps: drop(prev.steps), speeds: drop(prev.speeds), dwells: drop(prev.dwells) }
    })

  const addStep = () => {
    const id = Number(pick)
    if (!pick || draft.steps.includes(id)) return
    setDraft((prev) => ({ ...prev, steps: [...prev.steps, id], speeds: [...prev.speeds, ''], dwells: [...prev.dwells, ''] }))
    setPick('')
  }

  const setStepValue = (key, index, value) =>
    setDraft((prev) => ({ ...prev, [key]: prev[key].map((item, position) => (position === index ? value : item)) }))

  // The layout the route runs on, drawn for the review step with the route's blocks highlighted
  useEffect(() => {
    if (step !== 2 || layoutId === null) return
    Promise.all([layoutApi.getLayout(layoutId), layoutApi.getTrackLibrary()])
      .then(([layout, library]) => setPreview({ layout, library }))
      .catch(() => setPreview(null))
  }, [step, layoutId])

  const previewItems = useMemo(() => layoutBlocks.map((block) => fromServer('block', block)), [layoutBlocks])
  const libraryById = useMemo(() => Object.fromEntries((preview?.library ?? []).map((piece) => [piece.id, piece])), [preview])

  const problems = draft ? problemsWith(draft) : []

  const save = async () => {
    if (problems.length > 0) return
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const body = bodyFrom(draft)
      if (selectedId === 'new') {
        const created = await routesApi.create({ layout_id: layoutId, ...body })
        setSelectedId(created.id)
        setMessage(`Created ${created.name}`)
      } else {
        await routesApi.update(selectedId, body)
        setMessage(`Saved ${body.name}`)
      }
      await refreshRoutes(layoutId)
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const remove = async () => {
    if (!(await confirmAction(`Delete the route ${selectedRoute.name}?`, { okLabel: 'Delete', danger: true }))) return
    setBusy(true)
    setError('')
    try {
      await routesApi.remove(selectedId)
      setSelectedId(null)
      setDraft(null)
      await refreshRoutes(layoutId)
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const run = async () => {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      await routesApi.run(selectedId, Number(runLocomotive))
      setMessage(`Running ${selectedRoute.name}`)
      await refreshRoutes(layoutId)
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const stop = async () => {
    setBusy(true)
    setError('')
    try {
      await routesApi.stop(selectedId)
      setMessage(`Stopped ${selectedRoute.name}`)
      await refreshRoutes(layoutId)
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const available = layoutBlocks.filter((block) => !draft?.steps.includes(block.id))
  const saved = selectedId !== null && selectedId !== 'new'
  const canRun = saved && selectedRoute?.enabled && !running && !busy && runLocomotive !== ''

  return (
    <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 items-start">
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-lg font-semibold">Routes</h3>
          <button
            onClick={() => choose(null)}
            disabled={layoutId === null}
            className="px-3 py-1 bg-blue-600 rounded text-sm hover:bg-blue-700 disabled:opacity-50"
          >
            New route
          </button>
        </div>
        <select
          value={layoutId ?? ''}
          onChange={(event) => setLayoutId(Number(event.target.value))}
          className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
        >
          {layouts.map((layout) => (
            <option key={layout.id} value={layout.id}>
              {layout.name}
            </option>
          ))}
        </select>
        {routes.length === 0 ? (
          <p className="text-sm text-slate-400">No routes on this layout yet</p>
        ) : (
          <ul className="space-y-1">
            {routes.map((route) => (
              <li key={route.id}>
                <button
                  onClick={() => choose(route)}
                  className={`w-full text-left px-3 py-2 rounded ${
                    route.id === selectedId ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'
                  }`}
                >
                  <span className="font-semibold">{route.name}</span>
                  <span className="block text-xs opacity-80">
                    {route.block_sequence.length} blocks
                    {route.unlinked_steps.length > 0 && ` · ${route.unlinked_steps.length} not linked`}
                    {!route.enabled && ' · disabled'}
                    {route.is_running && ' · running'}
                    {route.waiting_for_block && ' · waiting for a block'}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {notices.length > 0 && (
        <div className="xl:col-span-3 bg-slate-800 rounded-lg p-3">
          <h4 className="font-semibold text-sm mb-1">Automation notices</h4>
          <ul className="text-sm text-amber-200 list-disc pl-5">
            {notices.map((text, index) => (
              <li key={index}>{text}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="xl:col-span-2 bg-slate-800 rounded-lg p-4 space-y-4">
        {error && (
          <div className="bg-red-900 border border-red-700 rounded p-3">
            <p className="text-red-200 text-sm">{error}</p>
          </div>
        )}
        {!draft ? (
          <p className="text-sm text-slate-400">Choose a route, or make a new one.</p>
        ) : (
          <>
            <ol className="flex gap-2 text-sm">
              {STEPS.map((title, index) => (
                <li key={title}>
                  <button
                    onClick={() => setStep(index)}
                    className={`px-3 py-1 rounded ${index === step ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'}`}
                  >
                    {index + 1}. {title}
                  </button>
                </li>
              ))}
            </ol>

            {step === 0 && (
              <div className="space-y-4">
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <label className="space-y-1 text-sm text-slate-300">
                    Name
                    <input
                      value={draft.name}
                      onChange={(event) => setDraftField('name', event.target.value)}
                      disabled={busy}
                      className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
                    />
                  </label>
                  <label className="flex items-center gap-2 text-sm text-slate-300 self-end">
                    <input type="checkbox" checked={draft.enabled} onChange={(event) => setDraftField('enabled', event.target.checked)} disabled={busy} />
                    Enabled
                  </label>
                </div>
                <h4 className="font-semibold">Blocks, in order</h4>
                {draft.steps.length === 0 ? (
                  <p className="text-sm text-slate-400">No blocks yet. Add the first block below.</p>
                ) : (
                  <ol className="space-y-1">
                    {draft.steps.map((id, index) => {
                      const block = blockById[id]
                      return (
                        <li key={`${id}-${index}`} className="space-y-1">
                          <div className="flex items-center gap-2 bg-slate-700 rounded px-3 py-2">
                            <span className="w-6 text-xs text-slate-400">{index + 1}</span>
                            <span className="flex-1">{block ? block.name : `Block ${id}`}</span>
                            <span className={block?.occupied_now ? 'text-red-300 text-sm' : 'text-green-300 text-sm'}>
                              {block ? (block.occupied_now ? 'Occupied' : 'Free') : ''}
                            </span>
                            <button onClick={() => move(index, -1)} disabled={busy || index === 0} className="px-2 text-sm bg-slate-600 rounded disabled:opacity-40">
                              ↑
                            </button>
                            <button
                              onClick={() => move(index, 1)}
                              disabled={busy || index === draft.steps.length - 1}
                              className="px-2 text-sm bg-slate-600 rounded disabled:opacity-40"
                            >
                              ↓
                            </button>
                            <button onClick={() => removeStep(index)} disabled={busy} className="px-2 text-sm bg-slate-600 rounded hover:bg-red-800">
                              Remove
                            </button>
                          </div>
                          {unlinked.includes(index) && (
                            <p className="text-xs text-amber-300 pl-9">
                              {block?.name} is not linked to {blockById[draft.steps[index + 1]]?.name ?? 'the next block'}. Set its next block on the Blocks page
                            </p>
                          )}
                        </li>
                      )
                    })}
                  </ol>
                )}
                <div className="flex gap-2">
                  <select
                    value={pick}
                    onChange={(event) => setPick(event.target.value)}
                    disabled={busy || available.length === 0}
                    className="flex-1 px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
                  >
                    <option value="">{available.length === 0 ? 'No more blocks on this layout' : 'Choose a block'}</option>
                    {available.map((block) => (
                      <option key={block.id} value={block.id}>
                        {block.name}
                      </option>
                    ))}
                  </select>
                  <button onClick={addStep} disabled={busy || !pick} className="px-4 py-2 bg-slate-600 rounded hover:bg-slate-500 disabled:opacity-50">
                    Add block
                  </button>
                </div>
              </div>
            )}

            {step === 1 && (
              <div className="space-y-4">
                <label className="flex items-center gap-2 text-sm text-slate-300">
                  <input type="checkbox" checked={draft.autoReturn} onChange={(event) => setDraftField('autoReturn', event.target.checked)} disabled={busy} />
                  Auto-return: run back along the route when it reaches the end
                </label>
                {draft.steps.length === 0 ? (
                  <p className="text-sm text-slate-400">Add blocks on the first step to set their options.</p>
                ) : (
                  <table className="w-full text-sm">
                    <thead className="text-left text-slate-400">
                      <tr>
                        <th className="py-1">Block</th>
                        <th>Speed (0 to 126, blank for the block&apos;s maximum)</th>
                        <th>Wait here (seconds)</th>
                      </tr>
                    </thead>
                    <tbody>
                      {draft.steps.map((id, index) => (
                        <tr key={`${id}-${index}`} className="border-t border-slate-700">
                          <td className="py-2">
                            {index + 1}. {blockById[id]?.name ?? `Block ${id}`}
                            <span className="text-xs text-slate-400"> · max {blockById[id]?.max_speed ?? '?'}</span>
                          </td>
                          <td>
                            <input
                              type="number"
                              min="0"
                              max="126"
                              value={draft.speeds[index]}
                              onChange={(event) => setStepValue('speeds', index, event.target.value)}
                              disabled={busy}
                              aria-label={`Speed for step ${index + 1}`}
                              className="w-24 px-2 py-1 bg-slate-700 rounded border border-slate-600 text-white"
                            />
                          </td>
                          <td>
                            <input
                              type="number"
                              min="0"
                              max="3600"
                              value={draft.dwells[index]}
                              onChange={(event) => setStepValue('dwells', index, event.target.value)}
                              disabled={busy}
                              aria-label={`Wait for step ${index + 1}`}
                              className="w-24 px-2 py-1 bg-slate-700 rounded border border-slate-600 text-white"
                            />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            )}

            {step === 2 && (
              <div className="space-y-4">
                <div className="bg-slate-700 rounded p-3 text-sm space-y-1">
                  <p>
                    <span className="font-semibold">{draft.name || 'Unnamed route'}</span> · {draft.steps.length} blocks ·{' '}
                    {draft.enabled ? 'enabled' : 'disabled'}
                    {draft.autoReturn ? ' · auto-return' : ''}
                  </p>
                  {unlinked.length > 0 && <p className="text-amber-300">{unlinked.length} step(s) are not linked to the next block</p>}
                </div>
                {problems.length > 0 && (
                  <ul className="text-sm text-red-300 list-disc pl-5">
                    {problems.map((problem) => (
                      <li key={problem}>{problem}</li>
                    ))}
                  </ul>
                )}
                {preview && draft.steps.length > 0 && (
                  <div className="bg-slate-900 rounded p-2 overflow-hidden">
                    <LayoutCanvas
                      liveOccupied={live.occupied}
                      liveAspects={live.aspects}
                      layout={preview.layout}
                      tracks={preview.layout?.elements?.tracks ?? []}
                      pieces={preview.layout?.elements?.track_pieces ?? []}
                      libraryById={libraryById}
                      ghost={null}
                      items={previewItems}
                      highlightedBlocks={draft.steps}
                      tool="operate"
                      selectedKey={null}
                      pendingStart={null}
                      cursor={null}
                      operating={false}
                      turnoutStates={{}}
                      onThrow={noop}
                      onCanvasClick={noop}
                      onCursor={noop}
                      onSelect={noop}
                      onMove={noop}
                    />
                    <p className="text-xs text-slate-400 p-2">The route&apos;s blocks are outlined in gold.</p>
                  </div>
                )}
              </div>
            )}

            <div className="flex flex-wrap items-center gap-3 border-t border-slate-700 pt-4">
              <button onClick={() => setStep(step - 1)} disabled={step === 0 || busy} className="px-4 py-2 bg-slate-700 rounded disabled:opacity-40">
                Back
              </button>
              {step < STEPS.length - 1 && (
                <button onClick={() => setStep(step + 1)} disabled={busy} className="px-4 py-2 bg-slate-600 rounded hover:bg-slate-500">
                  Next
                </button>
              )}
              {step === STEPS.length - 1 && (
                <button
                  onClick={save}
                  disabled={busy || problems.length > 0}
                  className="px-4 py-2 bg-green-600 rounded font-semibold hover:bg-green-700 disabled:opacity-50"
                >
                  Save route
                </button>
              )}
              {saved && (
                <button onClick={remove} disabled={busy || running} className="px-4 py-2 bg-slate-700 rounded hover:bg-red-800 disabled:opacity-50">
                  Delete route
                </button>
              )}
              {message && <span className="text-sm text-green-300">{message}</span>}
            </div>

            {saved && selectedRoute && (
              <div className="bg-slate-700 rounded p-3 space-y-2">
                <h4 className="font-semibold">Run</h4>
                {running ? (
                  <p className="text-sm">
                    Running: the train is in step {selectedRoute.current_block_index + 1} of {selectedRoute.block_sequence.length}
                    {selectedRoute.waiting_for_block && (
                      <span className="text-amber-300"> · waiting for {blockById[selectedRoute.waiting_for_block]?.name ?? 'a block'} to be free</span>
                    )}
                  </p>
                ) : (
                  <p className="text-sm text-slate-400">
                    {selectedRoute.enabled ? 'Ready to run. The locomotive starts in the first block' : 'Enable the route to run it'}
                  </p>
                )}
                <div className="flex flex-wrap items-center gap-2">
                  <select
                    value={runLocomotive}
                    onChange={(event) => setRunLocomotive(event.target.value)}
                    disabled={running || busy}
                    className="px-3 py-2 bg-slate-800 rounded border border-slate-600 text-white"
                  >
                    {locomotives.length === 0 && <option value="">No locomotives</option>}
                    {locomotives.map((loco) => (
                      <option key={loco.id} value={loco.id}>
                        {loco.name}
                      </option>
                    ))}
                  </select>
                  {running ? (
                    <button onClick={stop} disabled={busy} className="px-4 py-2 bg-red-600 rounded font-semibold hover:bg-red-700 disabled:opacity-50">
                      Stop
                    </button>
                  ) : (
                    <button onClick={run} disabled={!canRun} className="px-4 py-2 bg-green-600 rounded font-semibold hover:bg-green-700 disabled:opacity-50">
                      Run
                    </button>
                  )}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
