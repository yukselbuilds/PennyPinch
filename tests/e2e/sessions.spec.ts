import {test,expect,Page} from '@playwright/test';
import {build} from 'vite';import react from '@vitejs/plugin-react';
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
const root=process.cwd(),run=path.join(root,'work/proofs/sessions/ui',crypto.randomUUID());fs.mkdirSync(run,{recursive:true});
const bundle=path.join(run,'bundle');
const sourceFiles=[...['src','tests'].flatMap(d=>fs.readdirSync(path.join(root,d),{recursive:true}).map(p=>path.join(d,String(p)))
 .filter(p=>fs.statSync(path.join(root,p)).isFile())),'package.json','pnpm-lock.yaml','compatibility/clients.json','runtime/manifest.json'];
const fingerprint=()=>Object.fromEntries(sourceFiles.map(p=>[p.replaceAll('\\','/'),crypto.createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex')]));
const before=fingerprint(),outcomes:any[]=[],audits:any[]=[],browserVersions=new Set<string>();
test.beforeAll(async()=>{
 await build({configFile:false,logLevel:'silent',plugins:[react(),{name:'fixture-transport-only',generateBundle(){
  if([...this.getModuleIds()].some(p=>/[\\/]node_modules[\\/]@tauri-apps[\\/]api[\\/]/.test(p)))throw Error('native_api_in_fixture');
 }}],define:{'process.env.NODE_ENV':'"production"'},
 resolve:{alias:{'@tauri-apps/api/core':path.join(root,'tests/fixtures/desktopTransport.ts'),'@tauri-apps/api/event':path.join(root,'tests/fixtures/desktopTransport.ts'),'@tauri-apps/api/webview':path.join(root,'tests/fixtures/desktopTransport.ts')}},
 build:{outDir:bundle,emptyOutDir:false,lib:{entry:path.join(root,'tests/fixtures/desktop.tsx'),name:'SessionsFixture',formats:['iife'],fileName:()=> 'fixture.js',cssFileName:'fixture'}}});
});
test.afterEach(async({},info)=>{outcomes.push({name:info.title,status:info.status,expected:info.expectedStatus});});
async function setup(page:Page,language='tr',theme='dark',scenario='data'){
 const browserVersion=page.context().browser()?.version();if(browserVersion)browserVersions.add(browserVersion);
 await page.route('http://127.0.0.1:54330/',route=>route.fulfill({contentType:'text/html',body:'<!doctype html><html><head><title>Headroom isolated sessions fixture</title></head><body><div id="root"></div></body></html>'}));
 await page.goto('http://127.0.0.1:54330/');await page.addStyleTag({path:path.join(bundle,'fixture.css')});
 await page.addScriptTag({path:path.join(bundle,'fixture.js')});await page.addScriptTag({path:path.join(root,'node_modules/axe-core/axe.min.js')});
 await page.evaluate(v=>(window as any).SessionsFixture.mount(v.language,v.theme,v.scenario),{language,theme,scenario});
 await expect(page.locator('html')).toHaveAttribute('lang',language);await expect(page.locator('html')).toHaveAttribute('data-theme',theme);
}
const nav=(page:Page,screen:string)=>page.locator(`.primary-nav [data-screen="${screen}"]`);
async function openSessions(page:Page){await nav(page,'sessions').click();await expect(page.locator('.sessions')).toBeVisible();}
async function fixture(page:Page,method:string,...args:unknown[]){return page.evaluate(({method,args})=>(window as any).__HeadroomSessionFixture[method](...args),{method,args});}
async function audit(page:Page,label:string){
 const result=await page.evaluate(async()=>(window as any).axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21a','wcag21aa']}}));
 const violations=result.violations.map((v:any)=>({id:v.id,impact:v.impact,targets:v.nodes.map((n:any)=>n.target)}));audits.push({label,violations});
 expect(violations.filter((v:any)=>['critical','serious'].includes(v.impact))).toEqual([]);
}

test('sessions and accounts stay distinct; unknown scope is not zero',async({page})=>{
 for(const language of ['tr','en'])for(const theme of ['dark','light']){
  await setup(page,language,theme);await page.setViewportSize({width:683,height:720});await openSessions(page);
  const screen=page.locator('.sessions'),visibleRecords=screen.locator('.sessions__record:visible');await expect(visibleRecords).toHaveCount(2);
  await expect(visibleRecords.filter({hasText:'fixture-model'})).toHaveCount(2);
  await expect(visibleRecords.first()).toContainText(language==='tr'?'Ana oturum':'Main session');
  await expect(visibleRecords.nth(1)).toContainText(language==='tr'?'Alt ajan':'Subagent');
  await expect(screen.locator('.sessions__scope-row.is-unknown')).toHaveCount(2);
  await expect(screen.locator('.sessions__scope-row.is-unknown .sessions__scope-value')).toHaveText(Array(2).fill(language==='tr'?'Bilinmiyor':'Unknown'));
  const history=await fixture(page,'history');expect(history.filter((c:any)=>c.operation==='read')).toHaveLength(1);
  expect(await screen.locator('.sessions__previous').getAttribute('open')).toBeNull();
  const storedRecord=screen.locator('.sessions__previous .sessions__record');await expect(storedRecord).toHaveCount(1);await expect(storedRecord).toBeHidden();
  await screen.locator('.sessions__previous summary').click();await expect(storedRecord).toBeVisible();await expect(visibleRecords).toHaveCount(3);
  await expect(screen).toContainText(language==='tr'?'Son trafik ve sohbet tasarrufu bilinmiyor':'Last traffic and chat savings are unknown');
  await expect(screen).not.toContainText(/(?:\d+(?:[.,]\d+)?\s*%|[$€£]\s*\d)/);
  await expect(screen.locator('.overview__percent')).toHaveCount(0);await audit(page,`${language}-${theme}-sessions-stored`);
  for(const width of [683,512]){
   await page.setViewportSize({width,height:720});expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1)).toBe(false);
   for(const item of await screen.locator('button:visible,summary:visible').all()){
    await item.scrollIntoViewIfNeeded();const box=await item.boundingBox();expect(box).not.toBeNull();expect(box!.x).toBeGreaterThanOrEqual(-1);expect(box!.x+box!.width).toBeLessThanOrEqual(page.viewportSize()!.width+1);
   }
  }
  const shell=page.locator('.sessions-screen');
  await shell.getByRole('button',{name:language==='tr'?'Hesaplar':'Accounts',exact:true}).click();
  await expect(page.locator('.accounts')).toBeVisible();await expect(page.locator('.sessions')).toHaveCount(0);await audit(page,`${language}-${theme}-accounts-tab`);
  await shell.getByRole('button',{name:language==='tr'?'Sohbetler':'Chats',exact:true}).click();await expect(page.locator('.sessions__record:visible')).toHaveCount(2);
 }
});

test('empty snapshot separates observed zero from unknown clients',async({page})=>{
 await setup(page,'en','dark','empty');await openSessions(page);const screen=page.locator('.sessions');
 await expect(screen.locator('.sessions__record')).toHaveCount(0);await expect(screen.locator('.sessions__scope-row.is-unknown')).toHaveCount(2);
 await expect(screen.locator('.sessions__scope-row').first()).toContainText('0 loaded');
 const unknownScopeText=(await screen.locator('.sessions__scope-row.is-unknown').allTextContents()).join(' ');
 expect(unknownScopeText).not.toMatch(/0 loaded/);
 await expect(screen.locator('.sessions__empty')).toContainText('does not mean no sessions exist');await audit(page,'empty-unknown');
});

test('opening Codex needs confirmation and sends one fixed open operation',async({page})=>{
 await setup(page,'en','dark');await openSessions(page);const screen=page.locator('.sessions');
 const open=screen.getByRole('button',{name:'Open Codex chat center',exact:true});await expect(open).toBeEnabled();
 await open.click();const dialog=page.locator('dialog.accounts-dialog[open]');await expect(dialog).toBeVisible();
 const cancel=dialog.getByRole('button',{name:'Cancel',exact:true}),confirm=dialog.getByRole('button',{name:'Open Codex',exact:true});
 await expect(cancel).toBeFocused();await page.keyboard.press('Tab');await expect(confirm).toBeFocused();await page.keyboard.press('Shift+Tab');await expect(cancel).toBeFocused();await audit(page,'open-confirmation');
 await page.keyboard.press('Escape');await expect(dialog).toHaveCount(0);await expect(open).toBeEnabled();await expect(open).toBeFocused();
 expect((await fixture(page,'history')).filter((call:any)=>call.operation==='open_codex_center')).toHaveLength(0);
 await open.click();await expect(cancel).toBeFocused();await page.keyboard.press('Tab');await expect(confirm).toBeFocused();await page.keyboard.press('Enter');
 const calls=(await fixture(page,'history')).filter((call:any)=>call.operation==='open_codex_center');expect(calls).toHaveLength(1);
 await expect(page.locator('.sessions-screen > .sessions__notice')).toBeVisible();
 expect(Object.keys(calls[0]).sort()).toEqual(['confirm_open','operation','request_id','schema_version','snapshot_id']);
 expect(calls[0]).toMatchObject({schema_version:1,operation:'open_codex_center',snapshot_id:'ae0df834-8df0-48c2-a174-c0f652bc2ad1',confirm_open:true});
 expect(calls[0].request_id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
 for(const canary of ['title-canary','preview-canary','sk-private-canary','raw-ipc-canary','raw-transport-canary','response-canary'])
  await expect(page.locator('body')).not.toContainText(canary);
});

test('launch failure remains visible across its automatic read until manual refresh',async({page})=>{
 await setup(page,'en','dark');await openSessions(page);const screen=page.locator('.sessions');
 await fixture(page,'failLaunch');await screen.getByRole('button',{name:'Open Codex chat center',exact:true}).click();
 const dialog=page.locator('dialog.accounts-dialog[open]');await dialog.getByRole('button',{name:'Open Codex',exact:true}).click();
 const failure=screen.getByRole('alert');await expect(failure).toContainText('could not be opened');
 await expect(screen.locator('.sessions__record:visible')).toHaveCount(2);
 await expect(failure).toContainText('could not be opened');
 await expect(page.locator('.sessions-screen > .sessions__notice')).toHaveCount(0);
 await screen.getByRole('button',{name:'Refresh status',exact:true}).click();
 await expect(screen.locator('.sessions__record:visible')).toHaveCount(2);await expect(screen.locator('.sessions__notice--error')).toHaveCount(0);
});

test('a failed refresh clears the previous live session claims',async({page})=>{
 await setup(page,'tr','light');await openSessions(page);const screen=page.locator('.sessions');
 await expect(screen.locator('.sessions__record:visible')).toHaveCount(2);await fixture(page,'setMode','error');
 await screen.getByRole('button',{name:'Durumu yenile',exact:true}).click();
 await expect(screen.getByRole('alert')).toContainText('Oturum bilgisi alınamadı');
 await expect(screen.locator('.sessions__notice--warning')).toBeVisible();await expect(screen.locator('.sessions__record')).toHaveCount(0);
 await expect(screen.locator('.sessions__scope-name')).toHaveText(['Codex · geçerli profil','Claude Code','OpenCode']);
 await expect(screen.getByRole('button',{name:'Codex sohbet merkezini aç',exact:true})).toBeDisabled();
});

test('hidden session polling does not overlap and visibility resumes a fresh read',async({page})=>{
 await setup(page,'en','dark');await fixture(page,'blockReads');await openSessions(page);
 await expect(page.locator('.sessions__status')).toBeVisible();
 await page.evaluate(()=>{Object.defineProperty(document,'visibilityState',{configurable:true,value:'hidden'});document.dispatchEvent(new Event('visibilitychange'));});
 await page.waitForTimeout(10_200);expect(await fixture(page,'readStats')).toEqual({reads:1,max_active:1});
 await fixture(page,'releaseReads');await page.waitForTimeout(40);await expect(page.locator('.sessions__record:visible')).toHaveCount(0);
 await page.evaluate(()=>{Object.defineProperty(document,'visibilityState',{configurable:true,value:'visible'});document.dispatchEvent(new Event('visibilitychange'));});
 await expect(page.locator('.sessions__record:visible')).toHaveCount(2);expect(await fixture(page,'readStats')).toEqual({reads:2,max_active:1});
});

test.afterAll(()=>{
 const after=fingerprint();fs.writeFileSync(path.join(run,'result.json'),JSON.stringify({before,after,inputs_unchanged:JSON.stringify(before)===JSON.stringify(after),outcomes,audits,environment:{node:process.version,pnpm:process.env.HEADROOM_PNPM_VERSION??null,browser_channel:process.platform==='win32'?'msedge':'chromium',browser_versions:[...browserVersions],platform:process.platform,scope:'real App, isolated desktop transport; no native IPC or provider calls'}},null,2)+'\n');
});
