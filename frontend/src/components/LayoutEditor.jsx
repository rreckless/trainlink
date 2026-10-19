import { useStore } from '../store'

export default function LayoutEditor() {
  const layout = useStore((state) => state.layout)
  const editMode = useStore((state) => state.editMode)
  const setEditMode = useStore((state) => state.setEditMode)

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4">
        <div className="flex justify-between items-center mb-4">
          <h3 className="text-lg font-semibold">Layout Editor</h3>
          <button
            onClick={() => setEditMode(!editMode)}
            className={`px-4 py-2 rounded transition-colors ${
              editMode
                ? 'bg-blue-600 hover:bg-blue-700'
                : 'bg-slate-700 hover:bg-slate-600'
            }`}
          >
            {editMode ? 'Exit Edit Mode' : 'Enter Edit Mode'}
          </button>
        </div>

        {layout ? (
          <div className="bg-slate-900 rounded-lg p-8">
            <div className="w-full h-96 bg-slate-700 rounded border-2 border-slate-600 flex items-center justify-center">
              <p className="text-slate-400">
                {editMode ? 'Layout editing canvas' : 'Layout preview'}
              </p>
            </div>
          </div>
        ) : (
          <div className="bg-slate-900 rounded-lg p-8 text-center">
            <p className="text-slate-400 mb-4">No layout loaded</p>
            <button className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700">
              Create New Layout
            </button>
          </div>
        )}
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="bg-slate-800 rounded-lg p-4">
          <h4 className="font-semibold mb-3">Tools</h4>
          <div className="space-y-2">
            <button className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left">
              Track
            </button>
            <button className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left">
              Block
            </button>
            <button className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left">
              Signal
            </button>
            <button className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 text-left">
              Feedback
            </button>
          </div>
        </div>

        <div className="bg-slate-800 rounded-lg p-4">
          <h4 className="font-semibold mb-3">Properties</h4>
          <div className="space-y-2 text-sm">
            <p className="text-slate-400">Select an element to edit</p>
          </div>
        </div>
      </div>
    </div>
  )
}
