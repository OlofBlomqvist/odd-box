import{A as v,u as b,a as f,b as N,j as t,r as p,S as g,c as r,L as T,_ as P,d as C}from"./index-BAcZo2i6.js";import{S as n,I as i,E as k}from"./env_variables-DBgTPTRJ.js";const I=()=>{let s=window.location.protocol+"//"+window.location.hostname;window.location.port&&(s=`${s}:${window.location.port}`);const a=s,d=new v({baseUrl:a});return b({queryKey:["settings"],select:c=>c.data,queryFn:d.api.settings})},R=()=>{let s=window.location.protocol+"//"+window.location.hostname;window.location.port&&(s=`${s}:${window.location.port}`);const a=s,d=new v({baseUrl:a}),c=f();return{updateSettings:N({mutationKey:["update-settings"],mutationFn:d.api.saveSettings,onSettled:()=>{c.invalidateQueries({queryKey:["settings"]})}})}},A=()=>t.jsx(p.Suspense,{fallback:t.jsx("p",{children:"loading settings.."}),children:t.jsx(B,{})}),B=()=>{const{updateSettings:s}=R(),{data:a}=I(),[d,c]=p.useState(a.ip),[h,x]=p.useState(a.root_dir),[S,_]=p.useState(a.http_port),[m,j]=p.useState(a.tls_port),[w,y]=p.useState(a.port_range_start),o=(e,l)=>{let u=Array.isArray(l)||isNaN(l)===!1?l:`${l}`;P.promise(s.mutateAsync({...a,[e]:u}),{loading:"Updating settings..",success:"Settings updated!",error:"Failed to update settings"})};return t.jsxs("div",{style:{paddingBottom:"50px",maxWidth:"750px"},children:[t.jsx("p",{style:{textTransform:"uppercase",fontSize:".9rem",fontWeight:"bold",color:"var(--color2)"},children:"Settings"}),t.jsx("p",{style:{fontSize:".9rem",marginBottom:"30px"},children:"General settings that affect all sites"}),t.jsx(g,{noTopSeparator:!0,noBottomSeparator:!0,children:t.jsx(r,{title:"Root directory",subTitle:n.root_dir,children:t.jsx(i,{withSaveButton:!0,onSave:e=>{o("root_dir",e)},type:"text",originalValue:a.root_dir,value:h,onChange:e=>x(e.target.value)})})}),t.jsxs(g,{children:[t.jsx(r,{title:"HTTP Port",subTitle:n.default_http_port,defaultValue:"8080",children:t.jsx(i,{value:S,withSaveButton:!0,originalValue:a.http_port,onSave:e=>{o("http_port",e)},onChange:e=>{isNaN(Number(e.target.value))||_(Number(e.target.value))}})}),t.jsx(r,{title:"TLS Port",subTitle:n.default_tls_port,defaultValue:"4343",children:t.jsx(i,{value:m,originalValue:a.tls_port,withSaveButton:!0,onSave:e=>{o("tls_port",e)},onChange:e=>{isNaN(Number(e.target.value))||j(Number(e.target.value))}})}),t.jsx(r,{title:"IP Address",subTitle:n.proxy_ip,children:t.jsx(i,{value:d,originalValue:a.ip,withSaveButton:!0,onSave:e=>{o("ip",e)},onChange:e=>c(e.target.value)})})]}),t.jsxs(g,{noTopSeparator:!0,children:[t.jsx(r,{title:"Port range start",subTitle:n.port_range_start,children:t.jsx(i,{value:w,originalValue:a.port_range_start,withSaveButton:!0,onSave:e=>o("port_range_start",e),onChange:e=>{isNaN(Number(e.target.value))||y(Number(e.target.value))}})}),t.jsx(r,{title:"Use ALPN",labelFor:"alpn",subTitle:n.use_alpn,rowOnly:!0,children:t.jsx(i,{type:"checkbox",id:"alpn",checked:a.alpn,onChange:()=>o("alpn",!a.alpn),style:{width:"20px",height:"20px"}})})]}),t.jsx(g,{noTopSeparator:!0,children:t.jsx(r,{title:"Autostart",rowOnly:!0,subTitle:n.default_auto_start,labelFor:"autostart",children:t.jsx(i,{id:"autostart",type:"checkbox",checked:a.auto_start,onChange:()=>o("auto_start",!a.auto_start),style:{width:"20px",height:"20px"}})})}),t.jsxs(g,{noTopSeparator:!0,children:[t.jsx(r,{title:"Log level",subTitle:n.log_level,children:t.jsxs("select",{className:"text-black rounded pl-3 pr-3",value:a.log_level,onChange:e=>{o("log_level",e.target.value)},name:"loglevel",style:{height:"32px",width:"100%"},children:[t.jsx("option",{value:"Trace",children:"Trace"}),t.jsx("option",{value:"Debug",children:"Debug"}),t.jsx("option",{value:"Info",children:"Info"}),t.jsx("option",{value:"Warn",children:"Warn"}),t.jsx("option",{value:"Error",children:"Error"})]})}),t.jsx(r,{title:"Log format",subTitle:n.default_log_format,children:t.jsxs("select",{className:"text-black rounded pl-3 pr-3",value:a.default_log_format??T.Standard,onChange:e=>{o("default_log_format",e.target.value)},name:"log_format",style:{height:"32px",width:"100%"},children:[t.jsx("option",{value:"Standard",children:"Standard"}),t.jsx("option",{value:"Dotnet",children:"Dotnet"})]})})]}),t.jsx(r,{vertical:!0,title:"Environment variables",subTitle:n.global_env_vars,children:t.jsx(k,{keys:a.env_vars??[],onRemoveKey:e=>{var l;o("env_vars",(l=a.env_vars)==null?void 0:l.filter(u=>u.key!==e))},onNewKey:(e,l)=>{o("env_vars",[...a.env_vars.filter(u=>u.key!==e.key&&u.key!==l),{key:e.key,value:e.value}])}})})]})},E=C("/settings")({component:A});export{E as Route};
