import React from 'react';
import { createRoot, Root } from 'react-dom/client';
import { Accounts } from '../../src/features/integrations/Accounts';
import * as transport from './accountsTransport';
import '../../src/styles.css';
export { history, setFlowState, blockRead, releaseRead } from './accountsTransport';
let root: Root | null = null, language: 'tr' | 'en' = 'tr', generation = 1;
function render(visible = true) {
  root!.render(<main><button id="outside">Fixture focus target</button>{visible && <Accounts
    language={language} installations={transport.installations} scanGeneration={generation} />}</main>);
}
export function mount(lang: 'tr' | 'en', theme: string, scenario: string) {
  root?.unmount(); transport.reset(scenario); language = lang; generation = 1;
  document.documentElement.lang = lang; document.documentElement.dataset.theme = theme;
  document.body.innerHTML = '<div id="root"></div>'; root = createRoot(document.getElementById('root')!); render();
}
export function show(visible: boolean) { render(visible); }
export function rescan() { generation++; render(); }
