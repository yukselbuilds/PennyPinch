import React from 'react';
import {createRoot,Root} from 'react-dom/client';
import App from '../../src/App';
import {DisconnectDialog} from '../../src/features/integrations/DisconnectDialog';
import {reset,Scenario} from './desktopTransport';
import '../../src/styles.css';
export {bumpCompatibilityContext,blockCompatibility,releaseCompatibility,blockDiscoveryScan,releaseDiscoveryScan,resolveRecommendations} from './desktopTransport';
let root:Root|null=null;
export function mount(language='tr',theme='dark',scenario:Scenario='data'){
 root?.unmount();reset(language,theme,scenario);document.body.innerHTML='<div id="root"></div>';
 root=createRoot(document.getElementById('root')!);root.render(<App/>);
}
export function disconnect(language='tr'){
 root?.unmount();document.body.innerHTML='<button id="return-focus">Fixture trigger</button><div id="root"></div>';
 const button=document.getElementById('return-focus')!;button.focus();root=createRoot(document.getElementById('root')!);
 root.render(<DisconnectDialog open pending={false} language={language as 'tr'|'en'} clientName="Codex" scope="user_config" restoreFocusRef={{current:button}} onCancel={()=>{root?.unmount();button.focus();}} onConfirm={()=>{throw Error('fixture_mutation_not_allowed');}}/>);
}
