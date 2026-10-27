struct Params {
    n: u32, src: u32, dst: u32, basis: u32,
    count: u32, chunks: u32, mode: u32, add: u32,
    alpha: f32, aux: u32, pad0: u32, pad1: u32,
}
@group(0) @binding(0) var<storage, read> row_ptr: array<u32>;
@group(0) @binding(1) var<storage, read> columns: array<u32>;
@group(0) @binding(2) var<storage, read> values: array<f32>;
@group(0) @binding(3) var<storage, read_write> work: array<f32>;
@group(0) @binding(4) var<storage, read_write> partial: array<f32>;
@group(0) @binding(5) var<storage, read_write> coefficients: array<f32>;
@group(0) @binding(6) var<uniform> p: Params;
var<workgroup> sums: array<f32, 256>;

@compute @workgroup_size(256)
fn spmv(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n { return; }
    var sum = 0.0;
    for (var k = row_ptr[i]; k < row_ptr[i+1u]; k++) {
        sum += values[k] * work[p.src + columns[k]];
    }
    if p.mode == 1u { sum = work[p.aux + i] - sum; }
    work[p.dst + i] = sum;
}

// Each workgroup computes one chunk of one basis dot product (or a norm).
@compute @workgroup_size(256)
fn project(@builtin(workgroup_id) group: vec3<u32>,
           @builtin(local_invocation_index) lane: u32) {
    let i = group.x * 256u + lane;
    var value = 0.0;
    if i < p.n {
        let a = work[p.src + i];
        if p.mode == 1u { value = a*a; }
        else { value = a * work[p.basis + group.y * p.n + i]; }
    }
    sums[lane] = value;
    workgroupBarrier();
    for (var stride=128u; stride>0u; stride/=2u) {
        if lane < stride { sums[lane] += sums[lane+stride]; }
        workgroupBarrier();
    }
    if lane == 0u { partial[group.y*p.chunks+group.x] = sums[0]; }
}

@compute @workgroup_size(256)
fn reduce(@builtin(workgroup_id) group: vec3<u32>,
          @builtin(local_invocation_index) lane: u32) {
    var value=0.0;
    for(var i=lane; i<p.chunks; i+=256u) { value+=partial[group.x*p.chunks+i]; }
    sums[lane]=value;
    workgroupBarrier();
    for(var stride=128u; stride>0u; stride/=2u) {
        if lane<stride { sums[lane]+=sums[lane+stride]; }
        workgroupBarrier();
    }
    if lane==0u {
        if p.mode==1u { coefficients[p.aux]=sqrt(max(sums[0],0.0)); }
        else if p.add==1u {
            coefficients[p.aux+1u+group.x]=sums[0];
            coefficients[group.x]+=sums[0];
        }
        else { coefficients[group.x]=sums[0]; }
    }
}

@compute @workgroup_size(256)
fn vector(@builtin(global_invocation_id) id: vec3<u32>) {
    let i=id.x;
    if i>=p.n { return; }
    if p.mode==0u { work[p.dst+i]=p.alpha*work[p.src+i]; return; }
    if p.mode==3u {
        let length=coefficients[p.aux];
        var value=0.0;
        if length>0.0 { value=work[p.src+i]/length; }
        work[p.dst+i]=value;
        return;
    }
    var value=0.0;
    for(var j=0u; j<p.count; j++) {
        var coefficient=j;
        if p.mode==4u { coefficient=p.aux+1u+j; }
        value+=coefficients[coefficient]*work[p.basis+j*p.n+i];
    }
    if p.mode==1u || p.mode==4u { work[p.dst+i]-=value; }
    else { work[p.dst+i]+=value; }
}
