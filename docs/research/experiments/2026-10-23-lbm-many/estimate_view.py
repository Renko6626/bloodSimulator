"""Approximate birth-view count from a copied 2026-10-23 map snapshot.

This does not execute the production placement code or Godot. It is an
independent estimate, not a placement regression test. Window changes,
camera smoothing and floating-point boundaries may change the count.
"""
import math
N=[(-24,0),(-12,0),(0,-12),(18,-12),(0,12),(18,12),(30,0),(42,10),(36,26),(-16,26),(-28,12)]
E=[(0,1,None,3.5),(1,2,(-7,-11),3.1),(2,3,None,3.1),(3,6,(26,-10),3.1),(1,4,(-7,11),3.1),(4,5,None,3.1),(5,6,(26,10),3.1),(6,7,(41,0),3.5),(7,8,(46,23),3.5),(8,9,None,3.5),(9,10,(-27,25),3.5),(10,0,(-28,3),3.5)]
def dist(a,b):return math.hypot(a[0]-b[0],a[1]-b[1])
def path(e):
 a,b,c,_=e
 if c is None:return [N[a],N[b]]
 return [((1-t)**2*N[a][0]+2*t*(1-t)*c[0]+t*t*N[b][0],(1-t)**2*N[a][1]+2*t*(1-t)*c[1]+t*t*N[b][1]) for t in [i/16 for i in range(17)]]
P=[path(e) for e in E]
def edge_length(p):return sum(dist(a,b) for a,b in zip(p,p[1:]))
def point(p,along):
 for a,b in zip(p,p[1:]):
  length=dist(a,b)
  if along<=length:return (a[0]+(b[0]-a[0])*along/length,a[1]+(b[1]-a[1])*along/length)
  along-=length
 return p[-1]
def path_dist(p,q):
 best=1e9
 for a,b in zip(p,p[1:]):
  dx=b[0]-a[0];dy=b[1]-a[1]; t=max(0,min(1,((q[0]-a[0])*dx+(q[1]-a[1])*dy)/(dx*dx+dy*dy)))
  best=min(best,dist(q,(a[0]+t*dx,a[1]+t*dy)))
 return best
def signed(q):return min(path_dist(p,q)-e[3] for p,e in zip(P,E))
spawn=(-20,0); spacing=1.7;c=[]
for p in P:
 length=edge_length(p);count=max(1,math.floor(length/spacing))
 for i in range(count):
  q=point(p,length*(i+.5)/count)
  if signed(q)>-.86 or dist(q,spawn)<2.05 or any(dist(q,x)<spacing for x in c):continue
  c.append(q)
sel=[c[i*len(c)//72] for i in range(72)]
view=lambda q:(-20-1280/48<=q[0]<=-20+1280/48 and -800/48<=q[1]<=800/48)
print('candidates',len(c),'selected',len(sel),'visible_centers',sum(map(view,sel)))
print('selected by source edge',[(i,sum(map(view,[q for q in sel if min(range(12),key=lambda k:path_dist(P[k],q))==i]))) for i in range(12)])
print('view',(-20-1280/48,-20+1280/48,-800/48,800/48))
