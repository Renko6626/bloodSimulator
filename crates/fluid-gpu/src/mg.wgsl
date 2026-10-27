struct Params {
 n:u32,src:u32,dst:u32,basis:u32,
 count:u32,chunks:u32,mode:u32,add:u32,
 alpha:f32,aux:u32,row_offset:u32,col:u32,
 val:u32,diag:u32,split:u32,j:u32,
 restart:u32,state:u32,tolerance:f32,gate:u32,
}
@group(0) @binding(0) var<storage,read> row_ptr:array<u32>;
@group(0) @binding(1) var<storage,read> columns:array<u32>;
@group(0) @binding(2) var<storage,read> values:array<f32>;
@group(0) @binding(3) var<storage,read_write> work:array<f32>;
@group(0) @binding(4) var<storage,read_write> partial:array<f32>;
@group(0) @binding(5) var<storage,read_write> scalar:array<f32>;
@group(0) @binding(6) var<uniform> p:Params;
var<workgroup> sums:array<f32,256>;
fn enabled()->bool {return p.gate==0u || scalar[p.state+4u]>0.5;}
fn hbase()->u32 {return 2u*(p.restart+2u);}
fn csbase()->u32 {return hbase()+(p.restart+1u)*p.restart;}
fn snbase()->u32 {return csbase()+p.restart;}
fn gbase()->u32 {return snbase()+p.restart;}
fn ybase()->u32 {return gbase()+p.restart+1u;}
fn normslot()->u32 {return p.restart+1u;}
fn stop_threshold()->f32 {return p.tolerance*max(scalar[p.state],1e-30);}

@compute @workgroup_size(256)
fn spmv(@builtin(global_invocation_id) id:vec3<u32>) {
 let i=id.x;if i>=p.n || !enabled(){return;}
 var sum=0.0;
 for(var k=row_ptr[p.row_offset+i];k<row_ptr[p.row_offset+i+1u];k++) {sum+=values[p.val+k]*work[p.src+columns[p.col+k]];}
 if p.mode==1u {sum=work[p.aux+i]-sum;}
 if p.mode==2u {work[p.dst+i]+=sum;} else {work[p.dst+i]=sum;}
}
@compute @workgroup_size(256)
fn project(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
 let i=group.x*256u+lane;var value=0.0;
 if i<p.n && enabled() {
  let a=work[p.src+i];
  if p.mode==1u {value=a*a;}else{value=a*work[p.basis+group.y*p.n+i];}
 }
 sums[lane]=value;workgroupBarrier();
 for(var stride=128u;stride>0u;stride/=2u){if lane<stride{sums[lane]+=sums[lane+stride];}workgroupBarrier();}
 if lane==0u {partial[group.y*p.chunks+group.x]=sums[0];}
}
@compute @workgroup_size(256)
fn reduce(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
 var value=0.0;for(var i=lane;i<p.chunks;i+=256u){value+=partial[group.x*p.chunks+i];}
 sums[lane]=value;workgroupBarrier();
 for(var stride=128u;stride>0u;stride/=2u){if lane<stride{sums[lane]+=sums[lane+stride];}workgroupBarrier();}
 if lane==0u && enabled() {
  if p.mode==1u {scalar[normslot()]=sqrt(max(sums[0],0.0));}
  else if p.add==1u {scalar[p.restart+2u+group.x]=sums[0];scalar[group.x]+=sums[0];}
  else{scalar[group.x]=sums[0];}
 }
}
@compute @workgroup_size(256)
fn vector(@builtin(global_invocation_id) id:vec3<u32>) {
 let i=id.x;if i>=p.n || !enabled(){return;}
 if p.mode==0u {work[p.dst+i]=p.alpha*work[p.src+i];return;}
 if p.mode==5u {work[p.dst+i]=0.0;return;}
 if p.mode==3u {
  let length=scalar[normslot()];var value=0.0;if length>0.0 {value=work[p.src+i]/length;}
  work[p.dst+i]=value;return;
 }
 var value=0.0;
 for(var j=0u;j<p.count;j++){value+=scalar[p.diag+j]*work[p.basis+j*p.n+i];}
 if p.mode==1u {work[p.dst+i]-=value;}else{work[p.dst+i]+=value;}
}
@compute @workgroup_size(256)
fn jacobi(@builtin(global_invocation_id) id:vec3<u32>) {
 let i=id.x;if i>=p.n || !enabled(){return;}
 var ax=0.0;for(var k=row_ptr[p.row_offset+i];k<row_ptr[p.row_offset+i+1u];k++){ax+=values[p.val+k]*work[p.src+columns[p.col+k]];}
 work[p.dst+i]=work[p.src+i]+p.alpha*values[p.diag+i]*(work[p.aux+i]-ax);
}
@compute @workgroup_size(256)
fn pressure(@builtin(global_invocation_id) id:vec3<u32>) {
 let i=id.x;if i>=p.n || !enabled(){return;}
 if i<p.split {work[p.dst+i]=0.0;}else{work[p.dst+i]=-values[p.diag+i-p.split]*work[p.src+i];}
}
@compute @workgroup_size(1)
fn cycle_init() {
 for(var i=hbase();i<p.state;i++){scalar[i]=0.0;}
 let beta=scalar[normslot()];scalar[gbase()]=beta;
 scalar[p.state+1u]=beta;scalar[p.state+2u]=beta;scalar[p.state+5u]=0.0;
 scalar[p.state+4u]=0.0;
 if beta>stop_threshold(){scalar[p.state+4u]=1.0;scalar[p.state+7u]+=1.0;}
}
@compute @workgroup_size(1)
fn qr() {
 if !enabled(){return;}
 let j=p.j;let base=hbase()+j*(p.restart+1u);let raw_next=scalar[normslot()];
 for(var i=0u;i<=j;i++){scalar[base+i]=scalar[i];}scalar[base+j+1u]=raw_next;
 for(var i=0u;i<j;i++){
  let a=scalar[base+i];let b=scalar[base+i+1u];let c=scalar[csbase()+i];let s=scalar[snbase()+i];
  scalar[base+i]=c*a+s*b;scalar[base+i+1u]=-s*a+c*b;
 }
 let a=scalar[base+j];let b=scalar[base+j+1u];let length=sqrt(a*a+b*b);
 if length<1e-30 {scalar[p.state+6u]=1.0;scalar[p.state+4u]=0.0;return;}
 let c=a/length;let s=b/length;scalar[csbase()+j]=c;scalar[snbase()+j]=s;
 scalar[base+j]=length;scalar[base+j+1u]=0.0;
 let old=scalar[gbase()+j];scalar[gbase()+j]=c*old;scalar[gbase()+j+1u]=-s*old;
 let estimated=abs(scalar[gbase()+j+1u]);scalar[p.state+2u]=estimated;
 scalar[p.state+3u]+=1.0;scalar[p.state+5u]=f32(j+1u);
 if estimated<=stop_threshold() || raw_next<1e-30 {scalar[p.state+4u]=0.0;}
}
@compute @workgroup_size(1)
fn backsolve() {
 for(var i=0u;i<p.restart;i++){scalar[ybase()+i]=0.0;}
 let used=u32(scalar[p.state+5u]);
 for(var remaining=used;remaining>0u;remaining--){
  let i=remaining-1u;var value=scalar[gbase()+i];
  for(var j=i+1u;j<used;j++){value-=scalar[hbase()+j*(p.restart+1u)+i]*scalar[ybase()+j];}
  scalar[ybase()+i]=value/scalar[hbase()+i*(p.restart+1u)+i];
 }
}
