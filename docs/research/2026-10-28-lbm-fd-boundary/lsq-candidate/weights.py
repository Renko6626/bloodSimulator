#!/usr/bin/env python3
"""Fixed-radius, geometry-bound quadratic derivative weights for a research GPU path."""
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import numpy as np

RADIUS = 4

def build(path):
    raw = Path(path).read_bytes()
    assert raw[:8] == b'LSQGv1\0\0'
    n, periodic = struct.unpack_from('<II', raw, 8)
    total = n*n
    assert len(raw) == 16+2*total
    mask = np.frombuffer(raw, np.uint8, total, 16).reshape(n,n)
    side = np.frombuffer(raw, np.uint8, total, 16+total).reshape(n,n)
    def valid(x,y):
        return 0 <= x < n and (periodic or 0 <= y < n) and not mask[y%n,x]
    def visible(x,y,dx,dy):
        # Exact grid supercover from cell center to cell center; at corner ties
        # include both touched side cells, conservatively refusing wall crossings.
        cx,cy=x,y; sx=int(np.sign(dx)); sy=int(np.sign(dy))
        tx=.5/abs(dx) if dx else math.inf
        ty=.5/abs(dy) if dy else math.inf
        stepx=1/abs(dx) if dx else math.inf
        stepy=1/abs(dy) if dy else math.inf
        while (cx,cy)!=(x+dx,y+dy):
            if abs(tx-ty)<1e-12:
                if not valid(cx+sx,cy) or not valid(cx,cy+sy): return False
                cx+=sx;cy+=sy;tx+=stepx;ty+=stepy
            elif tx<ty: cx+=sx;tx+=stepx
            else: cy+=sy;ty+=stepy
            if not valid(cx,cy): return False
        return True
    headers=[(0,0,0.,0.) for _ in range(total)]
    entries=[]; stats=[]; rejected=0
    for y,x in np.argwhere(side):
        normal={1:(1,0),2:(-1,0),3:(0,1),4:(0,-1)}[int(side[y,x])]
        tangent=(-normal[1],normal[0]); points=[]; rows=[]; distance=[]
        for dy in range(-RADIUS,RADIUS+1):
            for dx in range(-RADIUS,RADIUS+1):
                r2=dx*dx+dy*dy
                if not r2 or r2>RADIUS**2: continue
                xx,yy=int(x+dx),int(y+dy)
                if not valid(xx,yy): continue
                kind=int(side[yy%n,xx]!=0)
                if kind and side[yy%n,xx]!=side[y,x]: continue
                if not visible(int(x),int(y),dx,dy): rejected+=1;continue
                s=(dx*normal[0]+dy*normal[1])/RADIUS
                z=(dx*tangent[0]+dy*tangent[1])/RADIUS
                points.append((yy%n*n+xx,kind));rows.append([s,z,.5*s*s,s*z,.5*z*z]);distance.append(1/math.sqrt(1+r2))
        a=np.array(rows); sw=np.array(distance)
        if len(points)<5: raise ValueError(f'unsupported ({x},{y}): only {len(points)} samples')
        inverse,_,rank,singular=np.linalg.lstsq(a*sw[:,None],np.eye(len(points)),rcond=1e-12)
        cond=float(singular[0]/singular[-1]) if rank==5 else math.inf
        if rank!=5 or cond>1000: raise ValueError(f'unsupported ({x},{y}): rank={rank} condition={cond}')
        weights=inverse[:2]*sw[None,:]/RADIUS
        headers[y*n+x]=(total+len(entries),len(points),0.,0.)
        entries.extend((i,k,float(wn),float(wt)) for (i,k),wn,wt in zip(points,weights[0],weights[1]))
        axial=all(valid(int(x+d*normal[0]),int(y+d*normal[1])) for d in (1,2))
        stats.append(dict(cell=[int(x),int(y)],samples=len(points),rank=int(rank),condition=cond,l1=[float(np.sum(abs(w))) for w in weights],axial_supported=bool(axial)))
    output=raw+b''.join(struct.pack('<IIff',*r) for r in headers+entries)
    Path(path+'.weights').write_bytes(output)
    summary=dict(geometry_sha256=hashlib.sha256(raw).hexdigest(),radius=RADIUS,openings=len(stats),samples_min=min(s['samples'] for s in stats),samples_max=max(s['samples'] for s in stats),condition_max=max(s['condition'] for s in stats),weight_l1_max=max(max(s['l1']) for s in stats),axial_unsupported=sum(not s['axial_supported'] for s in stats),wall_crossings_rejected=rejected,rows=stats)
    Path(path+'.stats.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k!='rows'}))

if __name__=='__main__':
    build(sys.argv[1])
