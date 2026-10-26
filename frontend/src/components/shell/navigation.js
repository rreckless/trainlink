import { Boxes, Cpu, Gauge, GitBranch, Layers, ListChecks, PenTool, Radio, Route, Settings, Split, Stethoscope, TrainFront } from 'lucide-react'

// The side navigation, as LocoLink lays it out: three sections that open and close, and within Basics a group of less
// common pages that is closed until it is opened. Colour is the accent of the page's icon tile
const PAGE = {
  control: { id: 'control', label: 'Control', icon: Gauge, accent: 'bg-amber-500', subtitle: 'Drive locomotives and set their functions' },
  locomotives: { id: 'locomotives', label: 'Locomotives', icon: TrainFront, accent: 'bg-violet-500', subtitle: 'The roster, pictures, and function labels' },
  decoder: { id: 'decoder', label: 'Decoder CVs', icon: Cpu, accent: 'bg-indigo-500', subtitle: 'Read and write decoder CVs on the programming track or the main track' },
  consists: { id: 'consists', label: 'Consists', icon: Layers, accent: 'bg-purple-500', subtitle: 'Group locomotives under one consist address' },
  switchboard: { id: 'switchboard', label: 'Switchboard', icon: GitBranch, accent: 'bg-blue-500', subtitle: 'Throw turnouts and watch the track' },
  blocks: { id: 'blocks', label: 'Blocks', icon: Boxes, accent: 'bg-cyan-500', subtitle: 'Block properties, and what each block is linked to' },
  turnouts: { id: 'turnouts', label: 'Turnouts', icon: Split, accent: 'bg-blue-400', subtitle: 'Every turnout on every layout, and which have an address' },
  routes: { id: 'routes', label: 'Routes', icon: Route, accent: 'bg-teal-500', subtitle: 'Routes through the blocks of a layout' },
  editor: { id: 'editor', label: 'Layout Editor', icon: PenTool, accent: 'bg-sky-500', subtitle: 'Draw the track and place its pieces' },
  setup: { id: 'setup', label: 'Setup', icon: ListChecks, accent: 'bg-green-500', subtitle: 'The steps to a working setup, and which are done' },
  feedback: { id: 'feedback', label: 'Feedback monitor', icon: Radio, accent: 'bg-rose-500', subtitle: 'The 128 sensor addresses, and the latest reports from the Z21' },
  health: { id: 'health', label: 'Health', icon: Stethoscope, accent: 'bg-emerald-500', subtitle: 'What is missing before automatic driving works' },
  settings: { id: 'settings', label: 'Settings', icon: Settings, accent: 'bg-slate-500', subtitle: 'The Z21 connection and the app' },
}

export const NAV_SECTIONS = [
  {
    id: 'basics',
    title: 'Basics',
    items: [PAGE.setup, PAGE.control, PAGE.locomotives, PAGE.turnouts, PAGE.routes, PAGE.decoder],
    more: { title: 'More basic features', items: [PAGE.consists, PAGE.switchboard] },
  },
  {
    id: 'advanced',
    title: 'Advanced',
    items: [PAGE.editor, PAGE.blocks, PAGE.feedback],
    more: null,
  },
  {
    id: 'advanced-more',
    title: 'More advanced features',
    items: [PAGE.health, PAGE.settings],
    more: null,
  },
]

export const PAGES = NAV_SECTIONS.flatMap((section) => [...section.items, ...(section.more?.items ?? [])])

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
