import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './app'
import './design-system/styles.css'

const appearanceQuery = window.matchMedia('(prefers-color-scheme: light)')
const applyAppearance = () => {
  if (appearanceQuery.matches) {
    document.documentElement.dataset.appearance = 'light'
  } else {
    delete document.documentElement.dataset.appearance
  }
}
applyAppearance()
appearanceQuery.addEventListener('change', applyAppearance)

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
