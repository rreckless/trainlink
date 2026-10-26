import { Boxes, Cpu, Gauge, GitBranch, Layers, PenTool, Route, Settings, Split, Stethoscope, TrainFront } from 'lucide-react'

// The pages, grouped as they appear in the side navigation. Colour is the accent of the page's icon tile
export const NAV_GROUPS = [
  {
    title: 'Drive',
    items: [
      { id: 'control', label: 'Control', icon: Gauge, accent: 'bg-amber-500', subtitle: 'Drive locomotives and set their functions' },
      { id: 'locomotives', label: 'Locomotives', icon: TrainFront, accent: 'bg-violet-500', subtitle: 'The roster, pictures, and function labels' },
      { id: 'decoder', label: 'Decoder CVs', icon: Cpu, accent: 'bg-indigo-500', subtitle: 'Read and write decoder CVs on the programming track or the main track' },
      { id: 'consists', label: 'Consists', icon: Layers, accent: 'bg-purple-500', subtitle: 'Group locomotives under one consist address' },
    ],
  },
  {
    title: 'Layout',
    items: [
      { id: 'switchboard', label: 'Switchboard', icon: GitBranch, accent: 'bg-blue-500', subtitle: 'Throw turnouts and watch the track' },
      { id: 'blocks', label: 'Blocks', icon: Boxes, accent: 'bg-cyan-500', subtitle: 'Block properties, and what each block is linked to' },
      { id: 'turnouts', label: 'Turnouts', icon: Split, accent: 'bg-blue-400', subtitle: 'Every turnout on every layout, and which have an address' },
      { id: 'routes', label: 'Routes', icon: Route, accent: 'bg-teal-500', subtitle: 'Routes through the blocks of a layout' },
      { id: 'editor', label: 'Layout Editor', icon: PenTool, accent: 'bg-sky-500', subtitle: 'Draw the track and place its pieces' },
    ],
  },
  {
    title: 'System',
    items: [
      { id: 'health', label: 'Health', icon: Stethoscope, accent: 'bg-emerald-500', subtitle: 'What is missing before automatic driving works' },
      { id: 'settings', label: 'Settings', icon: Settings, accent: 'bg-slate-500', subtitle: 'The Z21 connection and the app' },
    ],
  },
]

export const PAGES = NAV_GROUPS.flatMap((group) => group.items)

// What the Z21's track state means to the user: a label, and the colour of its dot
const TRACK_STATES = {
  disconnected: { text: 'Z21 not connected', dot: 'bg-slate-500' },
  lost: { text: 'Z21 not responding', dot: 'bg-red-500' },
  unknown: { text: 'Checking track power', dot: 'bg-amber-500' },
  short_circuit: { text: 'Short circuit', dot: 'bg-red-500' },
  track_power_off: { text: 'Track power off', dot: 'bg-amber-500' },
  ready: { text: 'Power on', dot: 'bg-green-500' },
}

export const describeTrack = (state) => TRACK_STATES[state] ?? TRACK_STATES.disconnected
