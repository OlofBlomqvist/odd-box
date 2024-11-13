import{r as n,c as f,p as v,n as T,j as e,S as g,i as s,k as a,I as r,q as c,B as C,s as N,C as k,d as D,f as E,g as L,h as F,m as P}from"./index-BW3JFscn.js";const B=()=>{const[l,m]=n.useState(""),[d,y]=n.useState(""),[h,b]=n.useState(!1),[u,j]=n.useState(!1),[p,_]=n.useState(!1),{data:i}=f(),w=v(),{updateDirServer:x}=T(),S=()=>{x.mutateAsync({siteSettings:{host_name:l,dir:d,capture_subdomains:h,enable_directory_browsing:u,enable_lets_encrypt:p}},{onSettled(t,H,o){o.hostname!==o.siteSettings.host_name&&w.navigate({to:"/site",search:{tab:1,hostname:N(o.siteSettings.host_name)}})}})};return e.jsxs(e.Fragment,{children:[e.jsxs(g,{marginTop:"0px",noTopSeparator:!0,children:[e.jsx(s,{title:"Hostname",subTitle:a.hostname_frontend,children:e.jsx(r,{value:l,placeholder:"my-server.local",onChange:t=>m(t.target.value)})}),e.jsx(s,{title:"Directory",subTitle:a.directory,children:e.jsx(r,{value:d,placeholder:"/home/me/mysite",onChange:t=>y(t.target.value)})}),e.jsx(s,{dangerText:e.jsxs("span",{className:"text-[.8rem]",children:["This is the HTTP port configured for all sites, you can change it on the ",e.jsx(c,{className:"text-[var(--accent-text)] underline cursor-pointer",to:"/settings",children:"general settings"})," page."]}),title:"HTTP Port",children:e.jsx(r,{value:i.http_port,readOnly:!0,disabled:!0})}),e.jsx(s,{dangerText:e.jsxs("span",{className:"text-[.8rem]",children:["This is the TLS port configured for all sites, you can change it on the ",e.jsx(c,{className:"text-[var(--accent-text)] underline cursor-pointer",to:"/settings",children:"general settings"})," page."]}),title:"TLS Port",children:e.jsx(r,{value:i.tls_port,readOnly:!0,disabled:!0})})]}),e.jsxs(g,{noTopSeparator:!0,children:[e.jsx(s,{labelFor:"capture_subdomains",rowOnly:!0,title:"Capture sub-domains",subTitle:a.capture_subdomains,children:e.jsx(r,{onChange:t=>{b(t.target.checked)},checked:h,type:"checkbox",name:"capture_subdomains",id:"capture_subdomains",style:{width:"20px",height:"20px"}})}),e.jsx(s,{rowOnly:!0,labelFor:"enable_directory_browsing",title:"Enable directory browsing",subTitle:a.enable_directory_browsing,children:e.jsx(r,{type:"checkbox",checked:u,onChange:t=>{j(t.target.checked)},id:"enable_directory_browsing",name:"enable_directory_browsing",style:{width:"20px",height:"20px"}})}),e.jsx(s,{rowOnly:!0,labelFor:"lets_encrypt",title:"Enable Lets-Encrypt",dangerText:e.jsxs("p",{className:"text-[.8rem]",children:["Note: You need to have a valid email address configured under"," ",e.jsx(c,{className:"text-[var(--accent-text)] underline cursor-pointer",to:"/settings",children:"general settings"})," ","to use this."]}),children:e.jsx(r,{disabled:!i.lets_encrypt_account_email,type:"checkbox",checked:p,onChange:t=>{_(t.target.checked)},id:"lets_encrypt",name:"lets_encrypt",style:{width:"20px",height:"20px"}})})]}),e.jsx("div",{style:{display:"flex",alignItems:"center",justifyContent:"end",marginTop:"20px"},children:e.jsx(C,{variant:"start",loadingText:"Creating..",isLoading:x.isPending,className:"uppercase w-max-content font-bold text-white",size:"sm",onClick:S,children:"Create site"})})]})},O=()=>e.jsx("main",{className:"grid flex-1 items-start gap-4 md:gap-8 max-w-[900px]",children:e.jsxs(k,{children:[e.jsxs(D,{children:[e.jsx(E,{children:"New directory server"}),e.jsxs(L,{children:["A directory server configuration allows you to serve files from a directory on the local filesystem.",e.jsx("br",{}),"Both unencrypted (http) and encrypted (https) connections are supported, either self-signed or thru lets-encrypt.",e.jsx("br",{}),"You can specify rules for how the cache should behave, and you can also specify rules for how the files should be served."]})]}),e.jsx(F,{children:e.jsx(B,{})})]})}),R=P("/new-dirserver")({component:O});export{R as Route};