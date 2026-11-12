import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { ComposeWindow, composeRoute } from './features/compose-window/ComposeWindow';
import './ui/global.css';

// Une seule page pour toutes les fenêtres : `#compose/<id>` est une fenêtre « Nouveau mail ».
const draftId = composeRoute(window.location.hash);

createRoot(document.getElementById('root')!).render(
  <StrictMode>{draftId != null ? <ComposeWindow draftId={draftId} /> : <App />}</StrictMode>,
);
