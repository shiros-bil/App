// Builds GPU-ready geometry off the main thread.
importScripts('earcut.min.js','topojson-client.min.js');
const MW=2000,MH=1000,GX=32,GY=16,NC=GX*GY,cw=MW/GX,ch=MH/GY;
const cell=(x,y)=>Math.min(GY-1,Math.max(0,(y/ch)|0))*GX+Math.min(GX-1,Math.max(0,(x/cw)|0));
function decode(topo){const tr=topo.transform,sx=tr.scale[0],sy=tr.scale[1],tx=tr.translate[0],ty=tr.translate[1];
  return topo.arcs.map(a=>{const f=new Float32Array(a.length*2);let x=0,y=0;for(let k=0;k<a.length;k++){x+=a[k][0];y+=a[k][1];f[2*k]=(x*sx+tx+180)*MW/360;f[2*k+1]=(90-(y*sy+ty))*MH/180}return f})}
class FA{constructor(T,n){this.T=T;this.a=new T(n||1024);this.n=0}push(v){if(this.n===this.a.length){const b=new this.T(this.a.length*2);b.set(this.a);this.a=b}this.a[this.n++]=v}get(){return this.a.subarray(0,this.n)}}
function fills(pos,rid,idx){const n=idx.length,cnt=new Uint32Array(NC),cellOf=new Uint16Array(n/3),fbb=new Float32Array(NC*4);for(let c=0;c<NC;c++){fbb[4*c]=1e9;fbb[4*c+1]=1e9;fbb[4*c+2]=-1e9;fbb[4*c+3]=-1e9}
  for(let t=0,q=0;t<n;t+=3,q++){const a=idx[t],b=idx[t+1],c=idx[t+2];const xa=pos[2*a],ya=pos[2*a+1],xb=pos[2*b],yb=pos[2*b+1],xc=pos[2*c],yc=pos[2*c+1];
    const k=cell((xa+xb+xc)/3,(ya+yb+yc)/3);cellOf[q]=k;cnt[k]+=3;
    const x0=Math.min(xa,xb,xc),x1=Math.max(xa,xb,xc),y0=Math.min(ya,yb,yc),y1=Math.max(ya,yb,yc);if(x0<fbb[4*k])fbb[4*k]=x0;if(y0<fbb[4*k+1])fbb[4*k+1]=y0;if(x1>fbb[4*k+2])fbb[4*k+2]=x1;if(y1>fbb[4*k+3])fbb[4*k+3]=y1}
  const fr=new Uint32Array(NC*2);let o=0;for(let k=0;k<NC;k++){fr[2*k]=o;fr[2*k+1]=cnt[k];o+=cnt[k]}const w=new Uint32Array(NC);for(let k=0;k<NC;k++)w[k]=fr[2*k];
  const sidx=new Uint32Array(n);for(let t=0,q=0;t<n;t+=3,q++){const k=cellOf[q];sidx[w[k]++]=idx[t];sidx[w[k]++]=idx[t+1];sidx[w[k]++]=idx[t+2]}
  return{pos,rid,sidx,fr,fbb}}
function lines(seg,attr,an){const ns=seg.length/4,cellOf=new Uint16Array(ns),cnt=new Uint32Array(NC);for(let i=0;i<ns;i++){const k=cell((seg[4*i]+seg[4*i+2])/2,(seg[4*i+1]+seg[4*i+3])/2);cellOf[i]=k;cnt[k]++}
  const rg=new Uint32Array(NC*2),bb=new Float32Array(NC*4);let o=0;for(let k=0;k<NC;k++){rg[2*k]=o;rg[2*k+1]=cnt[k];o+=cnt[k];bb[4*k]=1e9;bb[4*k+1]=1e9;bb[4*k+2]=-1e9;bb[4*k+3]=-1e9}
  const w=new Uint32Array(NC);for(let k=0;k<NC;k++)w[k]=rg[2*k];const S=new Float32Array(seg.length),L=new Float32Array(ns*an);
  for(let i=0;i<ns;i++){const k=cellOf[i],q=w[k]++;for(let j=0;j<4;j++)S[4*q+j]=seg[4*i+j];for(let j=0;j<an;j++)L[an*q+j]=attr[an*i+j];
    const x0=Math.min(seg[4*i],seg[4*i+2]),x1=Math.max(seg[4*i],seg[4*i+2]),y0=Math.min(seg[4*i+1],seg[4*i+3]),y1=Math.max(seg[4*i+1],seg[4*i+3]);
    if(x0<bb[4*k])bb[4*k]=x0;if(y0<bb[4*k+1])bb[4*k+1]=y0;if(x1>bb[4*k+2])bb[4*k+2]=x1;if(y1>bb[4*k+3])bb[4*k+3]=y1}
  return{S,L,rg,bb}}
const tr=o=>{const l=[];for(const k in o){const v=o[k];if(v&&v.buffer)l.push(v.buffer);else if(v&&typeof v==='object')l.push(...tr(v))}return l};
let ARL=null,ARR=null;
function buildRegions(topo){const arcs=decode(topo),G=topo.objects.r.geometries;
  if(!ARL){ARL=new Int32Array(arcs.length).fill(-1);ARR=new Int32Array(arcs.length).fill(-1);
    G.forEach((g,r)=>{const polys=g.type==='Polygon'?[g.arcs]:g.type==='MultiPolygon'?g.arcs:[];for(const p of polys)for(const ring of p)for(const ai of ring){const a=ai<0?~ai:ai;if(ARL[a]<0)ARL[a]=r;else if(ARL[a]!==r)ARR[a]=r}})}
  const pos=new FA(Float32Array,1<<16),rid=new FA(Float32Array,1<<15),idx=new FA(Uint32Array,1<<16);let base=0;
  G.forEach((g,r)=>{const polys=g.type==='Polygon'?[g.arcs]:g.type==='MultiPolygon'?g.arcs:[];
    for(const poly of polys){const flat=[],holes=[];
      for(let ri=0;ri<poly.length;ri++){if(ri>0)holes.push(flat.length/2);const st=flat.length;let first=true;
        for(const ai of poly[ri]){const a=ai<0?~ai:ai,f=arcs[a],m=f.length/2;for(let k=0;k<m;k++){if(k===0&&!first)continue;const j=ai<0?m-1-k:k;flat.push(f[2*j],f[2*j+1])}first=false}
        if(flat.length-st>=4&&flat[flat.length-2]===flat[st]&&flat[flat.length-1]===flat[st+1])flat.length-=2}
      const tri=earcut(flat,holes.length?holes:null,2),n=flat.length/2;for(let k=0;k<n;k++){pos.push(flat[2*k]);pos.push(flat[2*k+1]);rid.push(r)}for(const t of tri)idx.push(base+t);base+=n}});
  const seg=new FA(Float32Array,1<<16),lr=new FA(Float32Array,1<<15);
  const seg2=new FA(Float32Array,1<<16),lr2=new FA(Float32Array,1<<15),NK=G.map(g=>g.properties.k);
  arcs.forEach((f,a)=>{const l=ARL[a],r=ARR[a]<0?65535:ARR[a];const minor=r!==65535&&NK[l]===NK[r];const S=minor?seg2:seg,LR=minor?lr2:lr;
    for(let k=0;k+3<f.length;k+=2){const x0=f[k],y0=f[k+1],x1=f[k+2],y1=f[k+3];
    if((x0<.05&&x1<.05)||(x0>MW-.05&&x1>MW-.05)||(y0>MH-.05&&y1>MH-.05))continue;S.push(x0);S.push(y0);S.push(x1);S.push(y1);LR.push(l);LR.push(r)}});
  return{fill:fills(pos.get(),rid.get(),idx.get()),line:lines(seg.get(),lr.get(),2),line2:lines(seg2.get(),lr2.get(),2)}}
function buildWater(t){const P=c=>[(c[0]+180)*MW/360,(90-c[1])*MH/180];
  const pos=new FA(Float32Array),rk=new FA(Float32Array),idx=new FA(Uint32Array),oseg=new FA(Float32Array),osr=new FA(Float32Array),seg=new FA(Float32Array),sr=new FA(Float32Array);let base=0;
  for(const f of topojson.feature(t,t.objects.l).features){const g=f.geometry,PP=g.type==='Polygon'?[g.coordinates]:g.coordinates,r=f.properties.r;
    for(const poly of PP){const flat=[],holes=[];poly.forEach((ring,i)=>{if(i)holes.push(flat.length/2);const q=ring.map(P);for(let k=0;k<q.length-1;k++)flat.push(q[k][0],q[k][1]);
        for(let k=1;k<q.length;k++){oseg.push(q[k-1][0]);oseg.push(q[k-1][1]);oseg.push(q[k][0]);oseg.push(q[k][1]);osr.push(r)}});
      const tri=earcut(flat,holes.length?holes:null,2),n=flat.length/2;for(let k=0;k<n;k++){pos.push(flat[2*k]);pos.push(flat[2*k+1]);rk.push(r)}for(const x of tri)idx.push(base+x);base+=n}}
  for(const f of topojson.feature(t,t.objects.v).features){const g=f.geometry,LS=g.type==='LineString'?[g.coordinates]:g.coordinates,r=f.properties.r;
    for(const l of LS){const q=l.map(P);for(let k=1;k<q.length;k++){seg.push(q[k-1][0]);seg.push(q[k-1][1]);seg.push(q[k][0]);seg.push(q[k][1]);sr.push(r)}}}
  const RG=[1,3,5,7,8,99],grp=r=>RG.findIndex(t=>r<=t);
  const splitL=(S,R)=>RG.map((_,g)=>{const s2=new FA(Float32Array),r2=new FA(Float32Array);for(let i=0;i<R.length;i++)if(grp(R[i])===g){for(let j=0;j<4;j++)s2.push(S[4*i+j]);r2.push(R[i])}return lines(s2.get(),r2.get(),1)});
  const PP0=pos.get(),RK=rk.get(),I=idx.get();const splitF=RG.map((_,g)=>{const i2=new FA(Uint32Array);for(let t=0;t<I.length;t+=3)if(grp(RK[I[t]])===g){i2.push(I[t]);i2.push(I[t+1]);i2.push(I[t+2])}return fills(PP0,RK,i2.get())});
  return{RG,fill:splitF,riv:splitL(seg.get(),sr.get()),ol:splitL(oseg.get(),osr.get())}}
onmessage=async e=>{const files=['world_lo.json','world_mid.json','world.json'];
  for(let i=0;i<3;i++){const t=await (await fetch(files[i])).json();const d=buildRegions(t);postMessage({k:'lod',i,d},tr(d))}
  const w=buildWater(await (await fetch('water.json')).json());postMessage({k:"water",d:w},[...new Set(tr(w))])};
