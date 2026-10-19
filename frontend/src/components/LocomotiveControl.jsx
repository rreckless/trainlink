import { useState } from 'react'
import { useStore } from '../store'
import { refreshZ21Status } from '../hooks/useZ21Status'

export default function LocomotiveControl() {
  // Z21 state is polled once in App and shared through the store
  const z21 = useStore((state) => state.z21)
  // disconnected | unknown | short_circuit | track_power_off | ready
  const trackState = z21.state
  const z21Connected = z21.connected
  const controlsEnabled = trackState === 'ready'
  const [selectedAddress, setSelectedAddress] = useState(3)
  const [speed, setSpeed] = useState(0)
  const [direction, setDirection] = useState('stop')
  const [isLoading, setIsLoading] = useState(false)
  const [feedback, setFeedback] = useState('')
  const [error, setError] = useState('')

  const resumeTrackPower = async () => {
    setIsLoading(true)
    setError('')
    setFeedback('')

    try {
      const response = await fetch('http://localhost:8000/api/z21/track-power/on', {
        method: 'POST',
      })
      const data = await response.json()

      if (response.ok) {
        setFeedback('Track power on sent. Waiting for the Z21 to confirm.')
        refreshZ21Status()
      } else {
        setError(data.detail || 'Failed to resume track power')
      }
    } catch (error) {
      setError(error.message)
    } finally {
      setIsLoading(false)
    }
  }

  const sendSpeed = async (newSpeed, newDirection) => {
    if (!z21Connected) {
      setError('Z21 not connected')
      return
    }
    if (!controlsEnabled) {
      setError(`Track is not ready (${trackState}). Locomotive commands are blocked`)
      return
    }

    setIsLoading(true)
    setError('')
    setFeedback('')

    try {
      const directionBool =
        newDirection === 'forward' ? true : newDirection === 'reverse' ? false : true

      const response = await fetch(
        'http://localhost:8000/api/z21/loco/speed',
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            address: selectedAddress,
            speed: newSpeed,
            direction: directionBool,
          }),
        }
      )

      const data = await response.json()

      if (response.ok) {
        setSpeed(newSpeed)
        setDirection(newDirection)
        setFeedback(
          `Locomotive ${selectedAddress}: Speed ${newSpeed}, Direction ${newDirection}`
        )
      } else {
        setError(data.detail || 'Failed to send command')
      }
    } catch (error) {
      setError(error.message)
    } finally {
      setIsLoading(false)
    }
  }

  const handleSpeedChange = (e) => {
    const newSpeed = parseInt(e.target.value)
    sendSpeed(newSpeed, direction)
  }

  const handleDirectionChange = (newDirection) => {
    const newSpeed = newDirection === 'stop' ? 0 : speed
    sendSpeed(newSpeed, newDirection)
  }

  const handleEmergencyStop = async () => {
    setIsLoading(true)
    setError('')
    setFeedback('')

    try {
      const response = await fetch(
        'http://localhost:8000/api/z21/emergency-stop',
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ confirm: true }),
        }
      )

      const data = await response.json()

      if (response.ok) {
        setSpeed(0)
        setDirection('stop')
        setFeedback('🚨 EMERGENCY STOP - All locomotives stopped!')
      } else {
        setError(data.detail || 'Failed to send emergency stop')
      }
    } catch (error) {
      setError(error.message)
    } finally {
      setIsLoading(false)
    }
  }

  return (
    <div className="space-y-4">
      {/* Connection and Track Status */}
      <div
        className={`rounded-lg p-4 ${
          trackState === 'ready'
            ? 'bg-green-900 border border-green-700'
            : trackState === 'track_power_off' || trackState === 'unknown'
            ? 'bg-amber-900 border border-amber-700'
            : 'bg-red-900 border border-red-700'
        }`}
      >
        <div className="flex items-center gap-2">
          <div
            className={`w-3 h-3 rounded-full ${
              trackState === 'ready' ? 'bg-green-500 animate-pulse' : 'bg-red-500'
            }`}
          ></div>
          <span className="text-slate-100">
            {trackState === 'disconnected' && 'Z21 Disconnected'}
            {trackState === 'unknown' && 'Z21 Connected. Checking track status'}
            {trackState === 'short_circuit' && '⚡ Short Circuit: Track Power Off'}
            {trackState === 'track_power_off' && 'Z21 Connected: Track Power Off'}
            {trackState === 'ready' && 'Z21 Connected'}
          </span>
        </div>
        <p className="text-xs mt-1 opacity-75">
          {trackState === 'disconnected' && 'Connect Z21 in Settings to control locomotives'}
          {trackState === 'unknown' && 'Waiting for the Z21 to report its track state'}
          {trackState === 'short_circuit' &&
            'The Z21 reports a short circuit. Fix the short, then resume track power.'}
          {trackState === 'track_power_off' && 'Resume track power to run locomotives'}
          {trackState === 'ready' && 'Ready to control locomotives'}
        </p>

        {z21Connected && trackState !== 'ready' && trackState !== 'unknown' && (
          <button
            onClick={resumeTrackPower}
            disabled={isLoading || trackState === 'short_circuit'}
            className="mt-3 w-full py-2 bg-slate-700 rounded font-semibold hover:bg-slate-600 transition-colors disabled:opacity-50"
          >
            {trackState === 'short_circuit'
              ? 'Resume Track Power (fix short first)'
              : 'Resume Track Power'}
          </button>
        )}
      </div>

      {/* Main Control Panel */}
      {z21Connected && (
        <div className="bg-slate-800 rounded-lg p-4 space-y-4">
          {/* Locomotive Selection */}
          <div>
            <label className="block text-sm text-slate-300 mb-2">
              Select Locomotive (DCC Address)
            </label>
            <input
              type="number"
              min="1"
              max="9999"
              value={selectedAddress}
              onChange={(e) => {
                setSelectedAddress(parseInt(e.target.value))
                setSpeed(0)
                setDirection('stop')
              }}
              disabled={isLoading}
              className="w-full px-4 py-2 bg-slate-700 rounded border border-slate-600 text-white disabled:opacity-50"
              placeholder="Enter locomotive address (1-9999)"
            />
            <p className="text-xs text-slate-400 mt-1">
              Controlling: Locomotive {selectedAddress}
            </p>
          </div>

          {/* Speed Control */}
          <div>
            <div className="flex justify-between items-center mb-2">
              <label className="text-sm text-slate-300">Speed</label>
              <span className="text-lg font-bold text-blue-400">{speed}</span>
            </div>
            <input
              type="range"
              min="0"
              max="126"
              value={speed}
              onChange={handleSpeedChange}
              disabled={isLoading || !controlsEnabled || direction === 'stop'}
              className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer disabled:opacity-50"
            />
            <div className="flex justify-between text-xs text-slate-400 mt-1">
              <span>Stop (0)</span>
              <span>Max (126)</span>
            </div>
          </div>

          {/* Direction Control */}
          <div>
            <label className="block text-sm text-slate-300 mb-2">Direction</label>
            <div className="grid grid-cols-3 gap-2">
              <button
                onClick={() => handleDirectionChange('reverse')}
                disabled={isLoading || !controlsEnabled}
                className={`py-3 rounded font-semibold transition-colors ${
                  direction === 'reverse'
                    ? 'bg-orange-600 text-white'
                    : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                } disabled:opacity-50`}
              >
                ⬅️ Reverse
              </button>

              <button
                onClick={() => handleDirectionChange('stop')}
                disabled={isLoading || !controlsEnabled}
                className={`py-3 rounded font-semibold transition-colors ${
                  direction === 'stop'
                    ? 'bg-red-600 text-white'
                    : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                } disabled:opacity-50`}
              >
                ⏸️ Stop
              </button>

              <button
                onClick={() => handleDirectionChange('forward')}
                disabled={isLoading || !controlsEnabled}
                className={`py-3 rounded font-semibold transition-colors ${
                  direction === 'forward'
                    ? 'bg-green-600 text-white'
                    : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                } disabled:opacity-50`}
              >
                Forward ➡️
              </button>
            </div>
          </div>

          {/* Emergency Stop */}
          <button
            onClick={handleEmergencyStop}
            disabled={isLoading}
            className="w-full py-4 bg-red-700 hover:bg-red-800 text-white font-bold rounded-lg text-lg transition-colors disabled:opacity-50 border-2 border-red-600"
          >
            🚨 EMERGENCY STOP 🚨
          </button>

          {/* Feedback Messages */}
          {feedback && (
            <div className="bg-green-900 border border-green-700 rounded p-3">
              <p className="text-green-200 text-sm">{feedback}</p>
            </div>
          )}

          {error && (
            <div className="bg-red-900 border border-red-700 rounded p-3">
              <p className="text-red-200 text-sm">⚠️ {error}</p>
            </div>
          )}
        </div>
      )}

      {/* Multiple Locomotives Example */}
      {z21Connected && (
        <div className="bg-slate-800 rounded-lg p-4">
          <h3 className="text-lg font-semibold mb-3">Quick Presets</h3>
          <div className="space-y-2">
            <button
              onClick={() => setSelectedAddress(3)}
              className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left transition-colors"
            >
              🚂 Locomotive 3
            </button>
            <button
              onClick={() => setSelectedAddress(4)}
              className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left transition-colors"
            >
              🚂 Locomotive 4
            </button>
            <button
              onClick={() => setSelectedAddress(5)}
              className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left transition-colors"
            >
              🚂 Locomotive 5
            </button>
          </div>
          <p className="text-xs text-slate-400 mt-3">
            Click to quickly select a locomotive, or enter any address above
          </p>
        </div>
      )}
    </div>
  )
}
