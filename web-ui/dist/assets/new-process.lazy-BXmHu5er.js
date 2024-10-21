import{r,L as g,k as M,j as e,S as d,g as n,h as o,I as l,l as h,H as s,E as z,m as V,B as W,C as q,c as G,d as J,e as Q,f as U,i as X}from"./index-BXtM0M1h.js";const Y=()=>{const[j,N]=r.useState("hostname"),[p,P]=r.useState(80),[S,F]=r.useState(""),[C,B]=r.useState(""),[w,D]=r.useState(!0),[y,O]=r.useState(!1),[f,A]=r.useState(!1),[_,K]=r.useState(!1),[T,E]=r.useState(!1),[i,c]=r.useState([]),[k,L]=r.useState(g.Dotnet),[u,H]=r.useState([]),[x,v]=r.useState([]),{updateSite:R}=M(),I=()=>{p&&R.mutateAsync({siteSettings:{host_name:j,port:p,dir:S,bin:C,https:w,auto_start:y,capture_subdomains:f,disable_tcp_tunnel_mode:_,forward_subdomains:T,hints:i,log_format:k,env_vars:u,args:x}})};return e.jsxs(e.Fragment,{children:[e.jsxs(d,{marginTop:"0px",noTopSeparator:!0,children:[e.jsx(n,{title:"Hostname",subTitle:o.hostname,children:e.jsx(l,{placeholder:"my-site.com",value:j,onChange:t=>N(t.target.value)})}),e.jsx(n,{title:"Port",subTitle:o.port,children:e.jsx(l,{value:p,onChange:t=>{isNaN(Number(t.target.value))||P(Number(t.target.value))}})})]}),e.jsxs(d,{noTopSeparator:!0,noBottomSeparator:!0,children:[e.jsx(n,{title:"Directory",subTitle:o.directory,children:e.jsx(l,{placeholder:"/var/www/my-site",value:S,onChange:t=>F(t.target.value)})}),e.jsx(n,{title:"Bin",subTitle:o.binary,children:e.jsx(l,{placeholder:"my-binary",value:C,onChange:t=>B(t.target.value)})})]}),e.jsx(d,{noTopSeparator:!0,noBottomSeparator:!0,children:e.jsx(n,{labelFor:"use_https",rowOnly:!0,title:"HTTPS",subTitle:o.https,children:e.jsx(l,{checked:w,onChange:t=>{D(t.target.checked)},name:"use_https",id:"use_https",type:"checkbox",style:{width:"20px",height:"20px"}})})}),e.jsx(d,{noTopSeparator:!0,children:e.jsx(n,{labelFor:"auto_start",rowOnly:!0,title:"Auto start",subTitle:o.auto_start,children:e.jsx(l,{id:"auto_start",checked:y,name:"auto_start",onChange:t=>{O(t.target.checked)},type:"checkbox",style:{width:"20px",height:"20px"}})})}),e.jsxs(d,{noTopSeparator:!0,children:[e.jsx(n,{labelFor:"capture_subdomains",rowOnly:!0,title:"Capture sub-domains",subTitle:o.capture_subdomains,children:e.jsx(l,{onChange:t=>{A(t.target.checked)},checked:f,type:"checkbox",name:"capture_subdomains",id:"capture_subdomains",style:{width:"20px",height:"20px"}})}),e.jsx(n,{rowOnly:!0,labelFor:"disable_tcp_tunnel",title:"Disable TCP tunnel mode",subTitle:o.disable_tcp_tunnel,children:e.jsx(l,{type:"checkbox",checked:_,onChange:t=>{K(t.target.checked)},id:"disable_tcp_tunnel",name:"disable_tcp_tunnel",style:{width:"20px",height:"20px"}})}),e.jsx(n,{rowOnly:!0,labelFor:"forward_subdomains",title:"Forward sub-domains",subTitle:o.forward_subdomains,children:e.jsx(l,{type:"checkbox",checked:T,onChange:t=>{E(t.target.checked)},id:"forward_subdomains",name:"forward_subdomains",style:{width:"20px",height:"20px"}})})]}),e.jsx(d,{noTopSeparator:!0,children:e.jsx(n,{title:"Log format",subTitle:o.log_format,children:e.jsxs("select",{className:"text-black rounded pl-3 pr-3",value:k,onChange:t=>{L(t.target.value)},name:"log_format",style:{height:"32px",width:"100%"},children:[e.jsx("option",{value:g.Standard,children:"Standard"}),e.jsx("option",{value:g.Dotnet,children:"Dotnet"})]})})}),e.jsx("div",{style:{marginTop:"20px"}}),e.jsx(n,{title:"Hints",subTitle:o.h2_hint}),e.jsxs("div",{style:{display:"flex",gap:"10px",flexWrap:"wrap",justifyContent:"start",marginTop:"10px",marginBottom:"20px"},children:[e.jsx(h,{onClick:()=>{i.includes(s.H2)?c(t=>[...t.filter(a=>a!==s.H2)]):c(t=>[...t,s.H2])},checked:i.includes(s.H2),title:"H2"}),e.jsx(h,{onClick:()=>{i.includes(s.H2C)?c(t=>[...t.filter(a=>a!==s.H2C)]):c(t=>[...t,s.H2C])},checked:i.includes(s.H2C),title:"H2C"}),e.jsx(h,{onClick:()=>{i.includes(s.H2CPK)?c(t=>[...t.filter(a=>a!==s.H2CPK)]):c(t=>[...t,s.H2CPK])},checked:i.includes(s.H2CPK),title:"H2CPK"}),e.jsx(h,{onClick:()=>{i.includes(s.NOH2)?c(t=>[...t.filter(a=>a!==s.NOH2)]):c(t=>[...t,s.NOH2])},checked:i.includes(s.NOH2),title:"NOH2"})]}),e.jsx(d,{noBottomSeparator:!0,children:e.jsx(n,{vertical:!0,title:"Environment variables",subTitle:o.env_vars,children:e.jsx(z,{keys:u,onRemoveKey:t=>{H(u==null?void 0:u.filter(a=>a.key!==t))},onNewKey:(t,a)=>{H(m=>[...m.filter(b=>b.key!==a),t])}})})}),e.jsx(d,{noBottomSeparator:!0,noTopSeparator:!0,children:e.jsx(n,{vertical:!0,title:"Arguments",subTitle:o.args,children:e.jsx(V,{onAddArg:(t,a)=>{v(m=>[...m.filter(b=>b!==a),t])},onRemoveArg:t=>{v(x.filter(a=>a!==t))},defaultKeys:x})})}),e.jsx("div",{style:{display:"flex",alignItems:"center",justifyContent:"end",marginTop:"20px"},children:e.jsx(W,{onClick:I,style:{width:"max-content",background:"var(--color7)"},children:"Create site"})})]})},Z=()=>e.jsx("main",{className:"grid flex-1 items-start gap-4 md:pb-8 md:gap-8 max-w-[900px]",children:e.jsxs(q,{children:[e.jsxs(G,{children:[e.jsx(J,{children:"New hosted process"}),e.jsxs(Q,{children:["Creating a process that odd-box will manage.",e.jsx("br",{}),"This is a service that odd-box can start, stop, and restart."]})]}),e.jsx(U,{children:e.jsx(Y,{})})]})}),ee=X("/new-process")({component:Z});export{ee as Route};
