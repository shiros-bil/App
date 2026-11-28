// pure simulation benchmark (no render): N divisions on 5 fronts, plans, ticks driven directly; per-system ms/tick
const {chromium}=require('playwright');const PORT=process.argv[2]||8765,N=+process.argv[3]||600;
(async()=>{const b=await chromium.launch();const p=await b.newPage({viewport:{width:800,height:500}});const errs=[];p.on('pageerror',e=>errs.push(e.message));
await p.goto(`http://localhost:${PORT}/index.html`);await p.waitForFunction(()=>!document.getElementById('load'),null,{timeout:120000});await p.waitForFunction(()=>REG,null,{timeout:60000});
const r=await p.evaluate(([N])=>{let s=777;Math.random=()=>{s=(s*1103515245+12345)&0x7fffffff;return s/0x7fffffff};
  localStorage.clear();closeModal();window.toast=()=>{};const pi=NAT.findIndex(n=>n.iso==='ru');newGame(pi);setSpeed(0);
  const names=['moveDivs','planTick','coverTick','occSmooth','gridRoute','dayTick','hourTick','buildArea','zoneFlush','cutFlush'];const P={};
  for(const n of names){const f=window[n];P[n]={t:0,c:0,m:0};window[n]=function(){const t=performance.now();try{return f.apply(this,arguments)}finally{const d=performance.now()-t;P[n].t+=d;P[n].c++;if(d>P[n].m)P[n].m=d}}}
  const targets=['ua','kz','mn','by','ge'].map(i=>NAT.findIndex(n=>n.iso===i));for(const t of targets)act('war',t);const m=terr(pi),per=Math.ceil(N/50);
  for(const t of targets){const before=new Set(G.divs.map(d=>d.id));const q=terr(t)[0];const near=m.slice().sort((a,b)=>Math.hypot(LP[2*a]-LP[2*q],LP[2*a+1]-LP[2*q+1])-Math.hypot(LP[2*b]-LP[2*q],LP[2*b+1]-LP[2*q+1]));
    for(let i=0;i<per;i++)testDivs(near[i%5],0);SELS=new Set(G.divs.filter(d=>!before.has(d.id)).map(d=>d.id));selDiv=null;frontOnNation(LP[2*q],LP[2*q+1])}
  const run=n=>{for(let i=0;i<n;i++){hourTick();if(i%3==0)occSmooth(pi);if(i%4==0){zoneFlush(false);}if(i%12==0)cutFlush()}};
  run(120);for(const f of G.fronts){const rs=terr(f.t);let cx=0,cy=0;for(const r of rs){cx+=LP[2*r];cy+=LP[2*r+1]}cx/=rs.length;cy/=rs.length;f.plan=[];for(let a=-1;a<=1.001;a+=.1)f.plan.push([cx+a*8,cy+Math.sin(a*3)])}
  for(const k in P){P[k].t=0;P[k].c=0;P[k].m=0}const t0=performance.now();execPlans();run(300);const tot=performance.now()-t0;
  const out={};for(const k in P)if(P[k].c)out[k]={ms:+P[k].t.toFixed(0),calls:P[k].c,perCall:+(P[k].t/P[k].c).toFixed(2),max:Math.round(P[k].m)};
  let h=0;for(let i=0;i<CTL.length;i++){const c=REG[i]?ctrlOf(i):0;h=(h*31+c+1)|0}
  return{divs:G.divs.length,totalMs:Math.round(tot),msPerTick:+(tot/300).toFixed(2),hash:h,out}},[N]);
console.log(JSON.stringify(r),errs.slice(0,2));await b.close()})();
