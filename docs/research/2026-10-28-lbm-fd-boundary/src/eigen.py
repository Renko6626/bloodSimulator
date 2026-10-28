import numpy as np
from scipy.linalg import eig
C=np.array([[0,0],[1,0],[0,1],[-1,0],[0,-1],[1,1],[-1,1],[-1,-1],[1,-1]])
W=np.array([4/9,1/9,1/9,1/9,1/9,1/36,1/36,1/36,1/36])
OPP=np.array([0,3,4,1,2,7,8,5,6])
M=np.array([[1]*9,[-4,-1,-1,-1,-1,2,2,2,2],[4,-2,-2,-2,-2,1,1,1,1],[0,1,0,-1,0,1,-1,-1,1],[0,-2,0,2,0,1,-1,-1,1],[0,0,1,0,-1,1,1,-1,-1],[0,0,-2,0,2,1,1,-1,-1],[0,1,-1,1,-1,0,0,0,0],[0,0,0,0,0,1,-1,1,-1]],float)
N=np.array([9,36,36,6,12,6,12,4,4]);assert np.max(abs(M@M.T-np.diag(N)))==0
nx=32

def operator(U,tau,ky,outlet='velocity'):
 u=np.array([U,0.]);cu=C@u
 # Differential of equilibrium w.r.t. density and velocity, then d(u)=(d(j)-u*d(rho))/rho.
 R=np.array([np.ones(9),C[:,0],C[:,1]])
 E=np.column_stack([W*(1+3*cu+4.5*cu**2-1.5*(u@u))-(W[:,None]*(3*C+9*cu[:,None]*C-3*u))@u, W[:,None]*(3*C+9*cu[:,None]*C-3*u)])
 L=np.eye(9)-M.T@np.diag(np.array([0,1.1,1,0,1.2,0,1.2,1/tau,1/tau])/N)@M@(np.eye(9)-E@R)
 A=np.zeros((nx*9,nx*9),complex)
 for x in range(nx):
  for q,(cx,cy) in enumerate(C):
   dest=x+cx
   if dest<0 or dest>=nx: A[x*9+OPP[q],x*9:x*9+9]+=L[q]
   else: A[dest*9+q,x*9:x*9+9]+=np.exp(-1j*ky*cy)*L[q]
 for x,n in [(0,np.array([1,0])),(nx-1,np.array([-1,0]))]:
  t=np.array([-n[1],n[0]]);cn=C@n;ct=C@t;un=u@n
  a=A[x*9+np.where(cn==0)[0]].sum(axis=0);b=A[x*9+np.where(cn==-1)[0]].sum(axis=0)
  drho=(a+2*b)/(1-un);kn=drho*un
  if x==nx-1 and outlet=='pressure':drho=drho*0;kn=-a-2*b
  kt=drho*(u@t);d=A[x*9+np.where((cn==0)&(ct==1))[0][0]]-A[x*9+np.where((cn==0)&(ct==-1))[0][0]]
  axial=np.where((cn==1)&(ct==0))[0][0];plus=np.where((cn==1)&(ct==1))[0][0];minus=np.where((cn==1)&(ct==-1))[0][0]
  A[x*9+axial]=A[x*9+OPP[axial]]+2*kn/3
  A[x*9+plus]=A[x*9+OPP[plus]]+kn/6+(kt-d)/2
  A[x*9+minus]=A[x*9+OPP[minus]]+kn/6-(kt-d)/2
  if outlet.startswith('fd'):
   # All interior rows are from the complete post-streaming operator.
   ix=x+int(n[0]); ixx=x+2*int(n[0])
   def du(x):
    return np.array([C[:,0]-U,C[:,1]]) @ A[x*9:x*9+9]
   dx=float(n[0])*(2*du(ix)-.5*du(ixx))
   if outlet=='fd_rho1': drho=A[ix*9:ix*9+9].sum(axis=0)
   div=dx[0]; dev=dx[0]
   pxx=-(div/1.1+dev*tau)/3
   pyy=-(div/1.1-dev*tau)/3
   pxy=-dx[1]*tau/3
   for q,(cx,cy) in enumerate(C):
    eq=W[q]*(1+3*cx*U+4.5*(cx*U)**2-1.5*U*U)
    A[x*9+q]=eq*drho+4.5*W[q]*((cx*cx-1/3)*pxx+(cy*cy-1/3)*pyy+2*cx*cy*pxy)
 return A

if __name__=='__main__':
 import json
 from pathlib import Path
 root=Path(__file__).resolve().parent.parent
 output=[]
 for name,tau in [('fine',.5+3*.05*((1/120)/7)/(.25**2)),('coarse',.5+3*.05*((1/120)/4)/(.5**2))]:
  for closure in ['velocity','fd','fd_rho1']:
   best=(0,None,None,None)
   for k in range(17):
    ky=2*np.pi*k/32; a=operator(.022,tau,ky,closure)
    if k==0:
     density=np.tile(W*(1+3*C[:,0]*.022+4.5*(C[:,0]*.022)**2-1.5*.022**2),nx)
     print('NEUTRAL_DENSITY_RESIDUAL',name,closure,float(np.max(abs(a@density-density))),flush=True)
    ev,v=eig(a);j=np.argmax(abs(ev));r=float(abs(ev[j]))
    row=dict(profile=name,closure=closure,k=k,ky=ky,radius=r,eigenvalue=[float(ev[j].real),float(ev[j].imag)])
    output.append(row);print(json.dumps(row),flush=True)
    if r>best[0]: best=(r,k,ev[j],v[:,j])
    if name=='fine' and closure=='velocity' and k==13:
     mode=np.real(v[:,j].reshape(nx,9)[:,None,:]*np.exp(1j*ky*np.arange(32))[None,:,None]).transpose(2,1,0);mode/=np.max(abs(mode));mode.astype('float32').tofile(root/'results/baseline-mode.bin')
   r,k,value,vector=best;ky=2*np.pi*k/32
   mode=np.real(vector.reshape(nx,9)[:,None,:]*np.exp(1j*ky*np.arange(32))[None,:,None]).transpose(2,1,0);mode/=np.max(abs(mode));mode.astype('float32').tofile(root/f'results/{name}-{closure}-worst.bin')
   print('WORST',name,closure,r,k,value,flush=True)
 (root/'results/spectrum.json').write_text(json.dumps(output,indent=2)+'\n')
