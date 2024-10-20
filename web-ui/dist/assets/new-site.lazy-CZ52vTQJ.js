import{r as i,k as N,j as e,S as h,g as r,h as o,I as c,l as d,H as s,B as v,C as P,c as O,d as F,e as R,f as K,i as D}from"./index-C2HrC9wm.js";const I=()=>{const[p,C]=i.useState(""),[x,f]=i.useState(""),[u,H]=i.useState(80),[m,w]=i.useState(!0),[b,_]=i.useState(!1),[j,S]=i.useState(!1),[g,k]=i.useState(!1),[a,n]=i.useState([]),{updateRemoteSite:y}=N(),T=()=>{u&&y.mutateAsync({siteSettings:{host_name:x,backends:[{address:p,https:m,port:u,hints:a}],capture_subdomains:b,disable_tcp_tunnel_mode:j,forward_subdomains:g}})};return e.jsxs(e.Fragment,{children:[e.jsxs(h,{marginTop:"0px",noTopSeparator:!0,children:[e.jsx(r,{title:"Remote hostname",subTitle:o.remote_site_address,children:e.jsx(c,{value:p,placeholder:"my-site.com",onChange:t=>C(t.target.value)})}),e.jsx(r,{title:"Hostname",subTitle:o.hostname_frontend,children:e.jsx(c,{value:x,placeholder:"my-site-redirected.com",onChange:t=>f(t.target.value)})}),e.jsx(r,{title:"Port",subTitle:o.port,children:e.jsx(c,{value:u,onChange:t=>{isNaN(Number(t.target.value))||H(Number(t.target.value))}})})]}),e.jsx(h,{noTopSeparator:!0,children:e.jsx(r,{labelFor:"use_https",rowOnly:!0,title:"HTTPS",subTitle:o.https,children:e.jsx(c,{checked:m,onChange:t=>{w(t.target.checked)},name:"use_https",id:"use_https",type:"checkbox",style:{width:"20px",height:"20px"}})})}),e.jsxs(h,{noTopSeparator:!0,children:[e.jsx(r,{labelFor:"capture_subdomains",rowOnly:!0,title:"Capture sub-domains",subTitle:o.capture_subdomains,children:e.jsx(c,{onChange:t=>{_(t.target.checked)},checked:b,type:"checkbox",name:"capture_subdomains",id:"capture_subdomains",style:{width:"20px",height:"20px"}})}),e.jsx(r,{rowOnly:!0,labelFor:"disable_tcp_tunnel",title:"Disable TCP tunnel mode",subTitle:o.disable_tcp_tunnel,children:e.jsx(c,{type:"checkbox",checked:j,onChange:t=>{S(t.target.checked)},id:"disable_tcp_tunnel",name:"disable_tcp_tunnel",style:{width:"20px",height:"20px"}})}),e.jsx(r,{rowOnly:!0,labelFor:"forward_subdomains",title:"Forward sub-domains",subTitle:o.forward_subdomains,children:e.jsx(c,{type:"checkbox",checked:g,onChange:t=>{k(t.target.checked)},id:"forward_subdomains",name:"forward_subdomains",style:{width:"20px",height:"20px"}})})]}),e.jsx(r,{title:"Hints",subTitle:o.h2_hint}),e.jsxs("div",{style:{display:"flex",gap:"10px",flexWrap:"wrap",justifyContent:"start",marginTop:"10px"},children:[e.jsx(d,{onClick:()=>{a.includes(s.H2)?n(t=>[...t.filter(l=>l!==s.H2)]):n(t=>[...t,s.H2])},checked:a.includes(s.H2),title:"H2"}),e.jsx(d,{onClick:()=>{a.includes(s.H2C)?n(t=>[...t.filter(l=>l!==s.H2C)]):n(t=>[...t,s.H2C])},checked:a.includes(s.H2C),title:"H2C"}),e.jsx(d,{onClick:()=>{a.includes(s.H2CPK)?n(t=>[...t.filter(l=>l!==s.H2CPK)]):n(t=>[...t,s.H2CPK])},checked:a.includes(s.H2CPK),title:"H2CPK"}),e.jsx(d,{onClick:()=>{a.includes(s.NOH2)?n(t=>[...t.filter(l=>l!==s.NOH2)]):n(t=>[...t,s.NOH2])},checked:a.includes(s.NOH2),title:"NOH2"})]}),e.jsx("div",{style:{display:"flex",alignItems:"center",justifyContent:"end",marginTop:"20px"},children:e.jsx(v,{onClick:T,style:{width:"max-content",background:"var(--color7)"},children:"Create site"})})]})},M=()=>e.jsx("main",{className:"grid flex-1 items-start gap-4 md:pb-8 md:gap-8 max-w-[900px]",children:e.jsxs(P,{children:[e.jsxs(O,{children:[e.jsx(F,{children:"New remote site"}),e.jsxs(R,{children:["A remote site forwards traffic to external servers.",e.jsx("br",{}),"You can add more backends to a site after creating it."]})]}),e.jsx(K,{children:e.jsx(I,{})})]})}),B=D("/new-site")({component:M});export{B as Route};
