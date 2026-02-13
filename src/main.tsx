/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/main.tsx - React application entry point
 *
 * This is the top-level entry point for the MeedyaDL React application.
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
 * Error boundary component to catch and display React render errors visually.
 * Without this, a crash in any component would unmount the entire React tree
 * and leave the user with a blank/black screen and no indication of what went
 * wrong. This boundary catches the error, logs it to the console, and renders
 * a styled error overlay with the message, name, stack trace, and component
 * stack so the user (or developer) can diagnose the issue.
 *
 * Wrapped around `<App />` in the render call below.
 * @see {@link https://react.dev/reference/react/Component#catching-rendering-errors-with-an-error-boundary}
 */
class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { hasError: boolean; error: Error | null; errorInfo: React.ErrorInfo | null }
> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    this.setState({ errorInfo });
    console.error('ErrorBoundary caught:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          padding: '24px',
          fontFamily: 'monospace',
          fontSize: '13px',
          color: '#ff6b6b',
          backgroundColor: '#1a1a2e',
          height: '100vh',
          overflow: 'auto',
        }}>
          <h1 style={{ fontSize: '18px', marginBottom: '16px', color: '#fff' }}>
            React Error Caught
          </h1>
          <div style={{ marginBottom: '16px' }}>
            <strong style={{ color: '#ffd93d' }}>Error:</strong>{' '}
            {this.state.error?.message}
          </div>
          <div style={{ marginBottom: '16px' }}>
            <strong style={{ color: '#ffd93d' }}>Name:</strong>{' '}
            {this.state.error?.name}
          </div>
          <div style={{ marginBottom: '16px', whiteSpace: 'pre-wrap', fontSize: '11px' }}>
            <strong style={{ color: '#ffd93d' }}>Stack:</strong>{'\n'}
            {this.state.error?.stack}
          </div>
          {this.state.errorInfo && (
            <div style={{ whiteSpace: 'pre-wrap', fontSize: '11px' }}>
              <strong style={{ color: '#ffd93d' }}>Component Stack:</strong>{'\n'}
              {this.state.errorInfo.componentStack}
            </div>
          )}
        </div>
      );
    }
    return this.props.children;
  }
}

/**
 * Global error handlers -- catch errors that escape React's error boundary.
 *
 * React's ErrorBoundary only catches errors during rendering, lifecycle methods,
 * and constructors. These two global handlers cover everything else:
 *
 * 1. `window.onerror` -- synchronous JS runtime errors (e.g., reference errors
 *    in setTimeout callbacks, inline event handlers, or third-party scripts).
 * 2. `unhandledrejection` -- async errors from uncaught promise rejections
 *    (e.g., failed IPC calls where the caller forgot try/catch, or fire-and-forget
 *    async functions that throw).
 *
 * Both handlers surface the error as a visible toast notification so the user
 * knows something went wrong, rather than the app silently misbehaving.
 * The toast is dispatched via Zustand's imperative `getState()` API since these
 * handlers run outside of React's component tree.
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/Window/error_event}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/Window/unhandledrejection_event}
 */

/**
 * Synchronous JS error handler. Catches runtime errors that occur outside of
 * React's render cycle (setTimeout, requestAnimationFrame, inline handlers, etc.).
 * Returns `true` to suppress the default browser error logging (we log ourselves).
 */
window.onerror = (message, source, lineno, colno, error) => {
  const errorMessage = error?.message || String(message);
  console.error('Global error:', errorMessage, { source, lineno, colno, error });

  // Surface the error to the user via toast (lazy-import to avoid circular deps)
  try {
    const { useUiStore } = require('./stores/uiStore');
    useUiStore.getState().addToast(
      `Unexpected error: ${errorMessage}`,
      'error',
      8000,
    );
  } catch {
    // Store not available yet (app still booting) -- console.error above is enough
  }
};

/**
 * Unhandled promise rejection handler. Catches async errors that escape both
 * React's error boundary and component-level try/catch blocks.
 * Surfaces the error as a toast notification so the user sees feedback.
 */
window.addEventListener('unhandledrejection', (event) => {
  const reason = event.reason;
  const errorMessage = reason instanceof Error ? reason.message : String(reason);
  console.error('Unhandled Promise Rejection:', reason);

  // Surface the error to the user via toast
  try {
    const { useUiStore } = require('./stores/uiStore');
    useUiStore.getState().addToast(
      `Async error: ${errorMessage}`,
      'error',
      8000,
    );
  } catch {
    // Store not available yet (app still booting) -- console.error above is enough
  }
});

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
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);
