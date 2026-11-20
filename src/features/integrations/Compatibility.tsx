import {useEffect,useRef,useState} from "react";
import {compatibilityClient,type CompatibilityResult,type CompatibilitySnapshot,type EvidenceStatus} from "../../lib/compatibility";
import type {PreferenceLanguage} from "../../lib/preferences";
import "./compatibility.css";

const copy={
 tr:{title:"Test edilen sürümler ve bağlantılar",intro:"Bağlantı sorunlarında hangi sürüm ve giriş türlerinin test edildiğine buradan bakabilirsin. Uygulama kartı kendi güncel bağlantı durumunu gösterir.",refresh:"Yenile",loading:"Uyum kayıtları okunuyor…",failed:"Uyum kayıtları okunamadı. Yeniden deneyebilirsin.",unavailable:"Uyum kayıtları masaüstü uygulamasında okunabilir.",stale:"Tarama veya motor bilgisi değişti. Güncel kapsam için yenile.",changed:"Kurulum veya ayar bağlamı değişti. Bağlantıyı yeniden doğrula.",installed:"Bulunan sürümler",none:"Tamamlanmış taramada kurulum yok; bu, desteklenmediği anlamına gelmez.",notScanned:"Güncel kurulum taraması yok. Kayıtlar yalnız geçmiş testleri gösterir.",unknownVersion:"Sürüm bilinmiyor",supported:"Ayar adaptörü uygun",unsupported:"Ayar adaptörü bu sürümü desteklemiyor",needsScan:"Kurulum değişmiş veya yeniden okunamamış; yeniden tara ve doğrula.",unobserved:"Bu sürüm için bire bir kayıt yok; canlı uyum doğrulanmadı.",configIssue:"Sahip olunan ayar güncel doğrulanamadı; bağlantıyı yeniden denetle.",records:"Geçmiş testler",scope:"Test ayrıntıları",engine:"Motor",active:"Çalışan sürüm",inactive:"Çalışmıyor / bilinmiyor",development:"Geliştirme sürümü",pin:"Her PennyPinch paketi belirli bir motor sürümüyle gelir.",adapters:"Adaptör kayıtları",date:"Kanıt tarihi",statuses:{not_tested:"Denenmedi",unknown:"Bilinmiyor",environment_blocked:"Ortam engeli",local_fixture_partial:"Deneysel · kısmi yerel test",local_fixture_pass:"Deneysel · yerel test",live_verified:"Doğrulanmış kapsam",unsupported:"Desteklenmiyor"},surfaces:{runtime:"Motor",cli:"CLI",desktop:"Masaüstü",cli_desktop:"CLI ve masaüstü",other:"Diğer yüzeyler"},auth:{none:"Hesap gerekmez",synthetic:"Yerel test anahtarı",chatgpt:"ChatGPT hesabı",api_key:"Gerçek API anahtarı",oauth:"OAuth hesabı",unknown:"Hesap yöntemi bilinmiyor"},platforms:{windows_10_x64:"Windows 10 x64",windows_11_x64:"Windows 11 x64",macos:"macOS",linux:"Linux",unknown:"OS doğrulanmadı"}},
 en:{title:"Tested versions and connections",intro:"Use these records to look up tested versions and sign-in methods when troubleshooting. The application card shows your current connection status.",refresh:"Refresh",loading:"Reading compatibility records…",failed:"Compatibility records could not be read. You can try again.",unavailable:"Compatibility records are available in the desktop app.",stale:"Discovery or engine context changed. Refresh to read the current scope.",changed:"Installation or settings context changed. Verify the connection again.",installed:"Detected versions",none:"No installations in the completed scan; this does not mean unsupported.",notScanned:"No current installation scan. Records only describe historical tests.",unknownVersion:"Version unknown",supported:"Config adapter available",unsupported:"Config adapter does not support this version",needsScan:"Installation changed or could not be re-read; scan and verify again.",unobserved:"No exact record for this version; live compatibility is not verified.",configIssue:"Owned settings could not be verified as current; inspect the connection again.",records:"Recorded tests",scope:"Test details",engine:"Engine",active:"Running version",inactive:"Not running / unknown",development:"Development build",pin:"Each PennyPinch package includes a specific engine version.",adapters:"Adapter records",date:"Evidence date",statuses:{not_tested:"Not tested",unknown:"Unknown",environment_blocked:"Environment blocked",local_fixture_partial:"Experimental · partial local test",local_fixture_pass:"Experimental · local test",live_verified:"Verified scope",unsupported:"Unsupported"},surfaces:{runtime:"Engine",cli:"CLI",desktop:"Desktop",cli_desktop:"CLI and desktop",other:"Other surfaces"},auth:{none:"No account",synthetic:"Local test credential",chatgpt:"ChatGPT account",api_key:"Real API key",oauth:"OAuth account",unknown:"Account method unknown"},platforms:{windows_10_x64:"Windows 10 x64",windows_11_x64:"Windows 11 x64",macos:"macOS",linux:"Linux",unknown:"OS not verified"}},
};

export function Compatibility({language,contextKey,disabled}:{language:PreferenceLanguage;contextKey:string;disabled:boolean}){
 const text=copy[language];
 const [snapshot,setSnapshot]=useState<CompatibilitySnapshot|null>(null);
 const [loadedKey,setLoadedKey]=useState<string|null>(null);
 const [issue,setIssue]=useState<CompatibilityResult|null>(null);
 const [pending,setPending]=useState(false);
 const [changed,setChanged]=useState(false);
 const mounted=useRef(false),locked=useRef(false),sequence=useRef(0),currentKey=useRef(contextKey),priorFingerprint=useRef<string|null>(null);
 currentKey.current=contextKey;
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;sequence.current++;};},[]);
 const stale=loadedKey!==null&&loadedKey!==contextKey;
 async function refresh(){
  if(locked.current||disabled)return;
  locked.current=true;setPending(true);setIssue(null);const key=currentKey.current,seq=++sequence.current;setLoadedKey(key);
  try{
   const result=await compatibilityClient.getSnapshot();
   if(!mounted.current||sequence.current!==seq||currentKey.current!==key)return;
   if(result.kind==="snapshot"){
    const fingerprint=result.snapshot.context_fingerprint;
    setChanged(fingerprint!==null&&priorFingerprint.current!==null&&fingerprint!==priorFingerprint.current);
    priorFingerprint.current=fingerprint;setSnapshot(result.snapshot);setLoadedKey(key);
   }else{setIssue(result);}
  }finally{locked.current=false;if(mounted.current)setPending(false);}
 }
 const current=snapshot!==null&&!stale&&!disabled&&!pending&&issue===null;
 return <details className="compatibility" onToggle={event=>{if(event.currentTarget.open&&!snapshot&&!issue&&!pending)void refresh();}}>
  <summary>{text.title}</summary>
  <div className="compatibility__content">
   <div className="compatibility__heading"><p>{text.intro}</p><button className="button" type="button" disabled={disabled||pending} onClick={()=>void refresh()}>{pending?text.loading:text.refresh}</button></div>
   {pending&&<p role="status">{text.loading}</p>}
   {!pending&&disabled&&<p role="status" className="compatibility__notice">{language==="tr"?"İşlem sürüyor; güncel kapsamı bitince yenile.":"Another operation is in progress. Refresh when it finishes."}</p>}
   {!pending&&!disabled&&stale&&<p role="status" className="compatibility__notice">{text.stale}</p>}
   {!pending&&issue&&<p role="status" className="compatibility__notice">{issue.kind==="desktop_unavailable"?text.unavailable:text.failed}</p>}
   {current&&<>
    {changed&&<p role="status" className="compatibility__notice">{text.changed}</p>}
    <div className="compatibility__release"><strong>PennyPinch {snapshot.app_version}</strong><span>{text.development}</span><span>{text.engine} {snapshot.engine.pinned_version}</span><span>{text.active}: {snapshot.engine.engine_status==="ready"?snapshot.engine.active_version??text.inactive:text.inactive}</span></div>
    <p className="compatibility__pin">{text.pin}</p>
    <h3>{text.installed}</h3>
    {snapshot.scan_generation===null?<p>{text.notScanned}</p>:!snapshot.installations.length?<p>{text.none}</p>:<ul className="compatibility__installations">{snapshot.installations.map(i=><li key={i.installation_id}>
     <strong>{i.client_kind==="codex"?"Codex":i.client_kind==="claude_code"?"Claude Code":"OpenCode"} · {text.surfaces[i.surface]} · {i.version??text.unknownVersion}</strong>
     <span>{i.freshness!=="current"?text.needsScan:i.version===null?text.unknownVersion:i.config_supported?text.supported:text.unsupported}</span>
     {i.freshness==="current"&&!i.historical_evidence_ids.length&&<small>{text.unobserved}</small>}
     {!["not_owned","matches_owned_endpoint"].includes(i.config_state)&&<small>{text.configIssue}</small>}
    </li>)}</ul>}
    <h3>{text.records}</h3>
    <ul className="compatibility__records">{snapshot.records.map(r=><li key={r.id}>
     <div className="compatibility__record-heading"><strong>{r.product}</strong><span className={`compatibility__badge is-${r.evidence_status}`}>{text.statuses[r.evidence_status as EvidenceStatus]}</span></div>
     <span className="compatibility__version">{r.version}</span>
     <p>{text.surfaces[r.surface]} · {text.auth[r.auth_method]} · {text.platforms[r.platform]}</p>
     <details><summary>{text.scope}</summary><p>{r.scope}</p><ul>{r.capability_counts.map(c=><li key={c.status}>{c.count} · {text.statuses[c.status]}</li>)}</ul><p>{text.engine} {r.engine_version} · {text.date}: {r.evidence_date}</p><ul>{r.evidence.map(path=><li key={path}><code>{path}</code></li>)}</ul></details>
    </li>)}</ul>
    <details className="compatibility__adapters"><summary>{text.adapters}</summary><ul>{snapshot.adapters.map(a=><li key={a.client_kind}>{a.client_kind} <code>{a.fingerprint}</code></li>)}</ul></details>
   </>}
  </div>
 </details>;
}
