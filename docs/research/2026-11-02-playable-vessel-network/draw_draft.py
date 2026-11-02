#!/usr/bin/env python3
"""Design-only SVG source. No game map/config loader uses these coordinates."""
from pathlib import Path
import math, re, html, json
ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[2]
roads=[]
def road(name,phase,width,pts):
    roads.append(dict(name=name,phase=phase,width=width,pts=pts))
# Author coordinates = original heart coordinates translated by (+450,+650).
P=lambda x,y:(x+450,y+650)
# Preserve actual narrow internal paths; these are not the new 24/12/6/3 external hierarchy.
for name,pts,w in [
 ('体回接口—右房',[(38,-4),(38,32),(45,45)],10),
 ('右房—右室',[(45,45),(50,62),(49,78),(49,86)],10),
 ('右室内部导引',[(49,86),(49,110),(61,170),(62,100),(69,85)],10),
 ('右室—肺出接口',[(69,85),(78,65),(78,55),(78,-4)],9),
 ('肺回接口—左房',[(112,-4),(112,45)],10),
 ('左房—左室',[(112,45),(108,63),(106,78),(106,86)],10),
 ('左室内部导引',[(106,86),(106,176),(123,106),(132,86)],10),
 ('左室—体出接口',[(132,86),(150,65),(150,-4)],10)]: road(name,'heart',w,[P(*p) for p in pts])
road('肺出接近段','ps',24,[(528,646),(528,550)])
road('肺供西廊','ps',12,[(528,550),(480,500),(440,400),(450,220),(490,140)])
road('肺西外环','ps',12,[(450,220),(400,260),(400,360),(440,400)])
road('肺回接近段','pr',24,[(562,646),(562,550)])
road('肺回东廊','pr',12,[(562,550),(620,500),(660,400),(650,220),(610,140)])
road('肺东外环','pr',12,[(650,220),(700,270),(700,360),(660,400)])
road('肺上台地北路','lung',6,[(490,140),(520,100),(565,115),(585,90),(610,140)])
road('肺上台地南路','lung',6,[(490,140),(520,180),(570,170),(610,140)])
road('肺中部曲路','lung',6,[(440,400),(480,350),(530,370),(560,320),(615,350),(660,400)])
road('肺中部南路','lung',6,[(440,400),(490,420),(540,400),(590,430),(660,400)])
road('肺下口北路','lung',6,[(480,500),(490,460),(530,450),(580,470),(620,500)])
road('肺下口南路','lung',6,[(480,500),(500,490),(540,480),(580,490),(620,500)])
road('肺中央纵街','lung',6,[(570,170),(590,230),(555,260),(560,320)])
road('中央汇口西入口','lung',6,[(450,220),(490,240),(520,250),(555,260)])
road('中央汇口东入口','lung',6,[(650,220),(620,240),(555,260)])
road('上台地窄联络','lung',3,[(565,115),(555,140),(570,170)])
road('体供外围大道','bs',24,[(600,646),(700,600),(850,650),(940,820),(880,980),(600,1030),(320,970),(180,800),(210,600),(300,450)])
road('东侧供血支环','bs',12,[(850,650),(1000,680),(1030,810),(940,820)])
road('南侧供血支环','bs',12,[(600,1030),(470,1100),(310,1060),(320,970)])
road('西北供血支环','bs',12,[(210,600),(130,540),(180,420),(300,450)])
road('体回内侧大道','br',24,[(488,646),(400,600),(340,700),(380,850),(600,910),(780,850),(820,700)])
road('南内回流支环','br',12,[(380,850),(400,910),(450,940),(500,950),(560,940),(600,910)])
road('东内回流支环','br',12,[(780,850),(720,760),(740,670),(820,700)])
road('A上折路','local',6,[(940,820),(900,800),(860,760),(850,720),(820,700)])
road('A下折路','local',6,[(940,820),(900,870),(850,850),(820,870),(780,850)])
road('A南接近','local',6,[(880,980),(850,930),(800,920),(760,900),(780,850)])
road('B西弯路','local',6,[(600,1030),(550,1000),(565,970),(600,910)])
road('B东弯路','local',6,[(600,1030),(640,1000),(650,950),(600,910)])
road('B窄联络','local',3,[(565,970),(600,985),(650,950)])
road('C北弯路','local',6,[(300,450),(320,500),(320,550),(360,570),(400,600)])
road('C南弯路','local',6,[(210,600),(230,650),(280,670),(340,700)])
road('C中街','local',6,[(320,550),(330,600),(280,670)])
COL={'ps':'#447cc0','pr':'#ce6573','bs':'#cf715a','br':'#586f9c','lung':'#a66c92','local':'#8a7685','heart':'#c89b9c'}
# Read original cubic outlines verbatim, avoiding a hand-drawn approximation.
src=(REPO/'crates/simulation/src/map/heart.rs').read_text()
chambers=[]
for m in re.finditer(r'outline\(\s*"([a-z_]+)",\s*\[([\d.]+),\s*([\d.]+)\],\s*&\[(.*?)\n\s*\],\s*\)',src,re.S):
    ns=[float(x) for x in re.findall(r'[\d.]+',m[4])]
    curves=[ns[i:i+6] for i in range(0,len(ns),6)]
    chambers.append((m[1],(float(m[2]),float(m[3])),curves))
assert len(chambers)==4

def cross(a,b,c): return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
def on(a,b,p): return abs(cross(a,b,p))<1e-8 and min(a[0],b[0])<=p[0]<=max(a[0],b[0]) and min(a[1],b[1])<=p[1]<=max(a[1],b[1])
def intersect(a,b,c,d):
    return (cross(a,b,c)*cross(a,b,d)<0 and cross(c,d,a)*cross(c,d,b)<0) or any([on(a,b,c),on(a,b,d),on(c,d,a),on(c,d,b)])
def pdist(p,a,b):
    dx,dy=b[0]-a[0],b[1]-a[1]; t=max(0,min(1,((p[0]-a[0])*dx+(p[1]-a[1])*dy)/(dx*dx+dy*dy)))
    return math.dist(p,(a[0]+t*dx,a[1]+t*dy))
def sampled_chambers():
    result={}
    for name,start,curves in chambers:
        a=start; polygon=[P(*a)]
        for v in curves:
            b,c,d=(v[0:2],v[2:4],v[4:6])
            for i in range(1,33):
                t=i/32;u=1-t
                polygon.append(P(*(u*u*u*a[j]+3*u*u*t*b[j]+3*u*t*t*c[j]+t*t*t*d[j] for j in range(2))))
            a=d
        result[name]=polygon
    return result

def inside(p,poly):
    odd=False
    for a,b in zip(poly,poly[1:]+poly[:1]):
        if (a[1]>p[1])!=(b[1]>p[1]) and p[0]<(b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0]:odd=not odd
    return odd

def chamber_audit():
    polys=sampled_chambers(); issues=[]; min_gap=1e9; guide_issues=[]
    allowed=[{'right_atrium'},{'right_atrium','right_ventricle'},{'right_ventricle'},{'right_ventricle'},{'left_atrium'},{'left_atrium','left_ventricle'},{'left_ventricle'},{'left_ventricle'}]
    for i,r in enumerate(roads):
      for name,poly in polys.items():
       if r['phase']=='heart' and name in allowed[i]:continue
       for a,b in zip(r['pts'],r['pts'][1:]):
        distance=1e9
        for c,d in zip(poly,poly[1:]+poly[:1]):
         if c==d:continue
         v=0 if intersect(a,b,c,d) else min(pdist(a,c,d),pdist(b,c,d),pdist(c,a,b),pdist(d,a,b))
         distance=min(distance,v)
        gap=distance-r['width']/2
        overlaps=inside(a,poly) or inside(b,poly) or gap<0
        if r['phase']!='heart':
         min_gap=min(min_gap,gap)
         if overlaps:issues.append([r['name'],name,round(gap,3)])
        elif overlaps:guide_issues.append([r['name'],name,round(gap,3)])
    port_issues=[]
    for r in roads[:8]:
      for s in roads[8:]:
       for a,b in zip(r['pts'],r['pts'][1:]):
        for c,d in zip(s['pts'],s['pts'][1:]):
         if set([a,b])&set([c,d]):continue
         distance=0 if intersect(a,b,c,d) else min(pdist(a,c,d),pdist(b,c,d),pdist(c,a,b),pdist(d,a,b))
         if distance<(r['width']+s['width'])/2:port_issues.append([r['name'],s['name']])
    return {'unintended_external_narrow_guide_overlap':port_issues,'external_tube_chamber_overlap':issues,'minimum_external_tube_chamber_gap':round(min_gap,3),'narrow_guide_unrelated_chamber_overlap':guide_issues}

def audit():
    # Intracardiac out-and-back guides lie inside filled chambers, not separate roads.
    ext=[r for r in roads if r['phase']!='heart']; bad=[]; near=[]; junctions=set()
    for i,r in enumerate(ext):
      for s in ext[i+1:]:
       for a,b in zip(r['pts'],r['pts'][1:]):
        for c,d in zip(s['pts'],s['pts'][1:]):
         shared=set([a,b])&set([c,d])
         if shared: junctions.update(shared); continue
         if intersect(a,b,c,d): bad.append([r['name'],s['name'],a,b,c,d])
         else:
          gap=min(pdist(a,c,d),pdist(b,c,d),pdist(c,a,b),pdist(d,a,b))-(r['width']+s['width'])/2
          if gap<2: near.append([r['name'],s['name'],round(gap,2)])
    # Vertices include polyline turns. Only degree >=3 positions are choice junctions.
    graph={}; counts={}
    for r in ext:
      for a,b in zip(r['pts'],r['pts'][1:]):
       graph.setdefault(a,set()).add(b); graph.setdefault(b,set()).add(a)
      for p in r['pts']: counts.setdefault(p,set()).add(r['phase'])
    def reach(start,blocked=set()):
      seen=set(); todo=[start]
      while todo:
       p=todo.pop()
       if p in seen or p in blocked:continue
       seen.add(p);todo.extend(graph.get(p,[])-seen)
      return seen
    # Remove explicitly designated organ transfer roads, verify arterial/venous trunks isolated.
    pure={}
    for r in ext:
      if r['phase'] in ('lung','local'):continue
      for a,b in zip(r['pts'],r['pts'][1:]):pure.setdefault(a,set()).add(b);pure.setdefault(b,set()).add(a)
    def pure_reach(p):
      seen=set();todo=[p]
      while todo:
       q=todo.pop()
       if q in seen:continue
       seen.add(q);todo.extend(pure.get(q,set())-seen)
      return seen
    ports=[(528,646),(562,646),(600,646),(488,646)]
    junctions={p for p,neighbors in graph.items() if len(neighbors)>=3}
    self_cross=[]
    for r in ext:
      segs=list(zip(r['pts'],r['pts'][1:]))
      for k,(a,b) in enumerate(segs):
       for j,(c,d) in enumerate(segs[k+2:],k+2):
        if not set([a,b])&set([c,d]) and intersect(a,b,c,d):self_cross.append([r['name'],k,j])
    pockets=[]
    for cut in graph:
      remaining=set(graph)-{cut}
      while remaining:
        todo=[next(iter(remaining))];component=set()
        while todo:
          p=todo.pop()
          if p==cut or p in component:continue
          component.add(p);todo.extend(graph[p]-component-{cut})
        remaining-=component
        if not component.intersection(ports):pockets.append({'attachment':cut,'vertices':len(component)})
    result={'single_attachment_portless_pockets':pockets,'self_intersections':self_cross,'independent_cycles':sum(len(r['pts'])-1 for r in ext)-len(graph)+2,'external_roads':len(ext),'external_vertices':len(graph),'external_segments':sum(len(r['pts'])-1 for r in ext),'shared_junctions':len(junctions),'unshared_centerline_intersections':bad,'nonincident_segment_gap_below_2':near,'pulmonary_external_connected':ports[1] in reach(ports[0]),'body_external_connected':ports[3] in reach(ports[2]),'pulmonary_and_body_external_disjoint':ports[2] not in reach(ports[0]),'pulmonary_no_trunk_shunt':ports[1] not in pure_reach(ports[0]),'body_no_trunk_shunt':ports[3] not in pure_reach(ports[2])}
    result.update(chamber_audit())
    import heapq
    estimates=[]
    for title,start,end in [('肺出→下口西入口',(528,646),(480,500)),('肺出→中央汇口',(528,646),(555,260)),('肺出→上台地西入口',(528,646),(490,140)),('体出→A外围中入口',(600,646),(940,820)),('体出→B外围入口',(600,646),(600,1030)),('体回→C外围入口',(488,646),(210,600))]:
      best={start:0};queue=[(0,start)]
      while queue:
        length,p=heapq.heappop(queue)
        if p==end:
          estimates.append({'route':title,'distance':round(length,1),'seconds_at_4':round(length/4,1)});break
        if length!=best[p]:continue
        for q in graph[p]:
          candidate=length+math.dist(p,q)
          if candidate<best.get(q,math.inf):best[q]=candidate;heapq.heappush(queue,(candidate,q))
    result['centerline_walk_estimates']=estimates
    (ROOT/'audit.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(result,ensure_ascii=False,indent=2))
    return junctions

def label(x,y,s,size=14,color='#26384b',anchor='start'):
    return f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}" text-anchor="{anchor}">{html.escape(s)}</text>'

def render(filename,detail=False):
    # SVG coordinates use the same unit as production geometry; Y increases down.
    box=(350,40,410,830) if detail else (80,35,1010,1100)
    x,y,w,h=box
    out=[f'<svg xmlns="http://www.w3.org/2000/svg" width="{1230 if not detail else 960}" height="{1430 if not detail else 1740}" viewBox="{x} {y-100} {w} {h+230}">', '<rect x="-200" y="-200" width="1800" height="1800" fill="#f5f2eb"/>','<style>text{font-family:"Noto Sans CJK SC","DejaVu Sans",sans-serif}path,polyline{stroke-linejoin:round;stroke-linecap:round}</style>']
    out += [label(x+10,y-61,'心肺街区详图' if detail else '可玩血管路网候选 A',25),label(x+10,y-31,'设计草图，未实装｜真实管宽比例｜无跨层道路',13)]
    out.append(f'<defs><clipPath id="plot"><rect x="{x}" y="{y}" width="{w}" height="{h}"/></clipPath></defs><g clip-path="url(#plot)">')
    # Reserved color fields are NOT fluid chambers.
    zones=[(375,65,355,475,'肺部：上台地 / 中部 / 下口','#e6dce3'),(795,665,210,310,'器官预留区 A','#e9dfce'),(480,925,220,120,'器官预留区 B','#e2e6d8'),(200,540,205,170,'器官预留区 C','#dfe5e5')]
    for zx,zy,zw,zh,name,col in zones:
      if detail and name.startswith('器官'):continue
      out.append(f'<rect x="{zx}" y="{zy}" width="{zw}" height="{zh}" rx="28" fill="{col}" stroke="#bbb9af" stroke-dasharray="4 5"/>')
      tx,ty=(490,1040) if name.endswith(' B') else ((980,900) if name.endswith(' A') else (zx+12,zy+22))
      out.append(label(tx,ty,name,11))
    # External centerlines; diameter is world-unit width, not symbolic line weight.
    for r in roads:
      if r['phase']=='heart':continue
      pts=' '.join(f'{a},{b}' for a,b in r['pts'])
      out.append(f'<polyline points="{pts}" fill="none" stroke="#faf7f1" stroke-width="{r["width"]+3}"/>')
      out.append(f'<polyline points="{pts}" fill="none" stroke="{COL[r["phase"]]}" stroke-width="{r["width"]}"><title>{r["name"]}，直径{r["width"]}</title></polyline>')
    for name,start,curves in chambers:
      path=f'M {start[0]+450},{start[1]+650} '
      for v in curves:path+='C '+' '.join(f'{v[i]+450},{v[i+1]+650}' for i in (0,2,4))+' '
      out.append(f'<path d="{path} Z" fill="{ "#b8cee2" if name.startswith("right") else "#ecc4bd"}" stroke="#675e65" stroke-width="1.5"/>')
    for r in roads:
      if r['phase']!='heart':continue
      pts=' '.join(f'{a},{b}' for a,b in r['pts'])
      out.append(f'<polyline points="{pts}" fill="none" stroke="#ad8589" stroke-width="{r["width"]}"/>')
    for xx,yy,t in [(495,699,'右房'),(562,699,'左房'),(497,780,'右室'),(561,780,'左室')]:out.append(label(xx,yy,t,10,anchor='middle'))
    # Mark all true shared vertices; colors never imply a crossing bypass.
    for xx,yy in junctions:
      out.append(f'<circle cx="{xx}" cy="{yy}" r="2.5" fill="#f9f7f1" stroke="#564c55" stroke-width=".8"/>')
    names=[(383,245,'西外环'),(670,245,'东外环'),(473,204,'上台地双路'),(543,286,'中央汇口'),(470,335,'中部曲路'),(490,440,'下口双路'),(507,595,'肺出'),(570,595,'肺回'),(455,638,'体回'),(605,638,'体出')]
    if not detail:names += [(970,850,'东支环'),(350,1085,'南支环'),(125,400,'西北支环'),(405,930,'南内环'),(728,650,'东内环'),(865,730,'A 上折路'),(825,898,'A 南接近'),(548,995,'B 双弯路'),(238,620,'C 双入口')]
    for xx,yy,t in names:out.append(label(xx,yy,t,10))
    # Window and nominal viewport are different: 1280/48 by 800/48 world units.
    out.append('<rect x="610" y="740" width="80" height="80" fill="none" stroke="#8b9294" stroke-dasharray="3 3"/>')
    out.append('<rect x="624" y="757" width="26.6667" height="16.6667" fill="#ffffff" fill-opacity=".4" stroke="#54636b" stroke-width=".8"/>')
    out += [label(610,833,'80×80 地图/实体窗口',8),label(610,846,'5×5 个16单位区块',8),label(610,859,'小框：zoom2 一屏约26.7×16.7',8)]
    out.append('</g>')
    foot=y+h+25
    out.append(f'<path d="M {x+12},{foot} h 100 m -100,-4 v 8 m 100,-8 v 8" stroke="#334153" stroke-width="1.5"/>')
    out += [label(x+12,foot+18,'100 世界单位；主动速度4时约25秒',10),label(x+12,foot+40,'24 主干　12 周边环路　6 局部街路　3 少量窄联络',10),label(x+12,foot+59,'蓝/玫红：肺供/肺回；橙/靛蓝：体供/体回；紫：明确供回连接区',9),label(x+12,foot+77,'空心点为真实路口；阴影仅表示区域预留，不是流体腔室。',9),label(x+12,foot+95,'血流方向/速度待统一求解；绘图未完成 SDF 净空及可玩性验收。',9)]
    out.append('</svg>')
    (ROOT/filename).write_text('\n'.join(out))

if __name__=='__main__':
    junctions=audit()
    render('network-draft.svg');render('heart-lung-detail.svg',True)
    import sys
    if '--png' in sys.argv:
        import gi, cairo
        gi.require_version('Rsvg','2.0')
        from gi.repository import Rsvg
        for name in ('network-draft','heart-lung-detail'):
            handle=Rsvg.Handle.new_from_file(str(ROOT/(name+'.svg')))
            _,w,h=handle.get_intrinsic_size_in_pixels()
            surface=cairo.ImageSurface(cairo.FORMAT_ARGB32,int(w),int(h))
            rect=Rsvg.Rectangle();rect.x=0;rect.y=0;rect.width=w;rect.height=h
            handle.render_document(cairo.Context(surface),rect)
            surface.write_to_png(str(ROOT/(name+'.png')))
