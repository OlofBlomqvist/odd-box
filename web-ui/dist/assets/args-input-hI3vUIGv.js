import{j as e,S as d,r as x}from"./index-VhmgqZXc.js";import{a as h,b as p,c as m,d as u,e as g,I as j,f,B as c,g as v,P as w}from"./setting_descriptions-C0WjOK5n.js";const S=({show:r,value:l,originalValue:s,onClose:n,valueChanged:a,onAddArg:i,onRemoveArg:t})=>e.jsx(h,{open:r,onOpenChange:n,children:e.jsxs(p,{className:"bg-[#242424] border-l-[#ffffff10] w-full",children:[e.jsxs(m,{className:"text-left",children:[e.jsx(u,{className:"text-white",children:s!==""?"Edit argument":"New argument"}),e.jsx(g,{children:s===""?"Add a new argument":`Making changes to '${s}'`})]}),e.jsx(d,{marginTop:"10px",noBottomSeparator:!0,children:e.jsx("div",{style:{display:"flex",flexDirection:"column",gap:"10px"},children:e.jsx("div",{children:e.jsx(j,{withSaveButton:!0,placeholder:"Argument here..",originalValue:s,onSave:()=>{i(l,s),n()},type:"text",value:l,onChange:o=>a(o.target.value)})})})}),e.jsxs(f,{className:"flex flex-row gap-4",children:[s&&e.jsx(c,{onClick:()=>{t(s),n()},style:{width:"150px",whiteSpace:"nowrap",display:"flex",alignItems:"center",gap:"5px",justifyContent:"center"},dangerButton:!0,children:"Delete"}),e.jsx(v,{asChild:!0,children:e.jsx(c,{type:"submit",children:"Close"})})]})]})}),y=({defaultKeys:r,onAddArg:l,onRemoveArg:s})=>{const[n,a]=x.useState({show:!1,value:"",originalValue:void 0}),i=()=>{a({show:!0,value:"",originalValue:""})};return e.jsxs(e.Fragment,{children:[e.jsxs("div",{style:{background:"var(--color3)",color:"black",marginTop:"10px",borderRadius:"5px",overflow:"hidden"},children:[r==null?void 0:r.map(t=>e.jsx("div",{onClick:()=>{a({show:!0,value:t,originalValue:t})},className:"env-var-item",style:{display:"flex",justifyContent:"space-between",alignItems:"center",padding:"5px"},children:e.jsx("p",{style:{zIndex:1,fontSize:".8rem"},children:t})},t)),e.jsx("div",{onClick:i,className:"env-var-item",style:{display:"flex",justifyContent:"space-between",alignItems:"center",padding:"5px"},children:e.jsxs("div",{style:{zIndex:1,fontSize:".8rem",display:"flex",alignItems:"center",gap:"5px"},children:[e.jsx(w,{}),"New argument"]})})]}),e.jsx(S,{onAddArg:l,onRemoveArg:s,onClose:()=>a(t=>({...t,show:!1})),originalValue:n.originalValue,show:n.show,value:n.value,valueChanged:t=>a(o=>({...o,value:t}))})]})};export{y as A};