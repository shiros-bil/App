// stress test + profiler: N divisions, several plans, max speed
const {chromium}=require('playwright');const SP=__dirname+'/';const N=+process.argv[2]||600,SEC=+process.argv[3]||20;
(async()=>{const b=await chromium.launch();const p=await b.newPage({viewport:{width:1200,height:600}});const errs=[];p.on('pageerror',e=>errs.push(e.message));
await p.goto('http://localhost:8765/index.html');await p.waitForFunction(()=>!document.getElementById('load'),null,{timeout:120000});await p.waitForFunction(()=>REG,null,{timeout:60000});
const r=await p.evaluate(async([N,SEC])=>{localStorage.clear();closeModal();window.toast=()=>{};const pi=NAT.findIndex(n=>n.iso==='ru');newGame(pi);setSpeed(0);
  const names=['moveDivs','planTick','buildArea','coverTick','coverFront','frontGeom','occSmooth','zoneFlush','cutFlush','stepPockets','dayTick','hourTick','gridRoute','orderDiv','planBubble','occTrail','renderScene','drawDivs','drawFronts','queuePocket','anxLines','capUpdate','updHud','frame','loop','glFrame','drawLabels','drawCities','drawAnxBorders','drawSel','renderScene','refreshPanelLight','clockStr','drawFill','drawLine'];
  const P={};for(const n of names){const f=window[n];if(typeof f!=='function')continue;P[n]={t:0,c:0,m:0};window[n]=function(){const t=performance.now();try{return f.apply(this,arguments)}finally{const dd=performance.now()-t;P[n].t+=dd;P[n].c++;if(dd>P[n].m)P[n].m=dd}}}
  const targets=['ua','kz','mn','by','ge'].map(i=>NAT.findIndex(n=>n.iso===i)).filter(i=>i>=0);const m=terr(pi);
  for(const t of targets)act('war',t);
  const per=Math.ceil(N/targets.length/10);
  for(const t of targets){const before=new Set(G.divs.map(d=>d.id));const near=m.slice().sort((a,b)=>{const q=terr(t)[0];return Math.hypot(LP[2*a]-LP[2*q],LP[2*a+1]-LP[2*q+1])-Math.hypot(LP[2*b]-LP[2*q],LP[2*b+1]-LP[2*q+1])});
    for(let i=0;i<per;i++)testDivs(near[i%5],0);SELS=new Set(G.divs.filter(d=>!before.has(d.id)).map(d=>d.id));selDiv=null;const q=terr(t)[0];frontOnNation(LP[2*q],LP[2*q+1])}
  for(const k in P){P[k].t=0;P[k].c=0;P[k].m=0}
  setSpeed(4);await new Promise(r=>setTimeout(r,SEC*400));
  // plans: line through enemy centroid
  for(const f of G.fronts){const rs=terr(f.t);if(!rs.length)continue;let cx=0,cy=0;for(const r of rs){cx+=LP[2*r];cy+=LP[2*r+1]}cx/=rs.length;cy/=rs.length;f.plan=[];for(let a=-1;a<=1;a+=.1)f.plan.push([cx+a*8,cy+Math.sin(a*3)])}
  execPlans();const h0=G.hour;let fr=0,worst=0,lt=performance.now();const cnt=()=>{const n=performance.now();worst=Math.max(worst,n-lt);lt=n;fr++;if(window.__run)requestAnimationFrame(cnt)};window.__run=1;requestAnimationFrame(cnt);
  const t0=performance.now();await new Promise(r=>setTimeout(r,SEC*600));window.__run=0;const el=(performance.now()-t0)/1000;setSpeed(0);
  const out={};for(const k in P)if(P[k].c)out[k]=[+(P[k].t).toFixed(0),P[k].c,Math.round(P[k].m)];
  return{divs:G.divs.length,hours:G.hour-h0,el:+el.toFixed(1),fps:+(fr/el).toFixed(1),worstMs:Math.round(worst),prof:out}},[N,SEC]);
console.log(JSON.stringify(r,null,0),errs.slice(0,3));await b.close()})();
