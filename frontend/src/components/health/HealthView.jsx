import { useCallback, useEffect, useState } from 'react'
import { useStore } from '../../store'
import { overviewApi } from '../../services/overviewApi'
import { layoutApi } from '../../services/layoutApi'

// What is missing before the project is ready, with a button to the page where each item is fixed
export default function HealthView() {
  const setActiveTab = useStore((state) => state.setActiveTab)
  const [health, setHealth] = useState(null)
  const [error, setError] = useState('')
  const [checkedAt, setCheckedAt] = useState(null)

  const check = useCallback(
    () =>
      overviewApi
        .health()
        .then((result) => {
          setHealth(result)
          setError('')
          setCheckedAt(new Date())
        })
        .catch((err) => setError(`Could not run the check: ${err.message}`)),
    []
  )

  useEffect(() => {
    check()
  }, [check])

  // The import review is kept on each layout, so clearing it is a save of that layout
  const clearLegs = async (layoutId) => {
    try {
      const layout = await layoutApi.getLayout(layoutId)
      await layoutApi.updateLayout(layoutId, { elements: { ...layout.elements, no_track_legs: [] } })
      await check()
    } catch (err) {
      setError(`Could not clear the review: ${err.message}`)
    }
  }

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 flex flex-wrap items-center gap-3">
        <div className="flex-1">
          <h3 className="text-lg font-semibold">Project health</h3>
          <p className="text-sm text-slate-400">
            {health
              ? health.counts.missing === 0
                ? 'Nothing is missing'
                : `${health.counts.missing} ${health.counts.missing === 1 ? 'thing' : 'things'} to do before automatic driving works`
              : 'Checking the project'}
          </p>
          {checkedAt && <p className="text-xs text-slate-500">Checked at {checkedAt.toLocaleTimeString()}</p>}
        </div>
        <button onClick={check} className="px-4 py-2 bg-slate-700 rounded hover:bg-slate-600">
          Check again
        </button>
      </div>

      {error && (
        <div className="bg-red-900 border border-red-700 rounded p-3">
          <p className="text-red-200 text-sm">{error}</p>
        </div>
      )}

      {health && (
        <>
          <section className="bg-slate-800 rounded-lg p-4 space-y-2">
            <div className="flex items-center justify-between">
              <h4 className="font-semibold">
                Blocks without a sensor <span className="text-sm text-slate-400">({health.blocks_without_sensor.length})</span>
              </h4>
              <button onClick={() => setActiveTab('switchboard')} className="px-3 py-1 text-sm bg-slate-700 rounded hover:bg-slate-600">
                Open Switchboard
              </button>
            </div>
            <p className="text-xs text-slate-400">
              Without a sensor, TrainLink cannot see a train in the block, so automatic driving cannot stop there.
            </p>
            {health.blocks_without_sensor.length === 0 ? (
              <p className="text-sm text-green-300">Every block has a sensor</p>
            ) : (
              <ul className="space-y-1">
                {health.blocks_without_sensor.map((block) => (
                  <li key={block.id} className="text-sm bg-slate-700 rounded px-3 py-2">
                    {block.name}
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="bg-slate-800 rounded-lg p-4 space-y-2">
            <div className="flex items-center justify-between">
              <h4 className="font-semibold">
                Turnouts without an address <span className="text-sm text-slate-400">({health.turnouts_without_address.length})</span>
              </h4>
              <button onClick={() => setActiveTab('editor')} className="px-3 py-1 text-sm bg-slate-700 rounded hover:bg-slate-600">
                Open Layout Editor
              </button>
            </div>
            <p className="text-xs text-slate-400">TrainLink cannot throw a turnout without a DCC address.</p>
            {health.turnouts_without_address.length === 0 ? (
              <p className="text-sm text-green-300">Every turnout has an address</p>
            ) : (
              <ul className="space-y-1">
                {health.turnouts_without_address.map((turnout, index) => (
                  <li key={`${turnout.layout_id}-${index}`} className="text-sm bg-slate-700 rounded px-3 py-2">
                    {turnout.name} <span className="text-xs text-slate-400">on {turnout.layout_name}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="bg-slate-800 rounded-lg p-4 space-y-3">
            <h4 className="font-semibold">
              Turnout legs with no track <span className="text-sm text-slate-400">({health.legs_without_track.length})</span>
            </h4>
            <p className="text-xs text-slate-400">
              Found by the AnyRail import: a turnout leg that ends at a point no other track touches. Connect the track or remove the leg, then clear the review.
            </p>
            {health.legs_without_track.length === 0 ? (
              <p className="text-sm text-green-300">Every turnout leg has track on the other side</p>
            ) : (
              [...new Set(health.legs_without_track.map((leg) => leg.layout_id))].map((layoutId) => {
                const legs = health.legs_without_track.filter((leg) => leg.layout_id === layoutId)
                return (
                  <div key={layoutId} className="space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-sm">{legs[0].layout}</span>
                      <button onClick={() => clearLegs(layoutId)} className="px-3 py-1 text-sm bg-slate-700 rounded hover:bg-slate-600">
                        Clear review
                      </button>
                    </div>
                    <ul className="space-y-1">
                      {legs.map((leg, index) => (
                        <li key={`${leg.turnout}-${leg.leg}-${index}`} className="text-sm bg-slate-700 rounded px-3 py-2">
                          {leg.turnout} leg {leg.leg} <span className="text-xs text-slate-400">ends at {leg.point}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                )
              })
            )}
          </section>
        </>
      )}
    </div>
  )
}
