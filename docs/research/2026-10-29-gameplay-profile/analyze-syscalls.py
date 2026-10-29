#!/usr/bin/env python3
"""Aggregate completed strace -f -tt -T calls by TID and syscall, retaining idle waits."""
import re,json,sys,math,statistics
from collections import defaultdict
from pathlib import Path
p=Path(sys.argv[1]);groups=defaultdict(list)
main_tid=None
for line in p.open():
    m=re.match(r'(\d+)\s+\S+\s+(?:(\w+)\(|<\.\.\. (\w+) resumed>)',line)
    duration=re.search(r'<(\d+\.\d+)>$',line)
    if not m or not duration: continue
    tid=int(m[1]);main_tid=tid if main_tid is None else min(main_tid,tid)
    groups[(tid,m[2] or m[3])].append(float(duration[1])*1000)
result={'main_tid':main_tid,'groups':[]}
for (tid,name),v in sorted(groups.items()):
    v.sort()
    result['groups'].append(dict(tid=tid,syscall=name,n=len(v),sum_ms=sum(v),mean_ms=statistics.mean(v),p95_ms=v[math.ceil(.95*len(v))-1],max_ms=v[-1]))
Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n')
print('Main TID:',main_tid)
for g in result['groups']:
    if g['tid']==main_tid: print(g)
print('Non-main ioctl/poll/sleep:')
for g in result['groups']:
    if g['tid']!=main_tid and g['syscall'] not in ['futex','sched_yield']: print(g)
