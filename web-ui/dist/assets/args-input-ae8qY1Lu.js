import{r as c,j as e}from"./index-B6VXrZlL.js";import{P as x,O as p,I as u,B as i}from"./setting_descriptions-BY7o_I-X.js";const h=({defaultKeys:n,onAddArg:t,onRemoveArg:r})=>{const[a,s]=c.useState({show:!1,value:"",originalValue:void 0}),o=()=>{s({show:!0,value:"",originalValue:void 0})};return e.jsxs(e.Fragment,{children:[e.jsxs("div",{style:{background:"var(--color3)",color:"black",marginTop:"10px",borderRadius:"5px",overflow:"hidden"},children:[n==null?void 0:n.map(l=>e.jsx("div",{onClick:()=>{s({show:!0,value:l,originalValue:l})},className:"env-var-item",style:{display:"flex",justifyContent:"space-between",alignItems:"center",padding:"5px"},children:e.jsx("p",{style:{zIndex:1,fontSize:".8rem"},children:l})},l)),e.jsx("div",{onClick:o,className:"env-var-item",style:{display:"flex",justifyContent:"space-between",alignItems:"center",padding:"5px"},children:e.jsxs("div",{style:{zIndex:1,fontSize:".8rem",display:"flex",alignItems:"center",gap:"5px"},children:[e.jsx(x,{}),"New argument"]})})]}),e.jsx(p,{show:a.show,onClose:()=>s(l=>({...l,show:!1})),title:a.originalValue?"Edit argument":"New argument",children:e.jsxs("div",{style:{display:"flex",flexDirection:"column",gap:"10px"},children:[e.jsxs("div",{children:[e.jsx("p",{style:{fontSize:".8rem"},children:"VALUE"}),e.jsx(u,{type:"text",value:a.value,onChange:l=>s(d=>({...d,value:l.target.value}))})]}),e.jsxs("div",{style:{display:"flex",justifyContent:"space-between",gap:"10px",marginTop:"5px"},children:[a.originalValue!==void 0&&e.jsx(i,{dangerButton:!0,onClick:()=>{r(a.originalValue),s(l=>({...l,show:!1}))},children:"Delete"}),e.jsx(i,{secondary:!0,onClick:()=>s(l=>({...l,show:!1})),children:"Cancel"}),e.jsx(i,{disabled:a.value==="",onClick:()=>{t(a.value,a.originalValue),s(l=>({...l,show:!1}))},children:"Save"})]})]})})]})};export{h as A};
