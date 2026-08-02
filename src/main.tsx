import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import PasswordGate from './components/PasswordGate';
import './index.css';

const LEGACY_AUTH = import.meta.env.VITE_LEGACY_AUTH === 'true';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {LEGACY_AUTH ? (
      <PasswordGate>
        <App />
      </PasswordGate>
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
