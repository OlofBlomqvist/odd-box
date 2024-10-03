import{j as e,d as s}from"./index-B6VXrZlL.js";const r=()=>e.jsxs("div",{style:{paddingBottom:"50px"},children:[e.jsx("p",{style:{textTransform:"uppercase",fontSize:".9rem",fontWeight:"bold",color:"var(--color2)"},children:"Home"}),e.jsxs("div",{style:{marginTop:"20px"},children:[e.jsx("h3",{children:"ODD-BOX"}),e.jsx("hr",{}),e.jsxs("div",{style:{fontSize:".9rem",marginTop:"10px",maxWidth:"750px"},children:[e.jsx("p",{children:"A simple to use cross-platform toy-level reverse proxy server for local development and tinkering purposes."}),e.jsx("br",{}),e.jsx("p",{children:"It allows you to configure a list of processes to run and host them behind their own custom hostnames. Automatically generates (self-signed) certificates for HTTPS when accessing them the first time (cached in .odd-box-cache dir)."}),e.jsx("br",{}),e.jsx("p",{children:"Uses the 'port' environment variable to assign a port for each site. If your process does not support using the port environment variable, you can pass custom arguments or variables for your process instead."}),e.jsx("br",{}),e.jsx("p",{children:'You can enable or disable all sites or specific ones using the http://localhost/START and http://localhost/STOP endpoints, optionally using query parameter "?proc=my_site" to stop or start a specific site. (Mostly only useful for pre-build scripts where you dont want to manually stop and start the proxy on each rebuild. Sites start automatically again on the next request)'}),e.jsx("br",{}),e.jsx("h3",{children:"Main Features & Goals"}),e.jsxs("ul",{children:[e.jsx("li",{children:"Cross platform (win/lin/osx)"}),e.jsx("li",{children:"Easy to configure"}),e.jsx("li",{children:"Keep a list of specified binaries running"}),e.jsx("li",{children:"Uses PORT environment variable for routing"}),e.jsx("li",{children:"Allows for setting proc specific and global env vars"}),e.jsx("li",{children:"Remote target proxying"}),e.jsx("li",{children:"Terminating proxy that supports both HTTP/1.1 & HTTP2"}),e.jsx("li",{children:"TCP tunnelling for HTTP/1"}),e.jsx("li",{children:"TCP tunnelling for HTTPS/1 via SNI sniffing"}),e.jsx("li",{children:"TCP tunnelling for HTTP/2 over HTTP/1 (h2c upgrade)"}),e.jsx("li",{children:"H2C via terminating proxy"}),e.jsx("li",{children:"Automatic self-signed certs for all hosted processes "})]})]})]})]}),o=s("/")({component:r});export{o as Route};
