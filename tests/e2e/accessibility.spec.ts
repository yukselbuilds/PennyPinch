import {test,expect,Page} from '@playwright/test';
import {build} from 'vite';import react from '@vitejs/plugin-react';
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
const root=process.cwd(),base=path.join(root,'work/proofs/accessibility');
const run=path.join(base,'ui-'+crypto.randomUUID());fs.mkdirSync(run,{recursive:true});
const bundle=path.join(run,'bundle');
const sourceFiles=[...['src','tests'].flatMap(d=>fs.readdirSync(d,{recursive:true}).map(p=>path.join(d,String(p))).filter(p=>fs.statSync(p).isFile())),
 'package.json','pnpm-lock.yaml','compatibility/clients.json','runtime/manifest.json'];
const fingerprint=()=>Object.fromEntries(sourceFiles.map(p=>[p.replaceAll('\\','/'),crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex')]));
const before=fingerprint(),checks:any[]=[],violations:any[]=[],timings:number[]=[],outcomes:any[]=[];
test.afterEach(async({},info)=>{outcomes.push({name:info.title,status:info.status,expected:info.expectedStatus});});
test.beforeAll(async()=>{
 const mock=path.join(root,'tests/fixtures/desktopTransport.ts');
 await build({configFile:false,logLevel:'silent',plugins:[react(),{name:'no-native',generateBundle(){
  if([...this.getModuleIds()].some(p=>/[\\/]node_modules[\\/]@tauri-apps[\\/]api[\\/]/.test(p)))throw Error('native_api_in_test');
 }}],resolve:{alias:{'@tauri-apps/api/core':mock,'@tauri-apps/api/event':mock,'@tauri-apps/api/webview':mock}},
 define:{'process.env.NODE_ENV':'"production"'},build:{minify:true,emptyOutDir:false,outDir:bundle,
  lib:{entry:path.join(root,'tests/fixtures/desktop.tsx'),name:'DesktopFixture',formats:['iife'],fileName:()=> 'fixture.js',cssFileName:'fixture'}}});
});
async function setup(page:Page,language='tr',theme='dark',scenario='data'){
 await page.route('http://127.0.0.1:54328/',route=>route.fulfill({contentType:'text/html',body:'<!doctype html><html><head><title>Headroom isolated accessibility fixture</title></head><body><div id="root"></div></body></html>'}));
 await page.goto('http://127.0.0.1:54328/');await page.addStyleTag({path:path.join(bundle,'fixture.css')});await page.addScriptTag({path:path.join(bundle,'fixture.js')});
 await page.addScriptTag({path:path.join(root,'node_modules/axe-core/axe.min.js')});
 await page.evaluate(v=>(window as any).DesktopFixture.mount(v.language,v.theme,v.scenario),{language,theme,scenario});
 await expect(page.locator('html')).toHaveAttribute('lang',language);await expect(page.locator('html')).toHaveAttribute('data-theme',theme);
 await expect(page.locator('.overview__metric')).toBeVisible();
}
async function audit(page:Page,label:string){
 const result=await page.evaluate(async()=>(window as any).axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21a','wcag21aa']}}));
 const failures=result.violations.map((v:any)=>({id:v.id,impact:v.impact,nodes:v.nodes.map((n:any)=>({target:n.target,checks:n.any.map((a:any)=>({id:a.id,data:a.data}))}))}));
 violations.push(...failures.map((v:any)=>({label,...v})));checks.push({label,violations:failures.length,passes:result.passes.length});
}
test('four screens: names, contrast, empty/error/data and responsive actions',async({page})=>{
 for(const language of ['tr','en'])for(const theme of ['dark','light'])for(const scenario of ['data','empty','error']){
  await setup(page,language,theme,scenario);
  for(let screen=0;screen<4;screen++){
   await page.evaluate(()=>{(window as any).__navPaint=null;document.querySelector('.primary-nav')!.addEventListener('click',()=>{const start=performance.now();requestAnimationFrame(()=>requestAnimationFrame(()=>{(window as any).__navPaint=performance.now()-start;}));},{once:true,capture:true});});
   await page.locator('.primary-nav button').nth(screen).click();
   await expect(page.locator('.primary-nav button').nth(screen)).toHaveAttribute('aria-current','page');
   await expect.poll(()=>page.evaluate(()=>(window as any).__navPaint)).not.toBeNull();timings.push(await page.evaluate(()=>(window as any).__navPaint));
   // Give fixture effects one frame; every IPC stays inside the explicit transport.
   await page.waitForTimeout(40);await audit(page,`${language}-${theme}-${scenario}-${screen}`);
   for(const scale of [1,1.25,1.5]){
    await page.setViewportSize({width:Math.floor(1024/scale),height:Math.floor(720/scale)});
    const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1);expect(overflow).toBe(false);
    for(const element of await page.locator('main button:visible,main select:visible,main summary:visible').all()){
     await element.scrollIntoViewIfNeeded();const box=await element.boundingBox();expect(box).not.toBeNull();expect(box!.x).toBeGreaterThanOrEqual(-1);expect(box!.x+box!.width).toBeLessThanOrEqual(page.viewportSize()!.width+1);
    }
   }
   await page.setViewportSize({width:1024,height:720});
  }
 }
 expect(violations.filter(v=>['critical','serious'].includes(v.impact)).map(v=>({label:v.label,id:v.id,targets:v.nodes.map((n:any)=>n.target)}))).toEqual([]);
 expect([...timings].sort((a,b)=>a-b)[Math.ceil(timings.length*.95)-1]).toBeLessThanOrEqual(250);
});
test('keyboard navigation and modal focus stay usable',async({page})=>{
 for(const language of ['tr','en'])for(const theme of ['dark','light']){
  await setup(page,language,theme);await page.setViewportSize({width:1024,height:720});
  await page.keyboard.press('Tab');await expect(page.locator('.skip-navigation')).toBeFocused();await page.keyboard.press('Enter');await expect(page.locator('main')).toBeFocused();
  await page.locator('.export-button__trigger').focus();await page.keyboard.press('Enter');await expect(page.locator('.export-button__panel')).toBeVisible();await audit(page,`${language}-${theme}-export`);
  await page.keyboard.press('Escape');await expect(page.locator('.export-button__trigger')).toBeFocused();
  await page.locator('.primary-nav button').nth(2).click();
  await page.locator('.activity__filters summary').click();await page.locator('[name="model-kind"]').selectOption('exact');await page.locator('.activity__filter-actions button[type="submit"]').click();
  await expect(page.locator('[name="model-value"]')).toBeFocused();await expect(page.locator('#activity-model-error')).toHaveAttribute('role','alert');
  await page.locator('.activity__filters summary').click();await page.locator('.activity__rows button').first().click();
  const detail=page.locator('dialog[open]');await expect(detail).toBeVisible();await audit(page,`${language}-${theme}-detail`);await page.keyboard.press('Escape');await expect(page.locator('.activity__rows button').first()).toBeFocused();
  await page.locator('.primary-nav button').nth(1).click();await page.locator('.applications-grid button').filter({hasText:/Bağlantıyı yönet|Manage connection/}).first().click();
  const connection=page.locator('.connection-dialog[open]');await expect(connection).toBeVisible();
  await expect(connection.locator('.connection-dialog__actions .button-primary')).toBeEnabled();
  await audit(page,`${language}-${theme}-connect`);
  for(let i=0;i<15;i++){await page.keyboard.press('Tab');expect(await connection.evaluate(d=>d.contains(document.activeElement))).toBe(true);}
  await page.keyboard.press('Escape');await expect(connection).toHaveCount(0);
  await page.locator('.desktop-lifecycle__exit').click();const exit=page.locator('dialog[open]');await expect(exit).toBeVisible();await audit(page,`${language}-${theme}-exit`);await page.keyboard.press('Escape');await expect(page.locator('.desktop-lifecycle__exit')).toBeFocused();
  await page.evaluate(v=>(window as any).DesktopFixture.disconnect(v),language);const remove=page.locator('dialog[open]');await expect(remove).toBeVisible();await audit(page,`${language}-${theme}-remove`);
  for(let i=0;i<6;i++){await page.keyboard.press(i%2?'Shift+Tab':'Tab');expect(await remove.evaluate(d=>d.contains(document.activeElement))).toBe(true);}
  await page.keyboard.press('Escape');await expect(page.locator('#return-focus')).toBeFocused();
 }
 expect(violations.filter(v=>['critical','serious'].includes(v.impact)).map(v=>({label:v.label,id:v.id,targets:v.nodes.map((n:any)=>n.target)}))).toEqual([]);
});
test('compatibility: scoped records, responsive details, stale replies and context changes',async({page})=>{
 for(const language of ['tr','en'])for(const theme of ['dark','light']){
  for(const scenario of ['data','empty','error']){
   await setup(page,language,theme,scenario);await page.setViewportSize({width:1024,height:720});
   await page.locator('.primary-nav button').nth(1).click();await expect(page.locator('.applications-footer')).not.toContainText(/Checking local|kontrol ediliyor/);
   await page.locator('.compatibility > summary').click();
   const panel=page.locator('.compatibility');await expect(panel.locator('button')).toBeEnabled();
   if(scenario==='error')await expect(panel.locator('.compatibility__notice')).toContainText(/could not|okunamadı/);
   else{
    await expect(panel.locator('.compatibility__records > li')).toHaveCount(12);
    await expect(panel).toContainText(/does not verify|doğrulandığı anlamına gelmez/);
    if(scenario==='empty')await expect(panel).toContainText(/No installations|kurulum yok/);
    if(scenario==='data'){
     await panel.locator('.compatibility__records details > summary').first().click();await audit(page,`${language}-${theme}-catalog-detail`);
     if(language==='tr'&&theme==='dark'||language==='en'&&theme==='light'){
      await panel.locator('summary').first().evaluate(e=>e.scrollIntoView({block:'start'}));await page.screenshot({path:path.join(run,`catalog-${language}-${theme}.png`)});
     }
    }
   }
   await audit(page,`${language}-${theme}-catalog-${scenario}`);
   for(const scale of [1,1.25,1.5]){
    await page.setViewportSize({width:Math.floor(1024/scale),height:Math.floor(720/scale)});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1)).toBe(false);
    for(const element of await panel.locator('button:visible,summary:visible').all()){
     await element.scrollIntoViewIfNeeded();const box=await element.boundingBox();expect(box).not.toBeNull();expect(box!.x).toBeGreaterThanOrEqual(-1);expect(box!.x+box!.width).toBeLessThanOrEqual(page.viewportSize()!.width+1);
    }
   }
  }
 }
 await setup(page,'en','dark');await page.setViewportSize({width:1024,height:720});await page.locator('.primary-nav button').nth(1).click();
 await page.evaluate(()=>(window as any).DesktopFixture.blockCompatibility());await page.locator('.compatibility > summary').click();
 const panel=page.locator('.compatibility');await expect(panel.locator('button')).toBeDisabled();
 await page.getByRole('button',{name:'Scan again',exact:true}).click();await expect(panel.locator('button')).toBeDisabled();
 await page.evaluate(()=>(window as any).DesktopFixture.releaseCompatibility());await expect(panel.locator('button')).toBeEnabled();
 await expect(panel.locator('.compatibility__notice')).toContainText('context changed');await expect(panel.locator('.compatibility__records')).toHaveCount(0);
 await panel.locator('button').click();await expect(panel.locator('.compatibility__records > li')).toHaveCount(12);
 await page.evaluate(()=>(window as any).DesktopFixture.blockDiscoveryScan());await page.getByRole('button',{name:'Scan again',exact:true}).click();
 await expect(panel.locator('.compatibility__records')).toHaveCount(0);await expect(panel.locator('.compatibility__notice')).toContainText('operation is in progress');
 await page.evaluate(()=>(window as any).DesktopFixture.releaseDiscoveryScan());await expect(page.getByRole('button',{name:'Scan again',exact:true})).toBeEnabled();
 await panel.locator('button').click();await expect(panel.locator('.compatibility__records > li')).toHaveCount(12);
 await page.evaluate(()=>(window as any).DesktopFixture.bumpCompatibilityContext());await panel.locator('button').click();await expect(panel.locator('.compatibility__notice')).toContainText('settings context changed');
 expect(violations.filter(v=>['critical','serious'].includes(v.impact)).map(v=>({label:v.label,id:v.id}))).toEqual([]);
});

test.afterAll(async()=>{
 const after=fingerprint();const sorted=[...timings].sort((a,b)=>a-b),p95=sorted[Math.max(0,Math.ceil(sorted.length*.95)-1)]??null;
 fs.writeFileSync(path.join(run,'result.json'),JSON.stringify({passed:outcomes.length===3&&checks.length===84&&outcomes.every(v=>v.status===v.expected)&&violations.every(v=>!['critical','serious'].includes(v.impact))&&JSON.stringify(before)===JSON.stringify(after),outcomes,checks,violations,timing:{samples:timings.length,p95_ms:p95,threshold_ms:250,scope:'browser click event to second animation frame; excludes test-driver waiting'},before,after,environment:{node:process.version,platform:process.platform,scope:'real App, isolated transport; viewport equivalents are not real OS DPI'}},null,2)+'\n');
 console.log(JSON.stringify({run:path.relative(root,run),audits:checks.length,violations:violations.length,navigation_p95_ms:p95}));
});
