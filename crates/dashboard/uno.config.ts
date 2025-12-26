import { defineConfig, presetUno, presetAttributify } from 'unocss'

export default defineConfig({
  presets: [
    presetUno(),
    presetAttributify(),
  ],
  theme: {
    colors: {
      primary: '#3b82f6',
      secondary: '#64748b',
      success: '#22c55e',
      warning: '#f59e0b',
      danger: '#ef4444',
    },
  },
  shortcuts: {
    'btn': 'px-4 py-2 rounded cursor-pointer inline-block transition-colors',
    'btn-primary': 'btn bg-primary text-white hover:bg-blue-600',
    'btn-secondary': 'btn bg-secondary text-white hover:bg-slate-600',
    'btn-danger': 'btn bg-danger text-white hover:bg-red-600',
    'card': 'bg-white rounded-lg shadow p-4',
    'input': 'border border-gray-300 rounded px-3 py-2 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent',
  },
})
