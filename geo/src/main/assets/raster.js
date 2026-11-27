// Rasterizes every region into a cell grid (value = region index + 1, 0 = water) for occupation painting.
const MW=2000,MH=1000;
onmessage=async e=>{const GW=e.data.GW,GH=e.data.GH,topo=await (await fetch('world_mid.json')).json();
  const tr=topo.transform,sx=tr.scale[0],sy=tr.scale[1],tx=tr.translate[0],ty=tr.translate[1];
  const arcs=topo.arcs.map(a=>{const f=new Float32Array(a.length*2);let x=0,y=0;for(let k=0;k<a.length;k++){x+=a[k][0];y+=a[k][1];f[2*k]=(x*sx+tx+180)/360*GW;f[2*k+1]=(90-(y*sy+ty))/180*GH}return f});
  const grid=new Uint16Array(GW*GH),G=topo.objects.r.geometries;
  G.forEach((g,r)=>{const polys=g.type==='Polygon'?[g.arcs]:g.type==='MultiPolygon'?g.arcs:[];
    for(const poly of polys){const rows=new Map();
      for(const ring of poly){let px=null,py=null;
        for(const ai of ring){const a=ai<0?~ai:ai,f=arcs[a],m=f.length/2;
          for(let k=0;k<m;k++){const j=ai<0?m-1-k:k,x=f[2*j],y=f[2*j+1];
            if(px!==null&&py!==y){const y0=Math.min(py,y),y1=Math.max(py,y);let j0=Math.ceil(y0-.5),j1=Math.ceil(y1-.5);
              for(let row=Math.max(0,j0);row<Math.min(GH,j1);row++){const yc=row+.5,cx=px+(yc-py)*(x-px)/(y-py);let l=rows.get(row);if(!l)rows.set(row,l=[]);l.push(cx)}}
            px=x;py=y}}}
      for(const[row,l]of rows){l.sort((a,b)=>a-b);const o=row*GW;
        for(let i=0;i+1<l.length;i+=2){const a=Math.max(0,Math.ceil(l[i]-.5)),b=Math.min(GW,Math.ceil(l[i+1]-.5));for(let c=a;c<b;c++)grid[o+c]=r+1}}}});
  postMessage(grid,[grid.buffer])};
