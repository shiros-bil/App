// determinism / equivalence check: seeded scenario, fixed ticks, fake clock -> hash of control grid + divisions
const {chromium}=require('playwright');
(async()=>{const b=await chromium.launch();const p=await b.newPage({viewport:{width:800,height:500}});const errs=[];p.on('pageerror',e=>errs.push(e.message));
await p.goto('http://localhost:8765/index.html');await p.waitForFunction(()=>!document.getElementById('load'),null,{timeout:120000});await p.waitForFunction(()=>REG,null,{timeout:60000});
const r=await p.evaluate(()=>{let s=12345;Math.random=()=>{s=(s*1103515245+12345)&0x7fffffff;return s/0x7fffffff};let clk=0;const rn=performance.now.bind(performance);performance.now=()=>clk;
  localStorage.clear();closeModal();window.toast=()=>{};const pi=NAT.findIndex(n=>n.iso==='cn'),mn=NAT.findIndex(n=>n.iso==='mn'),kz=NAT.findIndex(n=>n.iso==='kz');newGame(pi);setSpeed(0);act('war',mn);act('war',kz);
  const m=terr(pi);const t0=rn();
  for(const [t,cnt] of [[mn,4],[kz,3]]){const before=new Set(G.divs.map(d=>d.id));const q=terr(t)[0];const near=m.slice().sort((a,b)=>Math.hypot(LP[2*a]-LP[2*q],LP[2*a+1]-LP[2*q+1])-Math.hypot(LP[2*b]-LP[2*q],LP[2*b+1]-LP[2*q+1]));
    for(let i=0;i<cnt;i++)testDivs(near[i],0);SELS=new Set(G.divs.filter(d=>!before.has(d.id)).map(d=>d.id));selDiv=null;frontOnNation(LP[2*q],LP[2*q+1])}
  const tick=n=>{for(let i=0;i<n;i++){clk+=50;hourTick();if(i%3==0)occSmooth(pi);stepPockets()}};tick(150);
  for(const f of G.fronts){const rs=terr(f.t);let cx=0,cy=0;for(const r of rs){cx+=LP[2*r];cy+=LP[2*r+1]}cx/=rs.length;cy/=rs.length;f.plan=[];for(let a=-1;a<=1.001;a+=.1)f.plan.push([cx+a*10,cy+Math.sin(a*3)])}
  execPlans();tick(500);for(let i=0;i<100;i++){clk+=2000;stepPockets()}
  let h=0;for(let i=0;i<CTL.length;i++){const c=REG[i]?ctrlOf(i):0;h=(h*31+c+1)|0}let hd=0;for(const d of G.divs)hd=(hd*31+Math.round(d.x*1000)*7+Math.round(d.y*1000))|0;
  return{hash:h,divHash:hd,ms:Math.round(rn()-t0),exec:G.fronts.map(f=>f.exec)}});
console.log(JSON.stringify(r),errs.slice(0,3));await b.close()})();
