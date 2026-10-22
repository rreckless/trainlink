import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { NATIVE_FILE_PICKER } from '../../services/api'

const DEFAULT_RADIUS_MM = '263.5'
const LIST_LIMIT = 10

function ReportList({ title, items, render }) {
  if (items.length === 0) return null
  return (
    <div>
      <p className="text-slate-300 text-sm font-semibold">
        {title} ({items.length})
      </p>
      <ul className="text-xs text-slate-400 list-disc pl-5">
        {items.slice(0, LIST_LIMIT).map((item, index) => (
          <li key={index}>{render(item)}</li>
        ))}
      </ul>
      {items.length > LIST_LIMIT && (
        <p className="text-xs text-slate-500 pl-5">and {items.length - LIST_LIMIT} more</p>
      )}
    </div>
  )
}

// Pick an AnyRail (JMRI layout-config XML) export and turn it into track pieces in the open layout
export default function AnyRailImport({ busy, report, onImport, onClose }) {
  // The browser's file, on the web. In the desktop app, the core reads the chosen file: { name, xml }
  const [file, setFile] = useState(null)
  const [chosen, setChosen] = useState(null)
  const [radius, setRadius] = useState(DEFAULT_RADIUS_MM)
  const [formError, setFormError] = useState('')
  const fileName = NATIVE_FILE_PICKER ? chosen?.name : file?.name

  // The desktop app's own file dialog. The core reads the file, because the webview cannot read every file it picks
  const chooseInDesktop = async () => {
    setFormError('')
    try {
      const picked = await invoke('pick_anyrail_file')
      if (picked) setChosen(picked)
    } catch (err) {
      setFormError(String(err))
    }
  }

  const submit = async (event) => {
    event.preventDefault()
    setFormError('')
    const radiusMm = Number(radius)
    if (NATIVE_FILE_PICKER ? !chosen : !file) return setFormError('Choose the XML file exported from AnyRail')
    if (!(radiusMm > 0)) return setFormError('The innermost curve radius must be a positive number of mm')
    let xml
    try {
      xml = NATIVE_FILE_PICKER ? chosen.xml : await file.text()
    } catch (err) {
      return setFormError(`Could not read the file: ${err.message}`)
    }
    await onImport(xml, radiusMm)
  }

  const countsByPiece = report
    ? report.track_pieces.reduce((counts, piece) => {
        counts[piece.piece_id] = (counts[piece.piece_id] || 0) + 1
        return counts
      }, {})
    : {}

  return (
    <div className="bg-slate-800 rounded-lg p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h4 className="font-semibold">Import from AnyRail</h4>
        <button onClick={onClose} className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600">
          Close
        </button>
      </div>

      <p className="text-xs text-slate-400">
        In AnyRail, export the layout as JMRI layout-config XML. The import replaces the track pieces in
        this layout, and the layout takes the size of the imported track. Save to keep it.
      </p>

      <form onSubmit={submit} className="grid grid-cols-1 md:grid-cols-3 gap-3 items-end">
        <div className="text-sm text-slate-300">
          <span className="block mb-1">XML file</span>
          {NATIVE_FILE_PICKER ? (
            <button
              type="button"
              onClick={chooseInDesktop}
              disabled={busy}
              className="px-3 py-2 bg-slate-700 rounded text-white hover:bg-slate-600 disabled:opacity-50"
            >
              Choose file…
            </button>
          ) : (
            <label className="inline-block px-3 py-2 bg-slate-700 rounded text-white hover:bg-slate-600 cursor-pointer">
              Choose file…
              <input
                type="file"
                accept=".xml,text/xml,application/xml"
                onChange={(event) => setFile(event.target.files[0] ?? null)}
                className="sr-only"
              />
            </label>
          )}
          <span className="ml-3 text-xs text-slate-400">{fileName || 'No file chosen'}</span>
        </div>
        <label className="text-sm text-slate-300">
          Innermost curve radius (mm)
          <input
            type="number"
            min="1"
            step="0.1"
            required
            value={radius}
            onChange={(event) => setRadius(event.target.value)}
            className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
          />
        </label>
        <button
          type="submit"
          disabled={busy}
          className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {busy ? 'Importing...' : 'Import'}
        </button>
      </form>

      {formError && <p className="text-sm text-red-300">{formError}</p>}

      {report && (
        <div className="space-y-3 border-t border-slate-700 pt-3">
          <p className="text-sm text-slate-200">
            {report.track_pieces.length} pieces, layout {report.width} × {report.height} mm, scale{' '}
            {report.scale_mm_per_pixel} mm per pixel
          </p>
          <ul className="text-xs text-slate-400 list-disc pl-5">
            {Object.entries(countsByPiece).map(([pieceId, count]) => (
              <li key={pieceId}>
                {pieceId}: {count}
              </li>
            ))}
          </ul>

          <ReportList
            title="Joints that don't close"
            items={report.gaps}
            render={(gap) => `${gap.joint}: ${gap.gap_mm} mm`}
          />
          <ReportList
            title="Pieces not placed"
            items={report.unplaced}
            render={(item) => `${item.ident} (${item.kind}): ${item.reason}`}
          />
          <ReportList
            title="Turnouts with the wrong hand"
            items={report.hand_mismatches}
            render={(ident) => ident}
          />
          <ReportList
            title="Curves that could not be confirmed"
            items={report.ambiguous_curves}
            render={(ident) => ident}
          />
          <ReportList
            title="Turnouts placed without an anchor"
            items={report.unanchored_turnouts}
            render={(ident) => ident}
          />
        </div>
      )}
    </div>
  )
}
