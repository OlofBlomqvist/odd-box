import{j as e,S as m,r as p}from"./index-C4yZE6Dy.js";import{I as j,B as c}from"./button-DCs68cQO.js";import{S as u,a as g,b as S,c as w,d as f,e as v,f as b,T as C,g as N,h,i as x,j as T,D as B}from"./env_variables-C_Ck8DGs.js";const A=({show:a,value:l,originalValue:t,onClose:r,valueChanged:n,onAddArg:i,onRemoveArg:o})=>e.jsx(u,{open:a,onOpenChange:r,children:e.jsxs(g,{className:"bg-[#242424] border-l-[#ffffff10] w-full",children:[e.jsxs(S,{className:"text-left",children:[e.jsx(w,{className:"text-white",children:t!==""?"Edit argument":"New argument"}),e.jsx(f,{children:t===""?"Add a new argument":`Making changes to '${t}'`})]}),e.jsx(m,{marginTop:"10px",noBottomSeparator:!0,children:e.jsx("div",{style:{display:"flex",flexDirection:"column",gap:"10px"},children:e.jsx("div",{children:e.jsx(j,{withSaveButton:!0,placeholder:"Example: use-kestrel",originalValue:t,onSave:()=>{i(l,t),r()},type:"text",value:l,onChange:s=>n(s.target.value)})})})}),e.jsxs(v,{className:"flex flex-row gap-4",children:[t&&e.jsx(c,{onClick:()=>{o(t),r()},style:{width:"150px",whiteSpace:"nowrap",display:"flex",alignItems:"center",gap:"5px",justifyContent:"center"},dangerButton:!0,children:"Delete"}),e.jsx(b,{asChild:!0,children:e.jsx(c,{type:"submit",children:"Close"})})]})]})}),F=({defaultKeys:a,onAddArg:l,onRemoveArg:t})=>{const[r,n]=p.useState({show:!1,value:"",originalValue:void 0}),i=()=>{n({show:!0,value:"",originalValue:""})},o=["hover:cursor-pointer"];return(a==null?void 0:a.length)===0&&o.push("border-0"),e.jsxs(e.Fragment,{children:[e.jsxs(C,{children:[e.jsx(N,{children:a==null?void 0:a.map(s=>e.jsx(h,{className:"hover:cursor-pointer",onClick:()=>{n({show:!0,value:s,originalValue:s})},children:e.jsx(x,{className:"font-medium",children:s})},JSON.stringify(s)))}),e.jsx(T,{className:o.join(" "),children:e.jsx(h,{onClick:i,children:e.jsx(x,{className:"bg-transparent",colSpan:3,children:e.jsxs("div",{className:"flex items-center gap-2 justify-center",children:[e.jsx(B,{}),e.jsx("span",{children:"Add new argument"})]})})})})]}),e.jsx(A,{onAddArg:l,onRemoveArg:t,onClose:()=>n(s=>({...s,show:!1})),originalValue:r.originalValue,show:r.show,value:r.value,valueChanged:s=>n(d=>({...d,value:s}))})]})};export{F as A};
