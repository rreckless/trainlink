import { useEffect, useMemo, useState } from 'react'
import { confirmAction } from '../services/dialogs'
import { layoutApi } from '../services/layoutApi'
import { routesApi } from '../services/routesApi'
import { tracksideApi } from '../services/tracksideApi'

// Routes: the blocks a train goes through, in order. Automatic driving that runs them is not built yet
export default function RouteManager() {
  const [layouts, setLayouts] = useState([])
  const [layoutId, setLayoutId] = useState(null)
  const [blocks, setBlocks] = useState([])
  const [routes, setRoutes] = useState([])
  const [selectedId, setSelectedId] = useState(null)
  const [draft, setDraft] = useState(null)
  const [pick, setPick] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')

  useEffect(() => {
    layoutApi
      .listLayouts()
      .then((list) => {
        setLayouts(list)
        if (list.length > 0) setLayoutId(list[0].id)
      })
      .catch((err) => setError(`Could not load layouts: ${err.message}`))
    tracksideApi
      .blocks()
      .then(setBlocks)
      .catch((err) => setError(`Could not load blocks: ${err.message}`))
  }, [])

  const refreshRoutes = (id) =>
    routesApi
      .list(id)
      .then(setRoutes)
      .catch((err) => setError(`Could not load routes: ${err.message}`))

  useEffect(() => {
    if (layoutId === null) return
    setSelectedId(null)
    setDraft(null)
    refreshRoutes(layoutId)
  }, [layoutId])

  const blockById = useMemo(() => Object.fromEntries(blocks.map((block) => [block.id, block])), [blocks])
  const layoutBlocks = blocks.filter((block) => block.layout_id === layoutId)
  const selectedRoute = routes.find((route) => route.id === selectedId) ?? null

  // Choosing a route, or New, loads its steps into the editor
  const choose = (route) => {
    setSelectedId(route ? route.id : 'new')
    setDraft({
      name: route ? route.name : '',
      enabled: route ? route.enabled : true,
      steps: route ? [...route.block_sequence] : [],
    })
    setMessage('')
    setError('')
  }

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

  const move = (index, delta) =>
    setDraft((prev) => {
      const steps = [...prev.steps]
      const target = index + delta
      if (target < 0 || target >= steps.length) return prev
      ;[steps[index], steps[target]] = [steps[target], steps[index]]
      return { ...prev, steps }
    })

  const removeStep = (index) => setDraft((prev) => ({ ...prev, steps: prev.steps.filter((_, position) => position !== index) }))

  const addStep = () => {
    const id = Number(pick)
    if (!pick || draft.steps.includes(id)) return
    setDraft((prev) => ({ ...prev, steps: [...prev.steps, id] }))
    setPick('')
  }

  const save = async () => {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const body = { name: draft.name.trim(), enabled: draft.enabled, block_sequence: draft.steps }
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

  const available = layoutBlocks.filter((block) => !draft?.steps.includes(block.id))

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
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

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
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <label className="space-y-1 text-sm text-slate-300">
                Name
                <input
                  value={draft.name}
                  onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                  disabled={busy}
                  className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
                />
              </label>
              <label className="flex items-center gap-2 text-sm text-slate-300 self-end">
                <input
                  type="checkbox"
                  checked={draft.enabled}
                  onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })}
                  disabled={busy}
                />
                Enabled
              </label>
            </div>

            <div className="space-y-2">
              <h4 className="font-semibold">Blocks, in order</h4>
              {draft.steps.length === 0 ? (
                <p className="text-sm text-slate-400">No blocks yet. Add the first block below.</p>
              ) : (
                <ol className="space-y-1">
                  {draft.steps.map((id, index) => {
                    const block = blockById[id]
                    const gap = unlinked.includes(index)
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
                        {gap && (
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

            <div className="flex flex-wrap items-center gap-3">
              <button
                onClick={save}
                disabled={busy || draft.name.trim() === '' || draft.steps.length === 0}
                className="px-4 py-2 bg-green-600 rounded font-semibold hover:bg-green-700 disabled:opacity-50"
              >
                Save route
              </button>
              {selectedRoute && (
                <button onClick={remove} disabled={busy} className="px-4 py-2 bg-slate-700 rounded hover:bg-red-800 disabled:opacity-50">
                  Delete route
                </button>
              )}
              <button disabled title="Automatic driving is not built yet" className="px-4 py-2 bg-slate-700 rounded opacity-50">
                Run (not built yet)
              </button>
              {message && <span className="text-sm text-green-300">{message}</span>}
            </div>
          </>
        )}
      </div>
    </div>
  )
}
