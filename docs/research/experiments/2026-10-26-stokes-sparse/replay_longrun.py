#!/usr/bin/env python3
"""Targeted replay control for trajectory differences observed in the long pair."""
import argparse
import json
import os
from pathlib import Path
import subprocess

from run import ROOT, sha


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--results',type=Path,default=ROOT/'results/longrun')
    parser.add_argument('--seconds',type=float,default=30)
    args=parser.parse_args()
    manifest=json.loads((args.results/'manifest.json').read_text())
    binary=Path(manifest['temporary_workspaces'])/'strict-longrun'
    assert sha(binary)==manifest['binary_sha256']['strict']
    out=args.results/'replay-strict'
    out.mkdir()
    command=['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',
             str(binary),str(args.seconds),str(out.resolve())]
    env=dict(os.environ)
    env.pop('SPARSE_PROFILE',None)
    print('Running targeted strict replay',args.seconds,'seconds',flush=True)
    with (out/'run.log').open('w') as stdout,(out/'stderr.log').open('w') as stderr:
        proc=subprocess.run(command,stdout=stdout,stderr=stderr,env=env)
    (out/'manifest.json').write_text(json.dumps({'command':command,'seconds':args.seconds,
        'binary_sha256':sha(binary),'script_sha256':sha(Path(__file__)),
        'reason':'observed paired trajectory divergence; compare identical tolerance repeat before attributing it',
        'exit_code':proc.returncode},indent=2)+'\n')
    print('replay exit',proc.returncode,flush=True)
    raise SystemExit(proc.returncode)


if __name__=='__main__':
    main()
