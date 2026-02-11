/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/main.tsx - React application entry point
 *
 * This is the top-level entry point for the gamdl-GUI React application.
 * Vite uses this file as the JavaScript module entry (configured in index.html
 * via `<script type="module" src="/src/main.tsx">`).
 *
 * Responsibilities:
 * 1. Import the root `<App />` component (see ./App.tsx)
 * 2. Import global CSS styles (Tailwind directives, CSS custom properties)
 * 3. Locate the root DOM element (`<div id="root">`) in index.html
 * 4. Create a React 18+ concurrent root via `createRoot()` and render the app
 * 5. Wrap the app in `<React.StrictMode>` for development-time safety checks
 *
 * @see {@link https://react.dev/reference/react-dom/client/createRoot} - React createRoot API
 * @see {@link https://react.dev/reference/react/StrictMode} - React StrictMode documentation
 * @see {@link https://v2.tauri.app/start/frontend/} - Tauri 2.0 frontend integration
 * @see {@link https://vite.dev/guide/#index-html-and-project-root} - Vite entry point conventions
 */

/**
 * React core library - provides JSX runtime, hooks, and component primitives.
 * Imported as a namespace to access `React.StrictMode`.
 */
import React from 'react';

/**
 * ReactDOM client API - provides `createRoot()` for React 18+ concurrent rendering.
 * The `/client` sub-path is the React 18 entry point; the legacy `ReactDOM.render()`
 * from `react-dom` is deprecated.
 * @see {@link https://react.dev/reference/react-dom/client} - ReactDOM client APIs
 */
import ReactDOM from 'react-dom/client';

/**
 * Root application component.
 * Handles platform detection, settings initialization, dependency checking,
 * event listeners, and page routing. See ./App.tsx for full documentation.
 */
import App from './App';

/**
 * Global CSS styles imported as a side-effect module.
 * Contains Tailwind CSS directives (@tailwind base/components/utilities),
 * CSS custom properties for theming, and base layout styles.
 * This import must come before any component rendering so styles are available.
 */
import './styles/globals.css';

/**
 * Locate the root DOM mount point.
 * The `<div id="root"></div>` element is defined in the project's index.html
 * and serves as the container for the entire React component tree.
 * Returns `HTMLElement | null` -- null if the element is missing from the DOM.
 */
const rootElement = document.getElementById('root');

/**
 * Guard clause: fail fast if the root element is missing.
 * This would indicate a corrupted or misconfigured index.html file.
 * Throwing here produces a clear error message instead of a silent failure
 * or cryptic "cannot read properties of null" runtime error.
 */
if (!rootElement) {
  throw new Error(
    'Root element not found. Ensure index.html contains a <div id="root"></div> element.'
  );
}

/**
 * Create a React 18 concurrent root and render the application.
 *
 * `createRoot()` enables React 18's concurrent features including:
 * - Automatic batching of state updates
 * - Transitions API support
 * - Suspense for data fetching (future use)
 *
 * `<React.StrictMode>` wraps the entire app and enables development-only checks:
 * - Double-invokes render functions and effects to detect impure renders
 * - Warns about deprecated lifecycle methods and legacy API usage
 * - Verifies components follow rules of hooks
 * - Has NO effect in production builds (zero runtime overhead)
 *
 * @see {@link https://react.dev/reference/react-dom/client/createRoot} - createRoot API
 * @see {@link https://react.dev/reference/react/StrictMode#fixing-bugs-found-by-double-rendering-in-development} - StrictMode double-rendering
 */
ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
