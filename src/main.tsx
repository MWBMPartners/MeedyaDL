/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * React application entry point.
 * Mounts the root App component into the DOM element defined in index.html.
 * Imports the global CSS styles including Tailwind directives and
 * platform-adaptive theme variables.
 */

import React from 'react';
import ReactDOM from 'react-dom/client';

// Import the root application component
import App from './App';

// Import global styles (Tailwind CSS directives + custom properties)
import './styles/globals.css';

// Find the root DOM element created in index.html
const rootElement = document.getElementById('root');

// Ensure the root element exists before attempting to mount React
if (!rootElement) {
  throw new Error(
    'Root element not found. Ensure index.html contains a <div id="root"></div> element.'
  );
}

// Create the React root and render the application
// StrictMode enables additional development-time checks and warnings
ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
