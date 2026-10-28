import numpy as np
from scipy.linalg import eig
C=np.array([[0,0],[1,0],[0,1],[-1,0],[0,-1],[1,1],[-1,1],[-1,-1],[1,-1]])
W=np.array([4/9,1/9,1/9,1/9,1/9,1/36,1/36,1/36,1/36])
OPP=np.array([0,3,4,1,2,7,8,5,6])
M=np.array([[1]*9,[-4,-1,-1,-1,-1,2,2,2,2],[4,-2,-2,-2,-2,1,1,1,1],[0,1,0,-1,0,1,-1,-1,1],[0,-2,0,2,0,1,-1,-1,1],[0,0,1,0,-1,1,1,-1,-1],[0,0,-2,0,2,1,1,-1,-1],[0,1,-1,1,-1,0,0,0,0],[0,0,0,0,0,1,-1,1,-1]],float)
N=np.array([9,36,36,6,12,6,12,4,4]);assert np.max(abs(M@M.T-np.diag(N)))==0
nx=32

def ampl(U,tau,ky,outlet='velocity',save=False):
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
 ev,v=eig(A);k=np.argmax(abs(ev));
 if save:
  ny=32; mode=np.real(v[:,k].reshape(nx,9)[:,None,:]*np.exp(1j*ky*np.arange(ny))[None,:,None]).transpose(2,1,0)
  mode/=np.max(abs(mode)); mode.astype('float32').tofile('/tmp/lbm-independent-sol-20261028/eigen-mode.bin')
 power=np.linalg.norm(v[:,k].reshape(nx,9),axis=1);return ev[k],np.argmax(power),power[-1]/power[0]
for U,tau,ky,outlet in [(.022,.502857142857,np.pi,'velocity'),(.022,.502857142857,2.5,'velocity'),(0.,.502857142857, np.pi/2,'velocity'),(.022,.502857142857,np.pi/2,'velocity'),(.022,.502857142857,np.pi/2,'pressure'),(.022,.55,np.pi/2,'velocity'),(.022,.502857142857,0.,'velocity')]:
 ev,x,ratio=ampl(U,tau,ky,outlet);print(dict(U=U,tau=tau,ky=ky,outlet=outlet,eigenvalue=str(ev),growth=abs(ev),peak_x=x,outlet_inlet_amplitude_ratio=ratio))

ev,x,ratio=ampl(.022,.5+3*.05*((1/120)/7)/(.25**2),2*np.pi*13/32,save=True);print('GPU_MODE',ev,abs(ev),x,ratio)
