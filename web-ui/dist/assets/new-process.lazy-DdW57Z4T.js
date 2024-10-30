import{b as V,r as a,L as x,m as M,j as e,S as c,h as n,i as r,I as i,n as u,H as s,B as R,k as z,C as W,c as q,d as G,f as J,g as Q,l as U}from"./index-Cox9BY69.js";const X=()=>{const{data:m}=V(),[g,H]=a.useState("hostname"),[h,k]=a.useState(""),[b,N]=a.useState(""),[j,P]=a.useState(""),[S,F]=a.useState(!0),[C,B]=a.useState(!1),[w,D]=a.useState(!1),[_,O]=a.useState(!1),[v,A]=a.useState(!1),[o,l]=a.useState([]),[T,L]=a.useState(x.Dotnet),[f,K]=a.useState(""),[y,E]=a.useState(""),{updateSite:p}=M(),I=()=>{p.mutateAsync({siteSettings:{host_name:g,port:h===""?void 0:Number(h),dir:b,bin:j,https:S,auto_start:C,capture_subdomains:w,disable_tcp_tunnel_mode:_,forward_subdomains:v,hints:o,log_format:T,env_vars:z(f),args:y.split(";")}})};return e.jsxs(e.Fragment,{children:[e.jsxs(c,{marginTop:"0px",noTopSeparator:!0,children:[e.jsx(n,{title:"Hostname",subTitle:r.hostname,children:e.jsx(i,{placeholder:"my-site.com",value:g,onChange:t=>H(t.target.value)})}),e.jsx(n,{title:"Port",defaultValue:m.http_port,subTitle:r.port,children:e.jsx(i,{value:h,placeholder:m.http_port.toString(),onChange:t=>{isNaN(Number(t.target.value))||k(t.target.value)}})})]}),e.jsxs(c,{noTopSeparator:!0,noBottomSeparator:!0,children:[e.jsx(n,{title:"Directory",subTitle:r.directory,children:e.jsx(i,{placeholder:"/var/www/my-site",value:b,onChange:t=>N(t.target.value)})}),e.jsx(n,{title:"Bin",subTitle:r.binary,children:e.jsx(i,{placeholder:"my-binary",value:j,onChange:t=>P(t.target.value)})})]}),e.jsx(c,{noTopSeparator:!0,noBottomSeparator:!0,children:e.jsx(n,{labelFor:"use_https",rowOnly:!0,title:"HTTPS",subTitle:r.https,children:e.jsx(i,{checked:S,onChange:t=>{F(t.target.checked)},name:"use_https",id:"use_https",type:"checkbox",style:{width:"20px",height:"20px"}})})}),e.jsx(c,{noTopSeparator:!0,children:e.jsx(n,{labelFor:"auto_start",rowOnly:!0,title:"Auto start",subTitle:r.auto_start,children:e.jsx(i,{id:"auto_start",checked:C,name:"auto_start",onChange:t=>{B(t.target.checked)},type:"checkbox",style:{width:"20px",height:"20px"}})})}),e.jsxs(c,{noTopSeparator:!0,children:[e.jsx(n,{labelFor:"capture_subdomains",rowOnly:!0,title:"Capture sub-domains",subTitle:r.capture_subdomains,children:e.jsx(i,{onChange:t=>{D(t.target.checked)},checked:w,type:"checkbox",name:"capture_subdomains",id:"capture_subdomains",style:{width:"20px",height:"20px"}})}),e.jsx(n,{rowOnly:!0,labelFor:"disable_tcp_tunnel",title:"Disable TCP tunnel mode",subTitle:r.disable_tcp_tunnel,children:e.jsx(i,{type:"checkbox",checked:_,onChange:t=>{O(t.target.checked)},id:"disable_tcp_tunnel",name:"disable_tcp_tunnel",style:{width:"20px",height:"20px"}})}),e.jsx(n,{rowOnly:!0,labelFor:"forward_subdomains",title:"Forward sub-domains",subTitle:r.forward_subdomains,children:e.jsx(i,{type:"checkbox",checked:v,onChange:t=>{A(t.target.checked)},id:"forward_subdomains",name:"forward_subdomains",style:{width:"20px",height:"20px"}})})]}),e.jsx(c,{noTopSeparator:!0,children:e.jsx(n,{title:"Log format",subTitle:r.log_format,children:e.jsxs("select",{className:"text-black rounded pl-3 pr-3",value:T,onChange:t=>{L(t.target.value)},name:"log_format",style:{height:"32px",width:"100%"},children:[e.jsx("option",{value:x.Standard,children:"Standard"}),e.jsx("option",{value:x.Dotnet,children:"Dotnet"})]})})}),e.jsx("div",{style:{marginTop:"20px"}}),e.jsx(n,{title:"Hints",subTitle:r.h2_hint}),e.jsxs("div",{style:{display:"flex",gap:"10px",flexWrap:"wrap",justifyContent:"start",marginTop:"10px",marginBottom:"20px"},children:[e.jsx(u,{onClick:()=>{o.includes(s.H2)?l(t=>[...t.filter(d=>d!==s.H2)]):l(t=>[...t,s.H2])},checked:o.includes(s.H2),title:"H2"}),e.jsx(u,{onClick:()=>{o.includes(s.H2C)?l(t=>[...t.filter(d=>d!==s.H2C)]):l(t=>[...t,s.H2C])},checked:o.includes(s.H2C),title:"H2C"}),e.jsx(u,{onClick:()=>{o.includes(s.H2CPK)?l(t=>[...t.filter(d=>d!==s.H2CPK)]):l(t=>[...t,s.H2CPK])},checked:o.includes(s.H2CPK),title:"H2CPK"}),e.jsx(u,{onClick:()=>{o.includes(s.NOH2)?l(t=>[...t.filter(d=>d!==s.NOH2)]):l(t=>[...t,s.NOH2])},checked:o.includes(s.NOH2),title:"NOH2"})]}),e.jsx(c,{noBottomSeparator:!0,children:e.jsx(n,{vertical:!0,title:"Environment variables",subTitle:r.env_vars,children:e.jsx(i,{value:f,onChange:t=>{K(t.target.value)}})})}),e.jsx(c,{noBottomSeparator:!0,noTopSeparator:!0,children:e.jsx(n,{vertical:!0,title:"Arguments",subTitle:r.args,children:e.jsx(i,{disableSaveButton:p.isPending,value:y,onChange:t=>E(t.target.value)})})}),e.jsx("div",{style:{display:"flex",alignItems:"center",justifyContent:"end",marginTop:"20px"},children:e.jsx(R,{variant:"start",loadingText:"Creating..",isLoading:p.isPending,className:"uppercase w-max-content font-bold",size:"sm",onClick:I,children:"Create site"})})]})},Y=()=>e.jsx("main",{className:"grid flex-1 items-start gap-4 md:pb-8 md:gap-8 max-w-[900px]",children:e.jsxs(W,{children:[e.jsxs(q,{children:[e.jsx(G,{children:"New hosted process"}),e.jsxs(J,{children:["Creating a process that odd-box will manage.",e.jsx("br",{}),"This is a service that odd-box can start, stop, and restart."]})]}),e.jsx(Q,{children:e.jsx(X,{})})]})}),$=U("/new-process")({component:Y});export{$ as Route};
