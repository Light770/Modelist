import{u as s,a as je,k as F,j as n,P as Ve,R as ze,G as Oe,b as We,c as qe,d as Ge,g as He}from"./StudioRoot.BytRkG0c.js";import{r as p}from"./index.CpbjX5yF.js";const Ke={plain:0,twill:1,satin:2,jersey:3,rib:4,sheer:5,terry:6},Xe=`#version 300 es
precision highp float;
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_norm;
layout(location = 2) in float a_metric;
layout(location = 3) in vec3 a_color;

uniform mat4 u_mvp;
out vec3 v_norm;
out vec3 v_pos;
out float v_metric;
out vec3 v_color;

void main() {
  v_norm = a_norm;
  v_pos = a_pos;
  v_metric = a_metric;
  v_color = a_color;
  gl_Position = u_mvp * vec4(a_pos, 1.0);
}
`,Qe=`#version 300 es
precision highp float;
in vec3 v_norm;
in vec3 v_pos;
in float v_metric;
in vec3 v_color;

uniform int u_mode; // 0=avatar, 1=cloth_material, 2=strain, 3=tension, 4=clearance, 5=xray, 6=floor
uniform int u_finish; // 0 plain, 1 twill, 2 satin, 3 jersey, 4 rib, 5 sheer, 6 terry
uniform vec3 u_baseColor;
uniform vec3 u_eyePos;
uniform float u_isDark;
uniform vec4 u_tapeY; // (bust_y, waist_y, hip_y, crotch_y)
uniform float u_neckY;
uniform float u_showTape;
out vec4 outColor;

vec3 heatRamp(float t) {
  t = clamp(t, 0.0, 1.0);
  if (t < 0.35) {
    return mix(vec3(0.10, 0.48, 0.90), vec3(0.08, 0.74, 0.50), t / 0.35);
  } else if (t < 0.70) {
    return mix(vec3(0.08, 0.74, 0.50), vec3(0.95, 0.68, 0.12), (t - 0.35) / 0.35);
  } else {
    return mix(vec3(0.95, 0.68, 0.12), vec3(0.88, 0.16, 0.18), (t - 0.70) / 0.30);
  }
}

void main() {
  // Studio Floor Plane with Architectural 20cm Grid + Soft Radial Contact Shadow
  if (u_mode == 6) {
    float r = length(v_pos.xz);
    float fade = 1.0 - smoothstep(0.65, 2.2, r);
    if (fade < 0.01) discard;

    vec2 g = abs(fract(v_pos.xz * 5.0 - 0.5) - 0.5) / max(fwidth(v_pos.xz * 5.0), vec2(0.001));
    float line = 1.0 - min(min(g.x, g.y), 1.0);
    vec2 gmaj = abs(fract(v_pos.xz * 1.0 - 0.5) - 0.5) / max(fwidth(v_pos.xz * 1.0), vec2(0.001));
    float majLine = 1.0 - min(min(gmaj.x, gmaj.y), 1.0);

    // Soft studio ambient-occlusion contact shadow under the mannequin feet
    float shadow = 0.24 * exp(-r * r * 14.0);

    vec3 floorBase = mix(vec3(0.92, 0.91, 0.89), vec3(0.12, 0.13, 0.16), u_isDark);
    vec3 gridCol = mix(vec3(0.76, 0.75, 0.72), vec3(0.24, 0.26, 0.32), u_isDark);
    vec3 majCol = mix(vec3(0.62, 0.61, 0.59), vec3(0.32, 0.35, 0.42), u_isDark);
    vec3 col = mix(floorBase, gridCol, line * 0.45);
    col = mix(col, majCol, majLine * 0.65);
    col *= (1.0 - shadow);
    outColor = vec4(col, fade * 0.92);
    return;
  }

  vec3 n = normalize(v_norm);
  bool backFace = !gl_FrontFacing;
  if (backFace) n = -n;
  vec3 vDir = normalize(u_eyePos - v_pos);

  // Studio 3-point lighting (warm key + cool fill + rim backlight)
  vec3 lKey = normalize(vec3(0.55, 0.82, 0.72));
  vec3 lFill = normalize(vec3(-0.75, 0.35, 0.45));
  vec3 lRim = normalize(vec3(-0.25, 0.55, -0.85));

  float key = max(dot(n, lKey), 0.0);
  float fill = max(dot(n, lFill), 0.0);
  float rim = max(dot(n, lRim), 0.0);
  float fresnel = pow(1.0 - max(dot(n, vDir), 0.0), 3.0);
  float hemi = 0.30 + 0.16 * n.y;

  if (u_mode == 0) {
    // Sculpted atelier porcelain-clay dress-form mannequin + calibrated tailor tape lines
    float wrap = max((dot(n, lKey) + 0.30) / 1.30, 0.0);
    float spec = pow(max(dot(reflect(-lKey, n), vDir), 0.0), 22.0) * 0.10;
    float ao = 0.88 + 0.12 * clamp(0.5 + 0.5 * n.y, 0.0, 1.0);
    float light = clamp(hemi + wrap * 0.56 + fill * 0.22 + rim * 0.14, 0.26, 1.10) * ao;
    vec3 col = u_baseColor * light + vec3(spec) + vec3(0.055) * fresnel;

    if (u_showTape > 0.5 && v_metric > 0.55 && v_pos.y > u_tapeY.w + 0.015 && v_pos.y < u_neckY - 0.015) {
      float dyB = abs(v_pos.y - u_tapeY.x) / max(fwidth(v_pos.y), 0.0005);
      float dyW = abs(v_pos.y - u_tapeY.y) / max(fwidth(v_pos.y), 0.0005);
      float dyH = abs(v_pos.y - u_tapeY.z) / max(fwidth(v_pos.y), 0.0005);
      float dxC = abs(v_pos.x) / max(fwidth(v_pos.x), 0.0005);
      float hLine = 1.0 - smoothstep(0.9, 2.2, min(min(dyB, dyW), dyH));
      float vLine = 1.0 - smoothstep(0.8, 1.9, dxC);
      float tape = max(hLine * 0.55, vLine * 0.34);
      vec3 tapeCol = mix(vec3(0.24, 0.25, 0.28), vec3(0.75, 0.78, 0.84), u_isDark);
      col = mix(col, tapeCol, tape);
    }

    outColor = vec4(col, 1.0);
    return;
  }

  if (u_mode == 1) {
    // Per-piece fabric colour with a procedural surface finish (look only, no physics).
    vec3 base = v_color;
    float tex = 1.0;
    float sheenAmt = 0.18;
    float specPow = 0.0;
    float alpha = 1.0;
    if (u_finish == 1) {            // twill: diagonal wales
      tex = 0.94 + 0.06 * sin((v_pos.x + v_pos.y) * 900.0);
    } else if (u_finish == 2) {     // satin: smooth with a strong sheen
      tex = 1.0; sheenAmt = 0.45; specPow = 28.0;
    } else if (u_finish == 3) {     // jersey: fine vertical knit
      tex = 0.95 + 0.05 * abs(sin(v_pos.x * 1400.0 + v_pos.z * 1400.0));
    } else if (u_finish == 4) {     // rib: pronounced vertical ribs
      tex = 0.86 + 0.14 * smoothstep(-0.2, 0.9, sin(v_pos.x * 420.0 + v_pos.z * 420.0));
    } else if (u_finish == 5) {     // sheer: light and translucent
      base = mix(base, vec3(1.0), 0.18); alpha = 0.72; sheenAmt = 0.3;
    } else if (u_finish == 6) {     // terry: matte, slightly mottled
      tex = 0.93 + 0.07 * fract(sin(dot(floor(v_pos * 700.0), vec3(12.9898, 78.233, 37.719))) * 43758.5453);
      sheenAmt = 0.06;
    } else {                        // plain weave
      tex = 0.97 + 0.03 * sin(v_pos.x * 260.0 + v_pos.y * 180.0) * cos(v_pos.y * 260.0 - v_pos.z * 180.0);
    }
    vec3 fabricCol = base * tex;
    if (backFace) {
      fabricCol *= vec3(0.74, 0.72, 0.70); // darker inside, like CLO3D
    }
    float wrapDiff = max((dot(n, lKey) + 0.25) / 1.25, 0.0);
    float light = clamp(hemi * 0.92 + wrapDiff * 0.62 + fill * 0.24 + rim * 0.16, 0.22, 1.12);
    vec3 sheen = mix(base, vec3(1.0), 0.35) * fresnel * sheenAmt;
    float spec = specPow > 0.0 ? pow(max(dot(reflect(-lKey, n), vDir), 0.0), specPow) * 0.35 : 0.0;
    outColor = vec4(fabricCol * light + sheen + vec3(spec), alpha);
    return;
  }

  float shade = clamp(hemi + key * 0.55 + fill * 0.20, 0.35, 1.08);
  if (u_mode == 2) {
    vec3 col = heatRamp(v_metric / 12.0) * (0.42 + 0.65 * shade);
    outColor = vec4(col, 1.0);
    return;
  }
  if (u_mode == 3) {
    vec3 col = heatRamp(v_metric / 160.0) * (0.42 + 0.65 * shade);
    outColor = vec4(col, 1.0);
    return;
  }
  if (u_mode == 4) {
    // Ease / Clearance (mm): <4mm red compression, 15-35mm green ideal, >65mm blue loose
    float t = 1.0 - clamp((v_metric - 2.0) / 68.0, 0.0, 1.0);
    vec3 col = heatRamp(t) * (0.42 + 0.65 * shade);
    outColor = vec4(col, 1.0);
    return;
  }
  // X-Ray translucent fit check
  outColor = vec4(vec3(0.22, 0.58, 0.96) * shade + vec3(0.15) * fresnel, 0.42);
}
`;function Je(a){const i=a.replace("#","");if(i.length!==6)return[.22,.36,.55];const d=parseInt(i.slice(0,2),16)/255,o=parseInt(i.slice(2,4),16)/255,r=parseInt(i.slice(4,6),16)/255;return[Number.isFinite(d)?d:.22,Number.isFinite(o)?o:.36,Number.isFinite(r)?r:.55]}function me(a,i,d){const o=a.createShader(i);if(a.shaderSource(o,d),a.compileShader(o),!a.getShaderParameter(o,a.COMPILE_STATUS))throw new Error(a.getShaderInfoLog(o)||"Shader compile failed");return o}function Ze(a,i,d,o){const r=1/Math.tan(a/2),x=1/(d-o);return new Float32Array([r/i,0,0,0,0,r,0,0,0,0,(o+d)*x,-1,0,0,2*o*d*x,0])}function $e(a,i){const d=a[0]-i[0],o=a[1]-i[1],r=a[2]-i[2],x=Math.hypot(d,o,r)||1,f=[d/x,o/x,r/x],B=f[2],E=0,v=-f[0],S=Math.hypot(B,E,v)||1,h=[B/S,E/S,v/S],_=[f[1]*h[2]-f[2]*h[1],f[2]*h[0]-f[0]*h[2],f[0]*h[1]-f[1]*h[0]];return new Float32Array([h[0],_[0],f[0],0,h[1],_[1],f[1],0,h[2],_[2],f[2],0,-(h[0]*a[0]+h[1]*a[1]+h[2]*a[2]),-(_[0]*a[0]+_[1]*a[1]+_[2]*a[2]),-(f[0]*a[0]+f[1]*a[1]+f[2]*a[2]),1])}function et(a,i){const d=new Float32Array(16);for(let o=0;o<4;o++)for(let r=0;r<4;r++)d[o*4+r]=a[0+r]*i[o*4+0]+a[4+r]*i[o*4+1]+a[8+r]*i[o*4+2]+a[12+r]*i[o*4+3];return d}function G(a){return a==="trouser_block"?{targetY:.62,dist:1.92}:a==="aline_skirt"?{targetY:.8,dist:1.78}:{targetY:1.18,dist:1.68}}const at=({style:a})=>{const i=p.useRef(null),d=s(t=>t.snapshot),o=G(d?.template_id),r=p.useRef({yaw:.36,pitch:.08,dist:o.dist,targetY:o.targetY,dragging:!1,lx:0,ly:0}),x=p.useRef(!0),f=p.useRef(-1),B=s(t=>t.sim3dPose),E=s(t=>t.sim3dBodyForm),v=s(t=>t.sim3dViewMode),S=s(t=>t.show3dAvatar),h=s(t=>t.show3dFloor),_=s(t=>t.show3dTapeLines),T=s(t=>t.sim3dCameraPreset),ve=s(t=>t.sim3dCameraSeq),he=s(t=>t.toggle3dAvatar),pe=s(t=>t.toggle3dFloor),xe=s(t=>t.toggle3dTapeLines),_e=s(t=>t.request3dCamera),g=s(t=>t.sim3dPlaying),be=s(t=>t.setSim3dPlaying),Ae=s(t=>t.requestSim3dReset),ge=s(t=>t.sim3dResetSeq),ye=s(t=>t.setSim3dTelemetry),D=p.useRef(null),[I,H]=p.useState(null),c=je(),K=t=>{D.current=t,ye(t)};p.useEffect(()=>{x.current=g},[g]),p.useEffect(()=>(K(F.initSim3d(B,E)),()=>{F.disposeSim3d(),s.getState().setSim3dTelemetry(null,null)}),[]);const X=p.useRef(!1);p.useEffect(()=>{if(!X.current){X.current=!0;return}K(F.resetSim3d(B,E)),f.current=-1},[B,E,ge]),p.useEffect(()=>{const t=G(d?.template_id);r.current.targetY=t.targetY,r.current.dist=t.dist},[d?.template_id]),p.useEffect(()=>{const t=G(s.getState().snapshot?.template_id),e=r.current;e.targetY=t.targetY,e.dist=t.dist,T==="front"?(e.yaw=0,e.pitch=.05):T==="back"?(e.yaw=Math.PI,e.pitch=.06):T==="left"?(e.yaw=Math.PI*.5,e.pitch=.05):T==="right"?(e.yaw=-Math.PI*.5,e.pitch=.05):(e.yaw=.36,e.pitch=.08)},[T,ve]),p.useEffect(()=>{const t=i.current;if(!t)return;const e=t.getContext("webgl2",{antialias:!0,alpha:!1});if(!e){H("webgl2");return}let l;try{const b=me(e,e.VERTEX_SHADER,Xe),m=me(e,e.FRAGMENT_SHADER,Qe);if(l=e.createProgram(),e.attachShader(l,b),e.attachShader(l,m),e.linkProgram(l),!e.getProgramParameter(l,e.LINK_STATUS))throw new Error(e.getProgramInfoLog(l)||"Program link failed")}catch(b){H(b.message);return}const we=e.getUniformLocation(l,"u_mvp"),M=e.getUniformLocation(l,"u_mode"),Q=e.getUniformLocation(l,"u_baseColor"),Fe=e.getUniformLocation(l,"u_eyePos"),Be=e.getUniformLocation(l,"u_isDark"),Ee=e.getUniformLocation(l,"u_tapeY"),Se=e.getUniformLocation(l,"u_neckY"),Te=e.getUniformLocation(l,"u_showTape"),Ce=e.getUniformLocation(l,"u_finish"),U=e.createVertexArray(),J=e.createBuffer(),Z=e.createBuffer(),$=e.createBuffer(),y=2.4,Ne=new Float32Array([-y,0,-y,y,0,-y,y,0,y,-y,0,y]),Pe=new Float32Array([0,1,0,0,1,0,0,1,0,0,1,0]),De=new Uint32Array([0,2,1,0,3,2]);e.bindVertexArray(U),e.bindBuffer(e.ARRAY_BUFFER,J),e.bufferData(e.ARRAY_BUFFER,Ne,e.STATIC_DRAW),e.enableVertexAttribArray(0),e.vertexAttribPointer(0,3,e.FLOAT,!1,0,0),e.bindBuffer(e.ARRAY_BUFFER,Z),e.bufferData(e.ARRAY_BUFFER,Pe,e.STATIC_DRAW),e.enableVertexAttribArray(1),e.vertexAttribPointer(1,3,e.FLOAT,!1,0,0),e.disableVertexAttribArray(2),e.vertexAttrib1f(2,0),e.bindBuffer(e.ELEMENT_ARRAY_BUFFER,$),e.bufferData(e.ELEMENT_ARRAY_BUFFER,De,e.STATIC_DRAW);const j=e.createVertexArray(),ee=e.createBuffer(),te=e.createBuffer(),re=e.createBuffer(),ae=e.createBuffer(),V=e.createVertexArray(),oe=e.createBuffer(),ne=e.createBuffer(),se=e.createBuffer(),ie=e.createBuffer(),ce=e.createBuffer();let z=0,O=0,W=0,q=0;const le=b=>{const m=F.getSim3dBuffers();O=m.clothIdx.length,b&&(z=m.avatarIdx.length,e.bindVertexArray(j),e.bindBuffer(e.ARRAY_BUFFER,ee),e.bufferData(e.ARRAY_BUFFER,m.avatarPos,e.STATIC_DRAW),e.enableVertexAttribArray(0),e.vertexAttribPointer(0,3,e.FLOAT,!1,0,0),e.bindBuffer(e.ARRAY_BUFFER,te),e.bufferData(e.ARRAY_BUFFER,m.avatarNorm,e.STATIC_DRAW),e.enableVertexAttribArray(1),e.vertexAttribPointer(1,3,e.FLOAT,!1,0,0),e.bindBuffer(e.ARRAY_BUFFER,re),e.bufferData(e.ARRAY_BUFFER,m.avatarMask,e.STATIC_DRAW),e.enableVertexAttribArray(2),e.vertexAttribPointer(2,1,e.FLOAT,!1,0,0),e.bindBuffer(e.ELEMENT_ARRAY_BUFFER,ae),e.bufferData(e.ELEMENT_ARRAY_BUFFER,m.avatarIdx,e.STATIC_DRAW));const N=s.getState().sim3dViewMode,C=N==="strain"?m.clothStrain:N==="tension"?m.clothTension:m.clothClearance;e.bindVertexArray(V),e.bindBuffer(e.ARRAY_BUFFER,oe),e.bufferData(e.ARRAY_BUFFER,m.clothPos,e.DYNAMIC_DRAW),e.enableVertexAttribArray(0),e.vertexAttribPointer(0,3,e.FLOAT,!1,0,0),e.bindBuffer(e.ARRAY_BUFFER,ne),e.bufferData(e.ARRAY_BUFFER,m.clothNorm,e.DYNAMIC_DRAW),e.enableVertexAttribArray(1),e.vertexAttribPointer(1,3,e.FLOAT,!1,0,0),e.bindBuffer(e.ARRAY_BUFFER,se),e.bufferData(e.ARRAY_BUFFER,C,e.DYNAMIC_DRAW),e.enableVertexAttribArray(2),e.vertexAttribPointer(2,1,e.FLOAT,!1,0,0),b&&(e.bindBuffer(e.ARRAY_BUFFER,ie),e.bufferData(e.ARRAY_BUFFER,m.clothColor,e.STATIC_DRAW),e.enableVertexAttribArray(3),e.vertexAttribPointer(3,3,e.FLOAT,!1,0,0),e.bindBuffer(e.ELEMENT_ARRAY_BUFFER,ce),e.bufferData(e.ELEMENT_ARRAY_BUFFER,m.clothIdx,e.STATIC_DRAW)),e.bindVertexArray(null)},fe=()=>{W=requestAnimationFrame(fe),q+=1;const b=He();if(b!==f.current){f.current=b;const u=F.stepSim3d(1);le(!0),u&&(D.current=u,s.getState().setSim3dTelemetry(u))}else if(x.current){const u=F.stepSim3d(1);if(le(!1),u&&(D.current=u),u&&q%12===0){const P=q%96===0?F.getSim3dQuality():void 0;s.getState().setSim3dTelemetry(u,P)}}const m=t.getBoundingClientRect(),N=window.devicePixelRatio||1,C=Math.max(1,Math.floor(m.width*N)),Y=Math.max(1,Math.floor(m.height*N));(t.width!==C||t.height!==Y)&&(t.width=C,t.height=Y);const R=s.getState(),w=R.theme==="dark";e.viewport(0,0,C,Y),e.clearColor(w?.1:.94,w?.11:.935,w?.13:.92,1),e.clear(e.COLOR_BUFFER_BIT|e.DEPTH_BUFFER_BIT),e.enable(e.DEPTH_TEST);const A=r.current,ue=Math.cos(A.pitch),L=[A.dist*ue*Math.sin(A.yaw),A.targetY+A.dist*Math.sin(A.pitch),A.dist*ue*Math.cos(A.yaw)],ke=Ze(.62,C/Y,.05,18),Ye=$e(L,[0,A.targetY,0]),Le=et(ke,Ye);if(e.useProgram(l),e.uniformMatrix4fv(we,!1,Le),e.uniform3f(Fe,L[0],L[1],L[2]),e.uniform1f(Be,w?1:0),R.show3dFloor&&(e.enable(e.BLEND),e.blendFunc(e.SRC_ALPHA,e.ONE_MINUS_SRC_ALPHA),e.uniform1i(M,6),e.bindVertexArray(U),e.drawElements(e.TRIANGLES,6,e.UNSIGNED_INT,0),e.disable(e.BLEND)),R.show3dAvatar&&z>0){const u=D.current;e.disable(e.BLEND),e.uniform1i(M,0),e.uniform3f(Q,w?.44:.66,w?.44:.67,w?.47:.7),e.uniform4f(Ee,u?.bust_y??1.33,u?.waist_y??1.12,u?.hip_y??.92,u?.crotch_y??.85),e.uniform1f(Se,u?.neck_y??1.6),e.uniform1f(Te,R.show3dTapeLines?1:0),e.bindVertexArray(j),e.drawElements(e.TRIANGLES,z,e.UNSIGNED_INT,0)}if(O>0){const u=R.sim3dViewMode,P=u==="material"?1:u==="strain"?2:u==="tension"?3:u==="clearance"?4:5,de=R.snapshot?.fabric.finish??"plain";(P===5||P===1&&de==="sheer")&&(e.enable(e.BLEND),e.blendFunc(e.SRC_ALPHA,e.ONE_MINUS_SRC_ALPHA),e.depthMask(!1));const[Ie,Me,Ue]=Je(R.snapshot?.fabric.color_hex||"#2d4c73");e.uniform1i(M,P),e.uniform1i(Ce,Ke[de]??0),e.uniform3f(Q,Ie,Me,Ue),e.bindVertexArray(V),e.drawElements(e.TRIANGLES,O,e.UNSIGNED_INT,0),e.depthMask(!0),e.disable(e.BLEND)}e.bindVertexArray(null)};return W=requestAnimationFrame(fe),()=>{cancelAnimationFrame(W),e.deleteBuffer(J),e.deleteBuffer(Z),e.deleteBuffer($),e.deleteVertexArray(U),e.deleteBuffer(ee),e.deleteBuffer(te),e.deleteBuffer(re),e.deleteBuffer(ae),e.deleteBuffer(oe),e.deleteBuffer(ne),e.deleteBuffer(se),e.deleteBuffer(ie),e.deleteBuffer(ce),e.deleteVertexArray(j),e.deleteVertexArray(V),e.deleteProgram(l)}},[]),p.useEffect(()=>{f.current=-1},[v]);const Re=v==="strain"||v==="tension"||v==="clearance",k=t=>({background:t?"var(--accent-soft)":void 0,color:t?"var(--accent)":"var(--text-secondary)"});return n.jsxs("div",{style:{...a,background:"var(--bg-canvas)"},className:"relative h-full min-w-[260px] overflow-hidden select-none",children:[I?n.jsx("div",{className:"p-4 text-xs text-rose-500 font-mono-num",children:I==="webgl2"?c("v3d.webglMissing"):I}):n.jsx("canvas",{ref:i,onPointerDown:t=>{t.currentTarget.setPointerCapture(t.pointerId),r.current.dragging=!0,r.current.lx=t.clientX,r.current.ly=t.clientY},onPointerMove:t=>{if(!r.current.dragging)return;const e=t.clientX-r.current.lx,l=t.clientY-r.current.ly;r.current.lx=t.clientX,r.current.ly=t.clientY,t.shiftKey||t.buttons===4?r.current.targetY=Math.max(.25,Math.min(1.55,r.current.targetY+l*.0028)):(r.current.yaw-=e*.0085,r.current.pitch=Math.max(-.75,Math.min(1.05,r.current.pitch+l*.0075)))},onPointerUp:()=>{r.current.dragging=!1},onWheel:t=>{t.preventDefault(),r.current.dist=Math.max(.72,Math.min(3.8,r.current.dist*(t.deltaY>0?1.08:.92)))},className:"w-full h-full block cursor-grab active:cursor-grabbing touch-none"}),Re&&n.jsxs("div",{className:"floating-panel absolute bottom-2 left-2 z-10 px-2.5 py-1.5 text-[10px] font-mono-num",children:[n.jsx("div",{className:"mb-1 font-sans font-semibold text-[10px]",children:c(v==="strain"?"v3d.legendStrain":v==="tension"?"v3d.legendTension":"v3d.legendEase")}),n.jsx("div",{className:"w-28 h-2 rounded-[2px]",style:{background:v==="clearance"?"linear-gradient(90deg, #e0292e, #f2ad1f, #14bd80, #1a7ae6)":"linear-gradient(90deg, #1a7ae6, #14bd80, #f2ad1f, #e0292e)"}}),n.jsxs("div",{className:"flex justify-between mt-0.5 text-[9px]",style:{color:"var(--text-muted)"},children:[n.jsx("span",{children:v==="clearance"?c("v3d.tight"):"0"}),n.jsx("span",{children:v==="clearance"?c("v3d.ideal"):v==="strain"?"6%":"80"}),n.jsx("span",{children:v==="clearance"?c("v3d.loose"):v==="strain"?"12%+":"160+"})]})]}),n.jsxs("div",{className:"floating-panel absolute bottom-2 left-1/2 -translate-x-1/2 z-10 h-[30px] px-1 flex items-center gap-0.5 text-[11px] whitespace-nowrap",role:"toolbar","aria-label":c("nav.title3d"),children:[[["front","v3d.front"],["three_quarter","v3d.threeQuarter"],["left","v3d.side"],["back","v3d.back"]].map(([t,e])=>n.jsx("button",{onClick:()=>_e(t),className:"h-[24px] px-1.5 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]",style:k(T===t),title:c("nav.camera",{view:c(e)}),children:c(e)},t)),n.jsx("div",{className:"w-px h-4 mx-1",style:{background:"var(--border-subtle)"}}),n.jsx("button",{onClick:he,"aria-pressed":S,className:"w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]",style:k(S),title:c("v3d.avatarTitle"),children:n.jsx(Ve,{className:"w-3.5 h-3.5"})}),n.jsx("button",{onClick:xe,"aria-pressed":_,className:"w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]",style:k(_),title:c("v3d.tapeTitle"),children:n.jsx(ze,{className:"w-3.5 h-3.5"})}),n.jsx("button",{onClick:pe,"aria-pressed":h,className:"w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]",style:k(h),title:c("v3d.floorTitle"),children:n.jsx(Oe,{className:"w-3.5 h-3.5"})}),n.jsx("div",{className:"w-px h-4 mx-1",style:{background:"var(--border-subtle)"}}),n.jsxs("button",{onClick:()=>be(!g),"aria-pressed":g,className:"h-[24px] px-1.5 flex items-center gap-1 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]",style:{color:g?"#0e9f6e":"var(--text-secondary)"},title:c(g?"v3d.pause":"v3d.play"),children:[g?n.jsx(We,{className:"w-3.5 h-3.5"}):n.jsx(qe,{className:"w-3.5 h-3.5"}),c(g?"nav.pause":"nav.simulate")]}),n.jsx("button",{onClick:Ae,className:"w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]",style:{color:"var(--text-secondary)"},title:c("v3d.reset"),"aria-label":c("v3d.reset"),children:n.jsx(Ge,{className:"w-3.5 h-3.5"})})]})]})};export{at as Viewport3D,at as default};
